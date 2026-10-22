#!/usr/bin/env python3
"""DAIA first-boot provisioning regression tests."""

from pathlib import Path
import os
import subprocess
import tempfile
import unittest


SOURCE = Path(__file__).resolve().parents[1] / "firstboot.sh"


class FirstbootProvisioningTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.script = SOURCE.read_text()

    def test_provisions_state_directory_with_expected_permissions(self):
        self.assertIn(
            'install -d -o root -g root -m 0755 "$state_directory"',
            self.script,
        )

    def test_provisions_model_content_directory(self):
        self.assertIn(
            'install -d -o root -g root -m 0755 "$state_directory/content"',
            self.script,
        )

    def test_provisioning_precedes_runtime(self):
        main = self.script.split("\nmain()\n", 1)[1]
        main = main.split('\nmain "$@"', 1)[0]
        self.assertLess(
            main.index('prepare_model_directory "$DAIA_STATE_DIRECTORY"'),
            main.index("    run_runtime"),
        )

    def test_executes_provisioning_in_temporary_directory(self):
        start = self.script.index("\nprepare_model_directory()\n")
        end = self.script.index("\nmark_firstboot_complete()\n", start)
        function = self.script[start:end]

        with tempfile.TemporaryDirectory(prefix="daia-firstboot-") as tmp:
            base = Path(tmp)
            state = base / "state"
            stub_dir = base / "bin"
            stub_dir.mkdir()

            stub = stub_dir / "install"
            stub.write_text(
                '#!/bin/bash\n'
                'set -euo pipefail\n'
                '[[ "$#" == 8 ]] || exit 41\n'
                '[[ "$1" == "-d" && "$2" == "-o" ]] || exit 42\n'
                '[[ "$3" == "root" && "$4" == "-g" ]] || exit 43\n'
                '[[ "$5" == "root" && "$6" == "-m" ]] || exit 44\n'
                '[[ "$7" == "0755" ]] || exit 45\n'
                'mkdir -p -- "$8"\n'
                'chmod 0755 -- "$8"\n'
            )
            stub.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(stub_dir) + os.pathsep + env["PATH"]

            command = (
                "set -euo pipefail\n"
                + function
                + '\nprepare_model_directory "$1"\n'
            )

            subprocess.run(
                ["bash", "-c", command, "daia-test", str(state)],
                env=env,
                check=True,
            )

            for directory in (state, state / "content"):
                self.assertTrue(directory.is_dir())
                self.assertEqual(
                    directory.stat().st_mode & 0o777,
                    0o755,
                )


if __name__ == "__main__":
    unittest.main()
