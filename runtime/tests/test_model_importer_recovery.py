#!/usr/bin/env python3
"""Regression checks for the DAIA graphical model recovery entry point."""

import unittest
from pathlib import Path


IMPORTER = Path(__file__).resolve().parents[1] / "daia-model-importer.sh"


class ModelImporterRecoveryTests(unittest.TestCase):
    def test_recovery_requires_explicit_privileged_command(self):
        source = IMPORTER.read_text()

        self.assertIn("resume-model", source)
        self.assertIn('resume-model "$model_name"', source)
        self.assertIn("pkexec /usr/bin/daia-model-import", source)

    def test_recovery_is_offered_after_import_failure(self):
        source = IMPORTER.read_text()

        failure = source.index('details="$(cat "$error_file")"')
        recovery = source.index('resume-model "$model_name"')

        self.assertLess(failure, recovery)
        self.assertIn("Attempt recovery for:", source)

    def test_success_message_is_separate_from_recovery(self):
        source = IMPORTER.read_text()

        self.assertIn("Model imported successfully.", source)
        self.assertIn("Model recovery completed successfully.", source)


class ModelImporterBehaviorTests(unittest.TestCase):
    def test_successful_import_never_invokes_recovery(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "calls.log"

            importer = root / "importer.sh"
            source = IMPORTER.read_text()

            original_helper = "/usr/bin/daia-model-import"
            mocked_helper = str(root / "mock-helper")

            self.assertIn(original_helper, source)
            importer.write_text(
                source.replace(original_helper, mocked_helper)
            )

            cli = bin_dir / "daia"
            cli.write_text(
                '#!/bin/sh\n'
                'printf "/dev/test-volume\n"\n'
            )
            cli.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text(
                '#!/bin/sh\n'
                'printf "%s\n" "$*" >> "$DAIA_TEST_LOG"\n'
                'if [ "$1" = "list-models" ]; then\n'
                '    printf "model.gguf\n"\n'
                'fi\n'
            )
            helper.chmod(0o700)

            pkexec = bin_dir / "pkexec"
            pkexec.write_text(
                '#!/bin/sh\n'
                'exec "$@"\n'
            )
            pkexec.chmod(0o700)

            kdialog = bin_dir / "kdialog"
            kdialog.write_text(
                '#!/bin/sh\n'
                'case " $* " in\n'
                '  *" --menu "*"source volume"*) printf "/dev/test-volume\n" ;;\n'
                '  *" --menu "*) printf "model.gguf\n" ;;\n'
                '  *" --inputbox "*) printf "recovery-model\n" ;;\n'
                '  *) exit 0 ;;\n'
                'esac\n'
            )
            kdialog.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(cli)
            env["DAIA_TEST_LOG"] = str(log)

            result = subprocess.run(
                ["bash", str(importer)],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)

            calls = log.read_text().splitlines()
            self.assertEqual(
                calls,
                [
                    "list-models /dev/test-volume",
                    "/dev/test-volume model.gguf recovery-model",
                ],
            )


    def test_failed_import_can_resume_model(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "calls.log"

            importer = root / "importer.sh"
            original = IMPORTER.read_text()
            self.assertIn("/usr/bin/daia-model-import", original)

            importer.write_text(
                original.replace(
                    "/usr/bin/daia-model-import",
                    str(root / "mock-helper"),
                )
            )

            cli = bin_dir / "daia"
            cli.write_text(
                '#!/bin/sh\n'
                'printf "/dev/test-volume\\n"\n'
            )
            cli.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text(
                '#!/bin/sh\n'
                'printf "%s\\n" "$*" >> "$DAIA_TEST_LOG"\n'
                'case "$1" in\n'
                '  list-models) printf "model.gguf\\n" ;;\n'
                '  /dev/test-volume) exit 1 ;;\n'
                '  resume-model) exit 0 ;;\n'
                '  *) exit 2 ;;\n'
                'esac\n'
            )
            helper.chmod(0o700)

            pkexec = bin_dir / "pkexec"
            pkexec.write_text('#!/bin/sh\nexec "$@"\n')
            pkexec.chmod(0o700)

            kdialog = bin_dir / "kdialog"
            kdialog.write_text(
                '#!/bin/sh\n'
                'case " $* " in\n'
                '  *" --menu "*"source volume"*) printf "/dev/test-volume\\n" ;;\n'
                '  *" --menu "*) printf "model.gguf\\n" ;;\n'
                '  *" --inputbox "*) printf "recovery-model\\n" ;;\n'
                '  *" --yesno "*) exit 0 ;;\n'
                '  *" --msgbox "*) exit 0 ;;\n'
                '  *) exit 2 ;;\n'
                'esac\n'
            )
            kdialog.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(cli)
            env["DAIA_TEST_LOG"] = str(log)

            result = subprocess.run(
                ["bash", str(importer)],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                log.read_text().splitlines(),
                [
                    "list-models /dev/test-volume",
                    "/dev/test-volume model.gguf recovery-model",
                    "resume-model recovery-model",
                ],
            )

    def test_failed_recovery_reports_error(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "calls.log"

            importer = root / "importer.sh"
            original = IMPORTER.read_text()
            self.assertIn("/usr/bin/daia-model-import", original)

            importer.write_text(
                original.replace(
                    "/usr/bin/daia-model-import",
                    str(root / "mock-helper"),
                )
            )

            cli = bin_dir / "daia"
            cli.write_text(
                '#!/bin/sh\n'
                'printf "/dev/test-volume\\n"\n'
            )
            cli.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text(
                '#!/bin/sh\n'
                'printf "%s\\n" "$*" >> "$DAIA_TEST_LOG"\n'
                'case "$1" in\n'
                '  list-models) printf "model.gguf\\n" ;;\n'
                '  /dev/test-volume) exit 1 ;;\n'
                '  resume-model) printf "simulated Ollama registration failure\\n" >&2; exit 1 ;;\n'
                '  *) exit 2 ;;\n'
                'esac\n'
            )
            helper.chmod(0o700)

            pkexec = bin_dir / "pkexec"
            pkexec.write_text('#!/bin/sh\nexec "$@"\n')
            pkexec.chmod(0o700)

            kdialog = bin_dir / "kdialog"
            kdialog.write_text(
                '#!/bin/sh\n'
                'case " $* " in\n'
                '  *" --menu "*"source volume"*) printf "/dev/test-volume\\n" ;;\n'
                '  *" --menu "*) printf "model.gguf\\n" ;;\n'
                '  *" --inputbox "*) printf "recovery-model\\n" ;;\n'
                '  *" --yesno "*) exit 0 ;;\n'
                '  *" --msgbox "*) exit 0 ;;\n'
                '  *) exit 2 ;;\n'
                'esac\n'
            )
            kdialog.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(cli)
            env["DAIA_TEST_LOG"] = str(log)

            result = subprocess.run(
                ["bash", str(importer)],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertEqual(
                log.read_text().splitlines(),
                [
                    "list-models /dev/test-volume",
                    "/dev/test-volume model.gguf recovery-model",
                    "resume-model recovery-model",
                ],
            )

    def test_declined_recovery_never_invokes_resume(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "calls.log"

            importer = root / "importer.sh"
            original = IMPORTER.read_text()
            self.assertIn("/usr/bin/daia-model-import", original)

            importer.write_text(
                original.replace(
                    "/usr/bin/daia-model-import",
                    str(root / "mock-helper"),
                )
            )

            cli = bin_dir / "daia"
            cli.write_text(
                '#!/bin/sh\n'
                'printf "/dev/test-volume\\n"\n'
            )
            cli.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text(
                '#!/bin/sh\n'
                'printf "%s\\n" "$*" >> "$DAIA_TEST_LOG"\n'
                'case "$1" in\n'
                '  list-models) printf "model.gguf\\n" ;;\n'
                '  /dev/test-volume) exit 1 ;;\n'
                '  resume-model) exit 0 ;;\n'
                '  *) exit 2 ;;\n'
                'esac\n'
            )
            helper.chmod(0o700)

            pkexec = bin_dir / "pkexec"
            pkexec.write_text('#!/bin/sh\nexec "$@"\n')
            pkexec.chmod(0o700)

            kdialog = bin_dir / "kdialog"
            kdialog.write_text(
                '#!/bin/sh\n'
                'case " $* " in\n'
                '  *" --menu "*"source volume"*) printf "/dev/test-volume\\n" ;;\n'
                '  *" --menu "*) printf "model.gguf\\n" ;;\n'
                '  *" --inputbox "*) printf "recovery-model\\n" ;;\n'
                '  *" --yesno "*"Attempt recovery for:"*) exit 1 ;;\n'
                '  *" --yesno "*) exit 0 ;;\n'
                '  *" --msgbox "*) exit 0 ;;\n'
                '  *) exit 2 ;;\n'
                'esac\n'
            )
            kdialog.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(cli)
            env["DAIA_TEST_LOG"] = str(log)

            result = subprocess.run(
                ["bash", str(importer)],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertEqual(
                log.read_text().splitlines(),
                [
                    "list-models /dev/test-volume",
                    "/dev/test-volume model.gguf recovery-model",
                ],
            )


if __name__ == "__main__":
    unittest.main()
