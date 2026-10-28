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


class ModelImporterProgressLifecycleTests(unittest.TestCase):
    def test_successful_import_closes_progress_before_success_dialog(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "events.log"

            original = IMPORTER.read_text()
            helper_path = str(root / "mock-helper")

            self.assertIn("/usr/bin/daia-model-import", original)

            importer = root / "importer.sh"
            importer.write_text(
                original.replace(
                    "/usr/bin/daia-model-import",
                    helper_path,
                )
            )

            scripts = {
                "daia": """#!/bin/bash
printf '/dev/test-volume\\n'
""",
                "pkexec": """#!/bin/bash
exec "$@"
""",
                "kdialog": """#!/bin/bash
case " $* " in
    *" --menu "*"source volume"*)
        printf '/dev/test-volume\\n'
        ;;
    *" --menu "*)
        printf 'model.gguf\\n'
        ;;
    *" --inputbox "*)
        printf 'test-model\\n'
        ;;
    *" --progressbar "*)
        printf 'OPEN\\n' >> "$DAIA_TEST_LOG"
        printf 'org.kde.kdialog-test /ProgressDialog\\n'
        ;;
    *" --msgbox "*)
        printf 'SUCCESS_DIALOG\\n' >> "$DAIA_TEST_LOG"
        ;;
    *" --yesno "*)
        exit 0
        ;;
esac
""",
                "qdbus6": """#!/bin/bash
case "$3" in
    org.kde.kdialog.ProgressDialog.showCancelButton)
        printf 'DISABLE_CANCEL\\n' >> "$DAIA_TEST_LOG"
        ;;
    org.kde.kdialog.ProgressDialog.setLabelText)
        printf 'UPDATE:%s\\n' "$4" >> "$DAIA_TEST_LOG"
        ;;
    org.kde.kdialog.ProgressDialog.close)
        printf 'CLOSE\\n' >> "$DAIA_TEST_LOG"
        ;;
esac
""",
            }

            for name, content in scripts.items():
                executable = bin_dir / name
                executable.write_text(content)
                executable.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text("""#!/bin/bash
if [[ "$1" == "list-models" ]]; then
    printf 'model.gguf\\n'
    exit 0
fi
printf 'DAIA_PROGRESS_BYTES=65536\\n'
printf 'DAIA_PROGRESS_PHASE=registering\\n'
""")
            helper.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(bin_dir / "daia")
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

            events = log.read_text()

            self.assertIn("OPEN", events)
            self.assertIn("DISABLE_CANCEL", events)
            self.assertIn("Copying GGUF model: model.gguf", events)
            self.assertIn("Transferred: 65536 bytes", events)
            self.assertIn("Registering model with Ollama: test-model", events)
            self.assertIn("CLOSE", events)
            self.assertIn("SUCCESS_DIALOG", events)

            self.assertLess(
                events.index("CLOSE"),
                events.index("SUCCESS_DIALOG"),
            )


    def test_failed_import_closes_progress_before_recovery_dialog(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "events.log"

            original = IMPORTER.read_text()
            helper_path = str(root / "mock-helper")

            self.assertIn("/usr/bin/daia-model-import", original)

            importer = root / "importer.sh"
            importer.write_text(
                original.replace(
                    "/usr/bin/daia-model-import",
                    helper_path,
                )
            )

            scripts = {
                "daia": """#!/bin/bash
printf '/dev/test-volume\\n'
""",
                "pkexec": """#!/bin/bash
exec "$@"
""",
                "kdialog": """#!/bin/bash
case " $* " in
    *" --menu "*"source volume"*)
        printf '/dev/test-volume\\n'
        ;;
    *" --menu "*)
        printf 'model.gguf\\n'
        ;;
    *" --inputbox "*)
        printf 'test-model\\n'
        ;;
    *" --progressbar "*)
        printf 'OPEN\\n' >> "$DAIA_TEST_LOG"
        printf 'org.kde.kdialog-test /ProgressDialog\\n'
        ;;
    *" --yesno "*"Attempt recovery for:"*)
        printf 'RECOVERY_DIALOG\\n' >> "$DAIA_TEST_LOG"
        exit 1
        ;;
    *" --yesno "*)
        exit 0
        ;;
esac
""",
                "qdbus6": """#!/bin/bash
case "$3" in
    org.kde.kdialog.ProgressDialog.showCancelButton)
        printf 'DISABLE_CANCEL\\n' >> "$DAIA_TEST_LOG"
        ;;
    org.kde.kdialog.ProgressDialog.setLabelText)
        printf 'UPDATE:%s\\n' "$4" >> "$DAIA_TEST_LOG"
        ;;
    org.kde.kdialog.ProgressDialog.close)
        printf 'CLOSE\\n' >> "$DAIA_TEST_LOG"
        ;;
esac
""",
            }

            for name, content in scripts.items():
                executable = bin_dir / name
                executable.write_text(content)
                executable.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text("""#!/bin/bash
if [[ "$1" == "list-models" ]]; then
    printf 'model.gguf\\n'
    exit 0
fi
printf 'DAIA_PROGRESS_BYTES=65536\\n'
printf 'simulated import failure\\n' >&2
exit 1
""")
            helper.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(bin_dir / "daia")
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

            events = log.read_text()

            self.assertIn("OPEN", events)
            self.assertIn("DISABLE_CANCEL", events)
            self.assertIn("Transferred: 65536 bytes", events)
            self.assertIn("CLOSE", events)
            self.assertIn("RECOVERY_DIALOG", events)

            self.assertEqual(events.count("CLOSE"), 1)
            self.assertLess(
                events.index("CLOSE"),
                events.index("RECOVERY_DIALOG"),
            )


class ModelImporterFastCompletionTests(unittest.TestCase):
    def test_fast_completion_drains_all_progress_events(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "events.log"

            original = IMPORTER.read_text()
            helper_path = str(root / "mock-helper")

            if original.count("/usr/bin/daia-model-import") < 1:
                self.fail("Privileged helper path missing")

            importer = root / "importer.sh"
            importer.write_text(
                original.replace(
                    "/usr/bin/daia-model-import",
                    helper_path,
                )
            )

            scripts = {
                "daia": """#!/bin/bash
printf '/dev/test-volume\\n'
""",
                "pkexec": """#!/bin/bash
exec "$@"
""",
                "kdialog": """#!/bin/bash
case " $* " in
    *" --menu "*"source volume"*)
        printf '/dev/test-volume\\n'
        ;;
    *" --menu "*)
        printf 'model.gguf\\n'
        ;;
    *" --inputbox "*)
        printf 'test-model\\n'
        ;;
    *" --progressbar "*)
        printf 'OPEN\\n' >> "$DAIA_TEST_LOG"
        printf 'org.kde.kdialog-test /ProgressDialog\\n'
        ;;
    *" --msgbox "*)
        printf 'SUCCESS_DIALOG\\n' >> "$DAIA_TEST_LOG"
        ;;
    *" --yesno "*)
        exit 0
        ;;
esac
""",
                "qdbus6": """#!/bin/bash
case "$3" in
    org.kde.kdialog.ProgressDialog.setLabelText)
        printf 'LABEL:%s\\n' "$4" >> "$DAIA_TEST_LOG"
        ;;
    org.freedesktop.DBus.Properties.Set)
        printf 'PERCENT:%s\\n' "$6" >> "$DAIA_TEST_LOG"
        ;;
    org.kde.kdialog.ProgressDialog.close)
        printf 'CLOSE\\n' >> "$DAIA_TEST_LOG"
        ;;
esac
""",
            }

            for name, content in scripts.items():
                executable = bin_dir / name
                executable.write_text(content)
                executable.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text("""#!/bin/bash
if [[ "$1" == "list-models" ]]; then
    printf 'model.gguf\\n'
    exit 0
fi
printf 'DAIA_PROGRESS_TOTAL_BYTES=150000\\n'
printf 'DAIA_PROGRESS_BYTES=65536\\n'
printf 'DAIA_PROGRESS_BYTES=150000\\n'
printf 'DAIA_PROGRESS_PHASE=registering\\n'
""")
            helper.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(bin_dir / "daia")
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

            events = log.read_text()
            for expected in (
                "OPEN",
                "Progress: 43%",
                "Progress: 100%",
                "Registering model with Ollama: test-model",
                "CLOSE",
                "SUCCESS_DIALOG",
            ):
                self.assertIn(expected, events)

            self.assertLess(
                events.index("Progress: 100%"),
                events.index("Registering model with Ollama: test-model"),
            )
            self.assertLess(
                events.index("Registering model with Ollama: test-model"),
                events.index("CLOSE"),
            )
            self.assertLess(
                events.index("CLOSE"),
                events.index("SUCCESS_DIALOG"),
            )


class ModelImporterSplitEventTests(unittest.TestCase):
    def test_split_progress_event_is_reassembled(self):
        import os
        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            log = root / "events.log"

            original = IMPORTER.read_text()
            helper_path = str(root / "mock-helper")

            if original.count("/usr/bin/daia-model-import") < 1:
                self.fail("Privileged helper path missing")

            importer = root / "importer.sh"
            importer.write_text(
                original.replace(
                    "/usr/bin/daia-model-import",
                    helper_path,
                )
            )

            scripts = {
                "daia": """#!/bin/bash
printf '/dev/test-volume\\n'
""",
                "pkexec": """#!/bin/bash
exec "$@"
""",
                "kdialog": """#!/bin/bash
case " $* " in
    *" --menu "*"source volume"*)
        printf '/dev/test-volume\\n'
        ;;
    *" --menu "*)
        printf 'model.gguf\\n'
        ;;
    *" --inputbox "*)
        printf 'test-model\\n'
        ;;
    *" --progressbar "*)
        printf 'OPEN\\n' >> "$DAIA_TEST_LOG"
        printf 'org.kde.kdialog-test /ProgressDialog\\n'
        ;;
    *" --msgbox "*)
        printf 'SUCCESS_DIALOG\\n' >> "$DAIA_TEST_LOG"
        ;;
    *" --yesno "*)
        exit 0
        ;;
esac
""",
                "qdbus6": """#!/bin/bash
case "$3" in
    org.kde.kdialog.ProgressDialog.setLabelText)
        printf 'LABEL:%s\\n' "$4" >> "$DAIA_TEST_LOG"
        ;;
    org.freedesktop.DBus.Properties.Set)
        printf 'PERCENT:%s\\n' "$6" >> "$DAIA_TEST_LOG"
        ;;
    org.kde.kdialog.ProgressDialog.close)
        printf 'CLOSE\\n' >> "$DAIA_TEST_LOG"
        ;;
esac
""",
            }

            for name, content in scripts.items():
                executable = bin_dir / name
                executable.write_text(content)
                executable.chmod(0o700)

            helper = root / "mock-helper"
            helper.write_text("""#!/bin/bash
if [[ "$1" == "list-models" ]]; then
    printf 'model.gguf\\n'
    exit 0
fi
printf 'DAIA_PROGRESS_TOTAL_BYTES=150000\\n'
printf 'DAIA_PROGRESS_BYTES=65'
sleep 0.4
printf '536\\n'
printf 'DAIA_PROGRESS_BYTES=150000\\n'
printf 'DAIA_PROGRESS_PHASE=registering\\n'
""")
            helper.chmod(0o700)

            env = os.environ.copy()
            env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
            env["DAIA_COMMAND"] = str(bin_dir / "daia")
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

            events = log.read_text()
            for expected in (
                "OPEN",
                "Progress: 43%",
                "Progress: 100%",
                "Registering model with Ollama: test-model",
                "CLOSE",
                "SUCCESS_DIALOG",
            ):
                self.assertIn(expected, events)

            self.assertLess(
                events.index("Progress: 100%"),
                events.index("Registering model with Ollama: test-model"),
            )
            self.assertLess(
                events.index("Registering model with Ollama: test-model"),
                events.index("CLOSE"),
            )
            self.assertLess(
                events.index("CLOSE"),
                events.index("SUCCESS_DIALOG"),
            )


class ModelImporterProgressTests(unittest.TestCase):
    def test_percentage_updates_and_boundary_values(self):
        import os
        import subprocess
        import tempfile

        source = IMPORTER.read_text()
        start = source.index("show_import_progress_event() {")
        end = source.index("\nrun_import_with_progress() {", start)
        function = source[start:end]

        with tempfile.TemporaryDirectory() as directory:
            updates = Path(directory) / "updates.txt"
            values = Path(directory) / "values.txt"

            script = """
selected_model="model.gguf"
model_name="test-model"
progress_service="org.kde.test"
progress_object="/ProgressDialog"
import_total_bytes=""
update_progress_dialog() {
    printf '%s\\n' "$1" >> "$DAIA_TEST_UPDATES"
}
qdbus6() {
    printf '%s\\n' "${@: -1}" >> "$DAIA_TEST_VALUES"
}
""" + function + """
show_import_progress_event "DAIA_PROGRESS_TOTAL_BYTES=200"
show_import_progress_event "DAIA_PROGRESS_BYTES=100"
show_import_progress_event "DAIA_PROGRESS_BYTES=200"
show_import_progress_event "DAIA_PROGRESS_BYTES=300"
show_import_progress_event "DAIA_PROGRESS_TOTAL_BYTES=0"
show_import_progress_event "DAIA_PROGRESS_BYTES=50"
"""

            env = os.environ.copy()
            env["DAIA_TEST_UPDATES"] = str(updates)
            env["DAIA_TEST_VALUES"] = str(values)

            result = subprocess.run(
                ["bash", "-c", script],
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)

            labels = updates.read_text()
            self.assertIn("Progress: 50%", labels)
            self.assertIn("Progress: 100%", labels)
            self.assertIn("Transferred: 300 / 200 bytes", labels)
            self.assertIn("Transferred: 50 bytes", labels)

            self.assertEqual(
                values.read_text().splitlines(),
                ["50", "100", "100"],
            )

    def test_progress_event_parser_updates_expected_labels(self):
        import subprocess

        source = IMPORTER.read_text()
        start = source.index("show_import_progress_event() {")
        end = source.index("\nrun_import_with_progress() {", start)
        function = source[start:end]

        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            updates = Path(directory) / "updates.txt"

            script = """
selected_model="model.gguf"
model_name="test-model"
update_progress_dialog() {
    printf '%s\\n' "$1" >> "$DAIA_TEST_UPDATES"
}
""" + function + """
show_import_progress_event "DAIA_PROGRESS_BYTES=65536"
show_import_progress_event "DAIA_PROGRESS_PHASE=registering"
"""

            import os

            env = os.environ.copy()
            env["DAIA_TEST_UPDATES"] = str(updates)

            result = subprocess.run(
                ["bash", "-c", script],
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)

            labels = updates.read_text()
            self.assertIn("Copying GGUF model: model.gguf", labels)
            self.assertIn("Transferred: 65536 bytes", labels)
            self.assertIn(
                "Registering model with Ollama: test-model",
                labels,
            )

    def test_progress_event_parser_ignores_malformed_events(self):
        import subprocess

        source = IMPORTER.read_text()
        start = source.index("show_import_progress_event() {")
        end = source.index("\nrun_import_with_progress() {", start)
        function = source[start:end]

        script = """
selected_model="model.gguf"
model_name="test-model"
update_progress_dialog() {
    printf 'UNEXPECTED UPDATE\\n'
}
""" + function + """
show_import_progress_event "DAIA_PROGRESS_BYTES=invalid"
show_import_progress_event "DAIA_PROGRESS_PHASE=unknown"
show_import_progress_event "unrelated output"
"""

        result = subprocess.run(
            ["bash", "-c", script],
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
