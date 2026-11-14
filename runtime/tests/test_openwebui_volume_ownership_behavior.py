"""Executable mock-Docker tests for Open WebUI volume ownership."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUIVolumeOwnershipBehaviorTests(unittest.TestCase):
    def check_volume(self, labels, volume="open-webui", inspect_exit=0):
        with tempfile.TemporaryDirectory(prefix="daia-webui-volume-") as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "docker.log"

            docker = bin_dir / "docker"
            docker.write_text(
                "#!/bin/sh\n"
                'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
                'case "$*" in\n'
                '  *volume\\ inspect*)\n'
                '    printf "%s\\n" "$DAIA_TEST_VOLUME_LABELS"\n'
                '    exit "$DAIA_TEST_INSPECT_EXIT" ;;\n'
                '  *) exit 99 ;;\n'
                'esac\n'
            )
            docker.chmod(0o755)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env.get("PATH", "")
            env["DAIA_TEST_DOCKER_LOG"] = str(log)
            env["DAIA_TEST_VOLUME_LABELS"] = labels.replace("\\t", "\t")
            env["DAIA_TEST_INSPECT_EXIT"] = str(inspect_exit)
            env["DAIA_TEST_MODULE"] = str(MODULE)
            env["DAIA_TEST_VOLUME"] = volume

            script = (
                'source "$DAIA_TEST_MODULE"\n'
                'openwebui_volume_owned "$DAIA_TEST_VOLUME"\n'
            )

            result = subprocess.run(
                ["bash", "-c", script],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
            )

            events = log.read_text() if log.exists() else ""
            return result, events

    def assert_read_only(self, events):
        self.assertNotIn("volume create", events)
        self.assertNotIn("volume rm", events)
        self.assertNotIn("container create", events)
        self.assertNotIn("container start", events)
        self.assertNotIn("docker run", events)

    def test_accepts_verified_volume(self):
        result, events = self.check_volume("openwebui\\ttrue")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("volume inspect", events)
        self.assert_read_only(events)

    def test_rejects_wrong_component(self):
        result, events = self.check_volume("pgvector\\ttrue")
        self.assertNotEqual(result.returncode, 0)
        self.assert_read_only(events)

    def test_rejects_unmanaged_volume(self):
        result, events = self.check_volume("openwebui\\tfalse")
        self.assertNotEqual(result.returncode, 0)
        self.assert_read_only(events)

    def test_rejects_missing_labels(self):
        result, events = self.check_volume("")
        self.assertNotEqual(result.returncode, 0)
        self.assert_read_only(events)

    def test_rejects_extra_fields(self):
        result, events = self.check_volume("openwebui\\ttrue\\textra")
        self.assertNotEqual(result.returncode, 0)
        self.assert_read_only(events)

    def test_rejects_unexpected_volume_name(self):
        result, events = self.check_volume(
            "openwebui\\ttrue", volume="untrusted"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("volume inspect", events)
        self.assert_read_only(events)

    def test_rejects_docker_inspection_failure(self):
        result, events = self.check_volume(
            "openwebui\\ttrue", inspect_exit=1
        )
        self.assertNotEqual(result.returncode, 0)
        self.assert_read_only(events)


if __name__ == "__main__":
    unittest.main()
