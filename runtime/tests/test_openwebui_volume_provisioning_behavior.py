"""Isolated behavioral tests for Open WebUI volume provisioning."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUIVolumeProvisioningBehaviorTests(unittest.TestCase):

    def run_case(self, inventory, labels="openwebui\ttrue",
                 inventory_exit=0, create_exit=0):
        with tempfile.TemporaryDirectory(prefix="daia-webui-provision-") as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "docker.log"

            docker = bin_dir / "docker"
            docker.write_text(
                '#!/bin/sh\n'
                'printf "%s\\n" "$*" >> "$DAIA_TEST_LOG"\n'
                'case "$1 $2" in\n'
                '  "volume ls")\n'
                '    printf "%s\\n" "$DAIA_TEST_INVENTORY"\n'
                '    exit "$DAIA_TEST_INVENTORY_EXIT" ;;\n'
                '  "volume inspect")\n'
                '    printf "%s\\n" "$DAIA_TEST_LABELS"\n'
                '    exit 0 ;;\n'
                '  "volume create")\n'
                '    exit "$DAIA_TEST_CREATE_EXIT" ;;\n'
                '  *) exit 99 ;;\n'
                'esac\n'
            )
            docker.chmod(0o755)

            env = os.environ.copy()
            env.update({
                "PATH": str(bin_dir) + os.pathsep + env.get("PATH", ""),
                "DAIA_TEST_LOG": str(log),
                "DAIA_TEST_INVENTORY": inventory,
                "DAIA_TEST_LABELS": labels,
                "DAIA_TEST_INVENTORY_EXIT": str(inventory_exit),
                "DAIA_TEST_CREATE_EXIT": str(create_exit),
                "DAIA_TEST_MODULE": str(MODULE),
            })

            result = subprocess.run(
                [
                    "bash", "-c",
                    'source "$DAIA_TEST_MODULE"; openwebui_ensure_volume'
                ],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )

            events = log.read_text().splitlines() if log.exists() else []
            return result, events

    def test_existing_trusted_volume_is_reused(self):
        result, events = self.run_case("open-webui")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any("volume inspect" in e for e in events))
        self.assertFalse(any("volume create" in e for e in events))

    def test_existing_untrusted_volume_is_rejected(self):
        result, events = self.run_case(
            "open-webui", labels="untrusted\tfalse"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any("volume create" in e for e in events))

    def test_new_volume_is_labeled_and_verified(self):
        result, events = self.run_case("")
        self.assertEqual(result.returncode, 0, result.stderr)

        creations = [e for e in events if "volume create" in e]
        self.assertEqual(len(creations), 1)
        self.assertIn("--label org.daia.component=openwebui", creations[0])
        self.assertIn("--label org.daia.managed=true", creations[0])
        self.assertTrue(any("volume inspect" in e for e in events))

    def test_new_untrusted_volume_is_rejected(self):
        result, events = self.run_case(
            "", labels="untrusted\tfalse"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(any("volume create" in e for e in events))
        self.assertTrue(any("volume inspect" in e for e in events))

    def test_inventory_failure_prevents_creation(self):
        result, events = self.run_case(
            "", inventory_exit=1
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any("volume create" in e for e in events))

    def test_creation_failure_is_rejected(self):
        result, events = self.run_case(
            "", create_exit=1
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(any("volume create" in e for e in events))
        self.assertFalse(any("volume inspect" in e for e in events))

    def test_no_container_mutation(self):
        result, events = self.run_case("open-webui")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any("container create" in e for e in events))
        self.assertFalse(any("container start" in e for e in events))
        self.assertFalse(any("volume rm" in e for e in events))


if __name__ == "__main__":
    unittest.main()
