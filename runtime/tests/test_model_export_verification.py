"""Isolated execution tests for DAIA exporter configuration verification."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
AI_MODULE = ROOT / "runtime/modules/ai.sh"

START = "    # Verify installed exporter configuration without activating its units.\n"
END = "    local service_environment\n"

FILES = (
    ("polkit-1/actions/org.daia.model-export.policy",
     "polkit/org.daia.model-export.policy"),
    ("services/daia-model-export-worker.service",
     "systemd/daia-model-export-worker.service"),
    ("services/daia-model-export.socket",
     "systemd/daia-model-export.socket"),
)


class ExportVerificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        content = AI_MODULE.read_text()
        if content.count(START) != 1 or content.count(END) != 1:
            raise AssertionError("Exporter verification boundaries changed")
        cls.block = content[
            content.index(START):content.index(END, content.index(START))
        ]

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="daia-export-verify-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.runtime = self.root / "runtime"
        self.polkit = self.root / "polkit"
        self.systemd = self.root / "systemd"

        for relative_source, relative_destination in FILES:
            source = self.runtime / relative_source
            destination = self.root / relative_destination
            source.parent.mkdir(parents=True, exist_ok=True)
            destination.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("trusted exporter configuration\n")
            shutil.copyfile(source, destination)
            destination.chmod(0o644)

        self.mock_bin = self.root / "bin"
        self.mock_bin.mkdir()

        systemctl = self.mock_bin / "systemctl"
        systemctl.write_text(
            "#!/bin/sh\n"
            'if [ "$1" != "show" ]; then exit 2; fi\n'
            'case "$3" in\n'
            '  --property=UnitFileState)\n'
            '    [ "${DAIA_TEST_INSPECTION_ERROR:-0}" = 1 ] && exit 2\n'
            '    printf "%s\n" "${DAIA_TEST_UNIT_STATE-static}"\n'
            '    ;;\n'
            '  --property=ActiveState)\n'
            '    [ "${DAIA_TEST_INSPECTION_ERROR:-0}" = 1 ] && exit 2\n'
            '    printf "%s\n" "${DAIA_TEST_ACTIVE_STATE:-inactive}"\n'
            '    ;;\n'
            '  *) exit 2 ;;\n'
            'esac\n'
        )
        systemctl.chmod(0o755)

        # Only the fixture's ownership is simulated. The real verifier
        # still requires root:root and mode 0644.
        stat = self.mock_bin / "stat"
        stat.write_text(
            "#!/bin/sh\n"
            'if [ "$1" = "-c" ] && [ "$2" = "%u:%g:%a" ]; then\n'
            '    shift 2\n'
            '    if [ "$1" = "--" ]; then shift; fi\n'
            '    mode=$(/usr/bin/stat -c %a -- "$1") || exit 1\n'
            '    printf "0:0:%s\\n" "$mode"\n'
            'else\n'
            '    exec /usr/bin/stat "$@"\n'
            'fi\n'
        )
        stat.chmod(0o755)

    def run_verification(
        self,
        enabled=False,
        active=False,
        unit_state=None,
        active_state=None,
        inspection_error=False,
    ):
        script = (
            'verify_exporter_fixture() {\n'
            '    local systemd_system_directory="$DAIA_TEST_SYSTEMD"\n'
            + self.block
            + '}\n'
            + 'verify_exporter_fixture\n'
        )

        environment = os.environ.copy()
        environment.update({
            "DAIA_TEST_RUNTIME": str(self.runtime),
            "DAIA_TEST_POLKIT": str(self.polkit),
            "DAIA_TEST_SYSTEMD": str(self.systemd),
            "DAIA_RUNTIME_ROOT": str(self.runtime),
            "DAIA_POLKIT_ACTIONS_DIRECTORY": str(self.polkit),
            "PATH": str(self.mock_bin) + os.pathsep + environment["PATH"],
            "DAIA_TEST_UNIT_STATE": (
                unit_state if unit_state is not None
                else ("enabled" if enabled else "static")
            ),
            "DAIA_TEST_ACTIVE_STATE": (
                active_state if active_state is not None
                else ("active" if active else "inactive")
            ),
            "DAIA_TEST_INSPECTION_ERROR": "1" if inspection_error else "0",
        })

        return subprocess.run(
            ["bash", "-c", script],
            env=environment,
            text=True,
            capture_output=True,
        )

    def test_accepts_valid_configuration_with_inactive_units(self):
        result = self.run_verification()
        self.assertEqual(
            result.returncode,
            0,
            f"Valid exporter configuration rejected: {result.stderr}",
        )

    def test_accepts_disabled_unit_state(self):
        result = self.run_verification(unit_state="disabled")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_masked_unit_state(self):
        result = self.run_verification(unit_state="masked")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unit-file state is unsafe", result.stderr)

    def test_rejects_unknown_unit_state(self):
        result = self.run_verification(unit_state="")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unit-file state is unsafe", result.stderr)

    def test_rejects_failed_active_state(self):
        result = self.run_verification(active_state="failed")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not inactive", result.stderr)

    def test_rejects_systemd_inspection_error(self):
        result = self.run_verification(inspection_error=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Cannot inspect exporter unit-file state", result.stderr)

    def test_rejects_missing_configuration(self):
        (self.systemd / "daia-model-export.socket").unlink()
        result = self.run_verification()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or unsafe", result.stderr)

    def test_rejects_symlink_configuration(self):
        destination = self.systemd / "daia-model-export.socket"
        destination.unlink()
        destination.symlink_to(
            self.runtime / "services/daia-model-export.socket"
        )
        result = self.run_verification()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or unsafe", result.stderr)

    def test_rejects_incorrect_permissions(self):
        (self.systemd / "daia-model-export.socket").chmod(0o666)
        result = self.run_verification()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ownership or mode invalid", result.stderr)

    def test_rejects_modified_contents(self):
        destination = self.systemd / "daia-model-export.socket"
        destination.write_text("tampered exporter configuration\n")
        result = self.run_verification()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("differs from packaged source", result.stderr)

    def test_rejects_enabled_exporter(self):
        result = self.run_verification(enabled=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unit-file state is unsafe", result.stderr)

    def test_rejects_active_exporter(self):
        result = self.run_verification(active=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not inactive", result.stderr)


if __name__ == "__main__":
    unittest.main()
