"""Headless regression tests for the DAIA PyQt6 Model Manager."""

import importlib.util
import os
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from PyQt6.QtCore import QProcess
from PyQt6.QtWidgets import QApplication, QMessageBox


ROOT = Path(__file__).resolve().parents[2]
MANAGER = ROOT / "runtime/daia-model-manager.py"

spec = importlib.util.spec_from_file_location("daia_model_manager_gui", MANAGER)
manager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(manager)

app = QApplication.instance() or QApplication([])


class ModelManagerGuiTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.work = Path(self.directory.name)
        self.bin = self.work / "bin"
        self.bin.mkdir()
        self.mode = self.work / "mode"
        self.arguments = self.work / "arguments"

        self.pkexec = self.bin / "pkexec"
        self.pkexec.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$@" > "$DAIA_TEST_ARGS"\n'
            'case "$(cat "$DAIA_TEST_MODE")" in\n'
            '  progress)\n'
            '    printf "DAIA_PROGRESS_TOTAL_"\n'
            '    sleep 0.08\n'
            '    printf "BYTES=1000\\n"\n'
            '    printf "DAIA_PROGRESS_BYTES=250\\n"\n'
            '    sleep 0.08\n'
            '    printf "DAIA_PROGRESS_BYTES=750\\n"\n'
            '    sleep 0.08\n'
            '    printf "DAIA_PROGRESS_PHASE=registering\\n"\n'
            '    sleep 0.08\n'
            '    exit 0 ;;\n'
            '  failure)\n'
            '    sleep 0.08\n'
            '    echo "Mock import failure" >&2\n'
            '    exit 1 ;;\n'
            '  cancelled) sleep 0.08; exit 126 ;;\n'
            '  *) exit 9 ;;\n'
            'esac\n'
        )
        self.pkexec.chmod(0o700)

        self.environment = patch.dict(
            os.environ,
            {
                "QT_QPA_PLATFORM": "offscreen",
                "PATH": str(self.bin) + os.pathsep + os.environ.get("PATH", ""),
                "DAIA_TEST_ARGS": str(self.arguments),
                "DAIA_TEST_MODE": str(self.mode),
            },
        )
        self.environment.start()
        self.addCleanup(self.environment.stop)

        self.mode.write_text("progress")
        self.tab = manager.ImportTab()
        self.tab.volumes.addItem("/dev/sdb1")
        self.tab.volumes.addItem("/dev/sdc1")
        self.tab.volumes.setCurrentRow(0)
        self.tab.models.addItem("source.gguf")
        self.tab.models.setCurrentRow(0)
        self.tab.model_name.setText("original-model")
        self.addCleanup(self.cleanup_tab)

    def cleanup_tab(self):
        for process in (
            self.tab.process,
            self.tab.model_process,
            self.tab.import_process,
            self.tab.recovery_process,
        ):
            if process.state() != QProcess.ProcessState.NotRunning:
                process.kill()
                process.waitForFinished(2000)
        self.tab.close()
        app.processEvents()

    def wait(self, condition, description):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            app.processEvents()
            if condition():
                app.processEvents()
                return
            time.sleep(0.01)
        self.fail("Timeout: " + description)

    def offer_recovery(self, name="original-model"):
        self.tab.failed_import_name = name
        self.tab.recovery_button.show()
        self.tab.recovery_button.setEnabled(True)

    def test_fragmented_progress_and_completion(self):
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")

        self.wait(
            lambda: self.tab.import_total_bytes == 1000,
            "fragmented total bytes",
        )
        self.wait(
            lambda: self.tab.import_copied_bytes >= 750,
            "copy progress",
        )
        self.assertEqual(self.tab.progress.value(), 75)

        self.wait(
            lambda: self.tab.status.text() == "Registering model with Ollama...",
            "registration phase",
        )
        self.assertEqual(self.tab.progress.maximum(), 0)

        self.wait(
            lambda: self.tab.import_process.state()
            == QProcess.ProcessState.NotRunning,
            "import completion",
        )
        self.assertEqual(
            self.tab.status.text(), "GGUF model imported successfully."
        )
        self.assertTrue(self.tab.progress.isHidden())

    def test_failed_import_offers_recovery(self):
        self.mode.write_text("failure")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")
        self.wait(
            lambda: self.tab.import_process.state()
            == QProcess.ProcessState.NotRunning
            and self.tab.scan_button.isEnabled(),
            "failed import",
        )
        self.assertEqual(self.tab.failed_import_name, "original-model")
        self.assertTrue(self.tab.recovery_button.isEnabled())
        self.assertIn("Mock import failure", self.tab.output.toPlainText())

    def test_authorization_cancellation_does_not_offer_recovery(self):
        self.mode.write_text("cancelled")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")
        self.wait(
            lambda: self.tab.import_process.state()
            == QProcess.ProcessState.NotRunning
            and self.tab.scan_button.isEnabled(),
            "cancelled import",
        )
        self.assertIsNone(self.tab.failed_import_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())

    def test_recovery_confirmation_and_original_identity(self):
        self.offer_recovery()
        self.tab.model_name.setText("edited-model")

        with patch.object(
            QMessageBox, "question",
            return_value=QMessageBox.StandardButton.No,
        ):
            self.tab.review_recovery()
        self.assertFalse(self.arguments.exists())

        with patch.object(
            QMessageBox, "question",
            return_value=QMessageBox.StandardButton.Yes,
        ):
            self.tab.review_recovery()

        self.wait(
            lambda: self.tab.recovery_process.state()
            == QProcess.ProcessState.NotRunning
            and self.tab.scan_button.isEnabled(),
            "recovery completion",
        )

        self.assertEqual(
            self.arguments.read_text().splitlines(),
            [
                "/usr/bin/daia-model-import",
                "resume-model",
                "original-model",
            ],
        )
        self.assertIsNone(self.tab.failed_import_name)
        self.assertTrue(self.tab.recovery_button.isHidden())

    def test_recovery_failure_preserves_retry(self):
        self.offer_recovery()
        self.mode.write_text("failure")
        self.tab.start_recovery("original-model")
        self.wait(
            lambda: self.tab.recovery_process.state()
            == QProcess.ProcessState.NotRunning
            and self.tab.scan_button.isEnabled(),
            "failed recovery",
        )
        self.assertEqual(self.tab.status.text(), "Model recovery failed.")
        self.assertEqual(self.tab.failed_import_name, "original-model")
        self.assertTrue(self.tab.recovery_button.isEnabled())

    def test_volume_change_invalidates_recovery(self):
        self.offer_recovery()
        self.tab.volumes.setCurrentRow(1)
        self.assertIsNone(self.tab.failed_import_name)
        self.assertTrue(self.tab.recovery_button.isHidden())
        self.assertFalse(self.tab.recovery_button.isEnabled())

    def test_import_failed_to_start_cleans_progress(self):
        os.environ["PATH"] = str(self.work / "nonexistent")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")
        self.wait(
            lambda: self.tab.status.text()
            == "Unable to start import authorization.",
            "FailedToStart",
        )
        self.assertTrue(self.tab.progress.isHidden())
        self.assertIsNone(self.tab.failed_import_name)
        self.assertTrue(self.tab.scan_button.isEnabled())


if __name__ == "__main__":
    unittest.main()
