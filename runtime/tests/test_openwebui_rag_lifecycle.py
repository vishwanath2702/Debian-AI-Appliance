"""Open WebUI RAG firstboot ordering and failure isolation."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "modules/openwebui.sh"
)


class RagLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.source = SOURCE.read_text(encoding="utf-8")

    def test_provisioning_precedes_authorization_and_docker_load(self):
        install = self.source.split("openwebui_install() {", 1)[1]
        self.assertLess(
            install.index('openwebui_prepare_rag "${tika_positional[@]}" || return 1'),
            install.index('/usr/libexec/daia-openwebui-rag-authorize "${tika_flags[@]}";'),
        )
        self.assertLess(
            install.index('/usr/libexec/daia-openwebui-rag-authorize "${tika_flags[@]}";'),
            install.index("docker load --input"),
        )

    def test_root_required_and_helpers_not_symlinks(self):
        prepare = self.source.split(
            "openwebui_prepare_rag() {", 1
        )[1].split("\nopenwebui_install() {", 1)[0]
        self.assertIn('[[ "$EUID" -ne 0 ]]', prepare)
        self.assertIn('-L "$helper"', prepare)
        self.assertIn('! -x "$helper"', prepare)

    def test_credential_precedes_database_role(self):
        prepare = self.source.split(
            "openwebui_prepare_rag() {", 1
        )[1].split("\nopenwebui_install() {", 1)[0]
        self.assertLess(
            prepare.index('if ! "$credential_helper";'),
            prepare.index('if ! "$provision_helper" "${provision_args[@]}";'),
        )

    def test_module_shell_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(SOURCE)],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_runtime_order_with_isolated_mock_helpers(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            log = directory / "events"
            credential = directory / "credential"
            provision = directory / "provision"
            authorize = directory / "authorize"

            for path, event, status in (
                (credential, "credential", 0),
                (provision, "provision", 0),
                (authorize, "authorize", 1),
            ):
                path.write_text(
                    "#!/bin/sh\n"
                    f"printf '%s\\n' '{event}' >> \"$EVENT_LOG\"\n"
                    f"exit {status}\n",
                    encoding="utf-8",
                )
                path.chmod(0o755)

            isolated = self.source
            replacements = (
                (
                    "/usr/libexec/daia-openwebui-pgvector-credentials.py",
                    str(credential),
                ),
                (
                    "/usr/libexec/daia-openwebui-rag-provision.py",
                    str(provision),
                ),
                (
                    "/usr/libexec/daia-openwebui-rag-authorize",
                    str(authorize),
                ),
            )
            for old, new in replacements:
                self.assertIn(old, isolated)
                isolated = isolated.replace(old, new)

            # Only the temporary test copy bypasses root.
            self.assertEqual(
                isolated.count('if [[ "$EUID" -ne 0 ]]; then'), 5
            )
            isolated = isolated.replace(
                'if [[ "$EUID" -ne 0 ]]; then',
                'if false; then',
                1,
            )

            mock_module = directory / "openwebui.sh"
            mock_module.write_text(
                isolated, encoding="utf-8"
            )

            script = (
                'source "$MODULE_PATH"\n'
                'openwebui_validate() { return 0; }\n'
                'docker() { printf "%s\\n" docker >> "$EVENT_LOG"; '
                'return 1; }\n'
                'openwebui_install\n'
            )
            env = dict(
                os.environ,
                MODULE_PATH=str(mock_module),
                EVENT_LOG=str(log),
            )
            result = subprocess.run(
                ["bash", "-c", script],
                env=env,
                capture_output=True,
                text=True,
                check=False,
                timeout=15,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(
                log.read_text(encoding="utf-8").splitlines(),
                ["docker", "credential", "provision", "authorize"],
            )

    def test_failed_credential_blocks_role(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            credential = directory / "credential"
            provision = directory / "provision"
            log = directory / "events"

            credential.write_text(
                '#!/bin/sh\nexit 1\n', encoding="utf-8"
            )
            provision.write_text(
                '#!/bin/sh\necho provision >> "$EVENT_LOG"\n',
                encoding="utf-8",
            )
            credential.chmod(0o755)
            provision.chmod(0o755)

            isolated = self.source.replace(
                "/usr/libexec/daia-openwebui-pgvector-credentials.py",
                str(credential),
            ).replace(
                "/usr/libexec/daia-openwebui-rag-provision.py",
                str(provision),
            ).replace(
                'if [[ "$EUID" -ne 0 ]]; then',
                'if false; then',
                1,
            )
            module = directory / "module.sh"
            module.write_text(isolated, encoding="utf-8")
            result = subprocess.run(
                [
                    "bash", "-c",
                    'source "$MODULE_PATH"; openwebui_prepare_rag',
                ],
                env=dict(
                    os.environ,
                    MODULE_PATH=str(module),
                    EVENT_LOG=str(log),
                ),
                capture_output=True,
                text=True,
                check=False,
                timeout=15,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(log.exists())


if __name__ == "__main__":
    unittest.main()
