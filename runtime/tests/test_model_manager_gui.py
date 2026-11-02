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



class ModelExportTabTests(unittest.TestCase):
    """Verify export selection never bypasses the disabled execution boundary."""

    def setUp(self):
        self.tab = manager.ExportTab()
        self.addCleanup(self.tab.close)

    def finish_inventory(self, payload, exit_code=0):
        with patch.object(
            self.tab.process,
            "readAllStandardOutput",
            return_value=payload,
        ):
            self.tab._finished(
                exit_code,
                QProcess.ExitStatus.NormalExit,
            )

    def test_export_remains_disabled(self):
        self.assertFalse(self.tab.export_button.isEnabled())
        self.tab.models.addItem("example:latest")
        self.tab.models.setCurrentRow(0)
        self.tab.destination.setText("/tmp/export-target")
        self.assertFalse(self.tab.export_button.isEnabled())

    def test_installed_models_display_without_authorization(self):
        self.finish_inventory(
            b'[{"name":"example:latest"},{"name":"second:q4_k_m"}]'
        )
        self.assertEqual(self.tab.models.count(), 2)
        self.assertEqual(self.tab.models.item(0).text(), "example:latest")
        self.assertFalse(self.tab.export_button.isEnabled())

    def test_malformed_inventory_rejected(self):
        self.finish_inventory(b'{"invalid":true}')
        self.assertEqual(self.tab.models.count(), 0)
        self.assertIn("Invalid", self.tab.status.text())
        self.assertFalse(self.tab.export_button.isEnabled())

    def test_inventory_failure_does_not_enable_export(self):
        self.finish_inventory(b"", exit_code=1)
        self.assertIn("Unable", self.tab.status.text())
        self.assertFalse(self.tab.export_button.isEnabled())


class ModelManagerGuiTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.work = Path(self.directory.name)
        self.bin = self.work / "bin"
        self.bin.mkdir()
        self.mode = self.work / "mode"
        self.status_mode = self.work / "status-mode"
        self.arguments = self.work / "arguments"

        self.pkexec = self.bin / "pkexec"
        self.pkexec.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$@" > "$DAIA_TEST_ARGS"\n'
            'if [ "$2" = "recovery-status" ]; then\n'
            '  case "$(cat "$DAIA_TEST_STATUS_MODE")" in\n'
            '    recorded) echo "RECORDED"; exit 0 ;;\n'
            '    delayed_recorded) sleep 0.3; echo "RECORDED"; exit 0 ;;\n'
            '    not_recorded) echo "NOT_RECORDED"; exit 0 ;;\n'
            '    malformed) echo "UNEXPECTED"; exit 0 ;;\n'
            '    cancelled) exit 126 ;;\n'
            '    failure) echo "State read failed" >&2; exit 1 ;;\n'
            '    *) exit 9 ;;\n'
            '  esac\n'
            'fi\n'
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
            '  recorded) echo "RECORDED"; exit 0 ;;\n'
            '  not_recorded) echo "NOT_RECORDED"; exit 0 ;;\n'
            '  malformed) echo "UNEXPECTED"; exit 0 ;;\n'
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
                "DAIA_TEST_STATUS_MODE": str(self.status_mode),
            },
        )
        self.environment.start()
        self.addCleanup(self.environment.stop)

        self.mode.write_text("progress")
        self.status_mode.write_text("not_recorded")
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
            self.tab.recovery_status_process,
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
        self.tab.active_import_name = name
        self.tab.active_import_filename = "source.gguf"
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

    def test_failed_import_does_not_offer_unverified_recovery(self):
        self.mode.write_text("failure")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")
        self.wait(
            lambda: self.tab.import_process.state()
            == QProcess.ProcessState.NotRunning
            and self.tab.scan_button.isEnabled(),
            "failed import",
        )
        self.assertIsNone(self.tab.failed_import_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())
        self.assertTrue(self.tab.recovery_button.isHidden())
        self.assertIn("Mock import failure", self.tab.output.toPlainText())

    def test_recorded_model_enables_verified_recovery(self):
        self.mode.write_text("failure")
        self.status_mode.write_text("recorded")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")

        self.wait(
            lambda: self.tab.recovery_status_process.state()
            == QProcess.ProcessState.NotRunning
            and self.tab.recovery_button.isEnabled(),
            "verified recovery offer",
        )
        self.assertEqual(self.tab.failed_import_name, "original-model")
        self.assertEqual(
            self.arguments.read_text().splitlines(),
            [
                "/usr/bin/daia-model-import",
                "recovery-status",
                "original-model",
                "source.gguf",
            ],
        )

    def test_model_name_change_invalidates_pending_recovery_status(self):
        self.mode.write_text("failure")
        self.status_mode.write_text("delayed_recorded")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")

        self.wait(
            lambda: (
                self.tab.recovery_status_process.state()
                == QProcess.ProcessState.Running
            ),
            "delayed recovery status started",
        )

        self.tab.model_name.setText("different-model")
        self.assertIsNone(self.tab.pending_recovery_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())

        self.wait(
            lambda: (
                self.tab.recovery_status_process.state()
                == QProcess.ProcessState.NotRunning
            ),
            "delayed recovery status completed",
        )

        self.assertIsNone(self.tab.failed_import_name)
        self.assertIsNone(self.tab.pending_recovery_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())
        self.assertTrue(self.tab.recovery_button.isHidden())

    def test_gguf_selection_change_invalidates_pending_recovery_status(self):
        self.mode.write_text("failure")
        self.status_mode.write_text("delayed_recorded")
        self.tab.models.addItem("different.gguf")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")

        self.wait(
            lambda: self.tab.recovery_status_process.state()
            == QProcess.ProcessState.Running,
            "delayed recovery status started",
        )

        self.tab.models.setCurrentRow(1)
        self.assertIsNone(self.tab.pending_recovery_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())

        self.wait(
            lambda: self.tab.recovery_status_process.state()
            == QProcess.ProcessState.NotRunning,
            "delayed recovery status completed",
        )

        self.assertEqual(self.tab.models.currentItem().text(), "different.gguf")
        self.assertIsNone(self.tab.failed_import_name)
        self.assertIsNone(self.tab.pending_recovery_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())
        self.assertTrue(self.tab.recovery_button.isHidden())

    def test_pending_recovery_status_blocks_overlapping_operations(self):
        self.mode.write_text("failure")
        self.status_mode.write_text("delayed_recorded")
        self.tab.start_import("/dev/sdb1", "source.gguf", "original-model")

        self.wait(
            lambda: (
                self.tab.recovery_status_process.state()
                == QProcess.ProcessState.Running
            ),
            "recovery-status query running",
        )

        self.assertFalse(self.tab.import_button.isEnabled())

        self.tab.start_import("/dev/sdb1", "source.gguf", "other-model")
        self.tab.start_recovery("original-model")
        self.tab.find_models()
        self.tab.scan()

        self.assertEqual(
            self.tab.recovery_status_process.state(),
            QProcess.ProcessState.Running,
        )
        self.assertEqual(
            self.tab.import_process.state(),
            QProcess.ProcessState.NotRunning,
        )
        self.assertEqual(
            self.tab.recovery_process.state(),
            QProcess.ProcessState.NotRunning,
        )
        self.assertEqual(
            self.tab.model_process.state(),
            QProcess.ProcessState.NotRunning,
        )
        self.assertEqual(
            self.tab.process.state(),
            QProcess.ProcessState.NotRunning,
        )

        self.wait(
            lambda: (
                self.tab.recovery_status_process.state()
                == QProcess.ProcessState.NotRunning
                and self.tab.import_button.isEnabled()
            ),
            "import button restored after recovery-status completion",
        )

        self.assertTrue(self.tab.recovery_button.isEnabled())
        self.assertEqual(self.tab.failed_import_name, "original-model")

    def test_negative_recovery_status_never_offers_recovery(self):
        for status_mode in (
            "not_recorded",
            "malformed",
            "failure",
            "cancelled",
        ):
            with self.subTest(status_mode=status_mode):
                self.mode.write_text("failure")
                self.status_mode.write_text(status_mode)
                self.tab.start_import(
                    "/dev/sdb1", "source.gguf", "original-model"
                )

                self.wait(
                    lambda: (
                        self.tab.recovery_status_process.state()
                        == QProcess.ProcessState.Running
                    ),
                    "negative recovery-status query started",
                )

                self.wait(
                    lambda: (
                        self.tab.recovery_status_process.state()
                        == QProcess.ProcessState.NotRunning
                        and self.tab.pending_recovery_name is None
                    ),
                    "negative recovery-status query finished",
                )

                self.assertIsNone(self.tab.failed_import_name)
                self.assertFalse(self.tab.recovery_button.isEnabled())
                self.assertTrue(self.tab.recovery_button.isHidden())
                self.assertTrue(self.tab.import_button.isEnabled())

    def test_recovery_status_failed_to_start_restores_import(self):
        self.tab.active_import_name = "original-model"
        self.tab.active_import_filename = "source.gguf"

        # Make pkexec unavailable before starting the actual QProcess.
        os.environ["PATH"] = str(self.work / "nonexistent")

        self.tab._start_recovery_status_check("original-model")

        self.wait(
            lambda: (
                self.tab.recovery_status_process.state()
                == QProcess.ProcessState.NotRunning
                and self.tab.pending_recovery_name is None
                and self.tab.import_button.isEnabled()
            ),
            "recovery-status FailedToStart cleanup",
        )

        self.assertEqual(
            self.tab.recovery_status_process.error(),
            QProcess.ProcessError.FailedToStart,
        )
        self.assertIsNone(self.tab.failed_import_name)
        self.assertIsNone(self.tab.pending_recovery_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())
        self.assertTrue(self.tab.recovery_button.isHidden())
        self.assertTrue(self.tab.import_button.isEnabled())
        self.assertFalse(self.arguments.exists())

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

        self.assertIsNone(self.tab.failed_import_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())

        self.tab.model_name.setText("original-model")
        self.offer_recovery()

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

    def test_recovery_rejects_different_selected_gguf(self):
        self.offer_recovery()
        self.tab.models.addItem("different.gguf")
        self.tab.models.setCurrentRow(1)

        # Simulate a stale recovery offer to test the execution guard
        # independently of the selection-change callback.
        self.tab.failed_import_name = "original-model"
        self.tab.recovery_button.setEnabled(True)
        self.tab.recovery_button.show()

        self.tab.start_recovery("original-model")

        self.assertEqual(
            self.tab.recovery_process.state(),
            QProcess.ProcessState.NotRunning,
        )
        self.assertFalse(self.arguments.exists())
        self.assertEqual(
            self.tab.models.currentItem().text(),
            "different.gguf",
        )

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

    def test_recovery_authorization_cancellation_disables_retry(self):
        self.offer_recovery()
        self.mode.write_text("cancelled")
        self.tab.start_recovery("original-model")

        self.wait(
            lambda: self.tab.recovery_process.state()
            == QProcess.ProcessState.NotRunning
            and self.tab.status.text() == "Model recovery failed.",
            "cancelled recovery",
        )

        self.assertIsNone(self.tab.failed_import_name)
        self.assertIsNone(self.tab.pending_recovery_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())
        self.assertTrue(self.tab.recovery_button.isHidden())

        previous_arguments = self.arguments.read_text()
        self.tab.review_recovery()
        self.assertEqual(self.arguments.read_text(), previous_arguments)
        self.assertEqual(
            self.tab.recovery_process.state(),
            QProcess.ProcessState.NotRunning,
        )

    def test_recovery_failed_to_start_clears_eligibility(self):
        self.offer_recovery()
        os.environ["PATH"] = str(self.work / "nonexistent")
        self.tab.start_recovery("original-model")

        self.wait(
            lambda: self.tab.status.text()
            == "Unable to start recovery authorization.",
            "recovery authorization FailedToStart",
        )

        self.assertIsNone(self.tab.failed_import_name)
        self.assertIsNone(self.tab.pending_recovery_name)
        self.assertFalse(self.tab.recovery_button.isEnabled())
        self.assertTrue(self.tab.recovery_button.isHidden())
        self.assertTrue(self.tab.scan_button.isEnabled())
        self.assertTrue(self.tab.volumes.isEnabled())
        self.assertTrue(self.tab.models.isEnabled())
        self.assertTrue(self.tab.model_name.isEnabled())
        self.assertEqual(
            self.tab.recovery_process.state(),
            QProcess.ProcessState.NotRunning,
        )
        self.assertFalse(self.arguments.exists())

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
