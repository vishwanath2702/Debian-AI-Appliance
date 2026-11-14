"""Isolated behavioral tests for Open WebUI firstboot recovery."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"
IDENTITY = "a" * 64
IMAGE = (
    "sha256:"
    "e29d4d2457b54bb14434bee2666420e822cb0e9389466f02405d69fa7fd8d06c"
)


class InstallRecoveryTests(unittest.TestCase):
    def run_case(self, *, exists=False, state="created",
                 trusted=True, start=True, registration=True):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            log = folder / "actions.log"
            rag = folder / "rag-authorize"
            helper = folder / "register-container"
            isolated = folder / "isolated.sh"

            rag.write_text(
                '#!/bin/sh\n'
                'echo rag-authorize >> "$ACTION_LOG"\n'
                'exit 0\n',
                encoding="utf-8",
            )
            helper.write_text(
                '#!/bin/sh\n'
                'echo "helper:$1" >> "$ACTION_LOG"\n'
                'if [ "$1" = "--verify-identity" ] && '
                '[ "$MOCK_TRUSTED" != yes ]; then exit 1; fi\n'
                'if [ "$1" = "--verify-running" ]; then\n'
                '  printf "%s\\n" "$2"\n'
                'elif [ "$1" = "--verify-identity" ]; then\n'
                '  printf "%s\\n" "$2"\n'
                'elif [ "$MOCK_REGISTRATION" != yes ]; then\n'
                '  exit 1\n'
                'fi\n',
                encoding="utf-8",
            )
            rag.chmod(0o700)
            helper.chmod(0o700)

            source = MODULE.read_text(encoding="utf-8")
            install = source.split(
                "openwebui_install() {", 1
            )[1].split("\nopenwebui_verify() {", 1)[0]
            install = "openwebui_install() {" + install
            # Test-only root simulation. Production root guard is untouched.
            self.assertEqual(
                install.count('[[ "$EUID" -ne 0 ]] ||'), 1
            )
            install = install.replace(
                '[[ "$EUID" -ne 0 ]] ||',
                '[[ "$MOCK_ROOT" != yes ]] ||',
                1,
            )
            install = install.replace(
                "/usr/libexec/daia-openwebui-rag-authorize",
                str(rag),
            ).replace(
                "/usr/libexec/daia-openwebui-register-container",
                str(helper),
            )

            # Only the extracted installer runs. All external actions
            # are replaced by isolated shell functions and fake helpers.
            isolated.write_text(install, encoding="utf-8")

            script = r"""
docker() {
    echo "docker:$1:$2" >> "$ACTION_LOG"
    if [[ "$1" == container && "$2" == inspect ]]; then
        if [[ "${3:-}" != --format ]]; then
            [[ "$MOCK_EXISTS" == yes ]]
            return
        fi
        if [[ "$4" == '{{.Id}}' ]]; then
            printf '%s\n' "$MOCK_ID"
        else
            printf '%s\n' "$MOCK_STATE"
        fi
        return 0
    fi
    if [[ "$1" == load ]]; then
        echo "Loaded image: test-image"
    elif [[ "$1" == image && "$2" == inspect ]]; then
        echo "$MOCK_IMAGE"
    elif [[ "$1" == container && "$2" == create ]]; then
        echo "$MOCK_ID"
    fi
    return 0
}
openwebui_validate() { return 0; }
openwebui_prepare_rag() {
    echo prepare-rag >> "$ACTION_LOG"
    return 0
}
openwebui_ensure_volume() {
    echo ensure-volume >> "$ACTION_LOG"
    return 0
}
openwebui_start_registered() {
    echo "start:$1" >> "$ACTION_LOG"
    [[ "$MOCK_START" == yes ]]
}
source "$1"
openwebui_install
"""
            env = {
                **os.environ,
                "ACTION_LOG": str(log),
                "MOCK_EXISTS": "yes" if exists else "no",
                "MOCK_ROOT": "yes",
                "MOCK_STATE": (
                    "true|false|false|running"
                    if state == "running" else
                    "false|false|false|" + state
                ),
                "MOCK_ID": IDENTITY,
                "MOCK_IMAGE": IMAGE,
                "MOCK_TRUSTED": "yes" if trusted else "no",
                "MOCK_START": "yes" if start else "no",
                "MOCK_REGISTRATION": (
                    "yes" if registration else "no"
                ),
            }
            result = subprocess.run(
                ["bash", "-c", script, "bash", str(isolated)],
                cwd=folder, env=env,
                capture_output=True, text=True,
                check=False, timeout=15,
            )
            actions = (
                log.read_text(encoding="utf-8").splitlines()
                if log.exists() else []
            )
            return result, actions

    def test_new_install_registers_before_start(self):
        result, actions = self.run_case()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertLess(
            actions.index("helper:" + IDENTITY),
            actions.index("start:" + IDENTITY),
        )
        self.assertIn("docker:container:create", actions)
        self.assertIn("prepare-rag", actions)

    def test_existing_running_uses_read_only_proof(self):
        result, actions = self.run_case(
            exists=True, state="running"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("helper:--verify-identity", actions)
        self.assertIn("helper:--verify-running", actions)
        self.assertNotIn("prepare-rag", actions)
        self.assertFalse(any(a.startswith("start:") for a in actions))
        self.assertFalse(any(a.startswith("docker:load") for a in actions))

    def test_existing_stopped_resumes_by_id(self):
        result, actions = self.run_case(exists=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("helper:--verify-identity", actions)
        self.assertIn("start:" + IDENTITY, actions)
        self.assertNotIn("prepare-rag", actions)
        self.assertFalse(any(a.startswith("docker:load") for a in actions))

    def test_untrusted_existing_container_is_never_started(self):
        result, actions = self.run_case(
            exists=True, trusted=False
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(a.startswith("start:") for a in actions))
        self.assertFalse(any(a.startswith("docker:load") for a in actions))

    def test_registration_failure_blocks_start(self):
        result, actions = self.run_case(registration=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(a.startswith("start:") for a in actions))

    def test_start_failure_fails_installation(self):
        result, actions = self.run_case(start=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("start:" + IDENTITY, actions)


if __name__ == "__main__":
    unittest.main()
