"""Behavioral verification of the Open WebUI installation gate."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUIRAGGateBehaviorTests(unittest.TestCase):

    def test_missing_authorizer_prevents_docker_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()

            log = root / "docker.log"
            image = root / "images" / "open-webui.tar"
            image.parent.mkdir()
            image.write_bytes(b"test-image")

            docker = bin_dir / "docker"
            docker.write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' \"$*\" >> \"$DAIA_DOCKER_LOG\"\n"
                "case \"$1\" in\n"
                "  info) exit 0 ;;\n"
                "  *) exit 99 ;;\n"
                "esac\n"
            )
            docker.chmod(0o755)

            env = os.environ.copy()
            env.update({
                "PATH": str(bin_dir) + os.pathsep + env["PATH"],
                "DAIA_PAYLOAD_ROOT": str(root),
                "DAIA_DOCKER_LOG": str(log),
            })

            # Isolate the test from host-installed RAG helpers.
            isolated_module = root / "openwebui-isolated.sh"
            original_module = MODULE.read_text(encoding="utf-8")
            authorization_path = (
                "/usr/libexec/daia-openwebui-rag-authorize"
            )
            self.assertIn(authorization_path, original_module)
            isolated_module.write_text(
                original_module.replace(
                    authorization_path,
                    str(root / "missing-rag-authorizer"),
                ),
                encoding="utf-8",
            )

            script = (
                'source "$1"\n'
                'openwebui_prepare_rag() { return 0; }\n'
                'openwebui_install\n'
            )

            result = subprocess.run(
                ["bash", "-c", script, "bash", str(isolated_module)],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("RAG authorization", result.stderr)

            operations = log.read_text().splitlines() if log.exists() else []
            self.assertEqual(
                operations,
                ["info", "container inspect open-webui"],
            )


if __name__ == "__main__":
    unittest.main()
