#!/usr/bin/env python3
"""DAIA graphical Model Manager."""

import json
import os
import re
import sys

from PyQt6.QtCore import QProcess
from PyQt6.QtWidgets import (
    QApplication,
    QLabel,
    QListWidget,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QProgressBar,
    QTabWidget,
    QTableWidget,
    QTableWidgetItem,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)


class InstalledTab(QWidget):
    """Read-only view of locally registered Ollama models."""

    def __init__(self, parent=None):
        super().__init__(parent)

        self.process = QProcess(self)
        self.process.finished.connect(self._finished)
        self.process.errorOccurred.connect(self._process_error)

        self.status = QLabel("Select Refresh to load installed Ollama models.")
        self.table = QTableWidget(0, 6)
        self.table.setHorizontalHeaderLabels(
            ["Model", "Size", "Parameters", "Quantization", "Family", "Modified"]
        )
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        self.output = QTextEdit()
        self.output.setReadOnly(True)
        self.output.hide()

        self.refresh_button = QPushButton("Refresh")
        self.refresh_button.clicked.connect(self.refresh)

        layout = QVBoxLayout(self)
        layout.addWidget(self.status)
        layout.addWidget(self.table)
        layout.addWidget(self.output)
        layout.addWidget(self.refresh_button)

    def refresh(self):
        if self.process.state() != QProcess.ProcessState.NotRunning:
            return

        self.output.clear()
        self.output.hide()
        self.table.setRowCount(0)
        self.status.setText("Loading installed Ollama models...")
        self.refresh_button.setEnabled(False)

        command = os.environ.get("DAIA_COMMAND", "/usr/bin/daia")
        self.process.start(command, ["model", "installed", "--json"])

    def _finished(self, exit_code, exit_status):
        stdout = bytes(self.process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        )
        stderr = bytes(self.process.readAllStandardError()).decode(
            "utf-8", errors="replace"
        )

        self.refresh_button.setEnabled(True)

        if (
            exit_status == QProcess.ExitStatus.NormalExit
            and exit_code == 0
        ):
            try:
                models = json.loads(stdout)
                if not isinstance(models, list):
                    raise ValueError("Inventory must be a JSON array")
                for model in models:
                    if not isinstance(model, dict):
                        raise ValueError("Invalid model entry")
                    details = model.get("details") or {}
                    if not isinstance(details, dict):
                        raise ValueError("Invalid model details")
                    if not isinstance(model.get("name"), str):
                        raise ValueError("Missing model name")
                    size = model.get("size_bytes")
                    if type(size) is not int or size < 0:
                        raise ValueError("Invalid model size")
                    row = self.table.rowCount()
                    self.table.insertRow(row)
                    values = [
                        model["name"],
                        f"{size / (1024 ** 3):.2f} GiB",
                        details.get("parameter_size") or "",
                        details.get("quantization_level") or "",
                        details.get("family") or "",
                        model.get("modified_at") or "",
                    ]
                    for column, value in enumerate(values):
                        self.table.setItem(
                            row, column, QTableWidgetItem(str(value))
                        )
                self.status.setText(
                    f"Installed Ollama models: {len(models)}"
                )
            except (ValueError, TypeError) as error:
                self.table.setRowCount(0)
                self.status.setText("Invalid installed-model inventory.")
                self.output.setPlainText(str(error))
                self.output.show()
        else:
            self.status.setText("Unable to load installed models.")
            self.output.setPlainText(stderr or stdout or "Unknown CLI error.")
            self.output.show()

    def _process_error(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self.status.setText("Unable to start DAIA CLI.")
            self.output.setPlainText(self.process.errorString())
            self.output.show()
            self.refresh_button.setEnabled(True)


class ImportTab(QWidget):
    """Read-only discovery of model source volumes."""

    def __init__(self, parent=None):
        super().__init__(parent)

        self.process = QProcess(self)
        self.process.finished.connect(self._finished)
        self.process.errorOccurred.connect(self._process_error)

        self.model_process = QProcess(self)
        self.model_process.finished.connect(self._models_finished)
        self.model_process.errorOccurred.connect(self._models_error)

        self.import_process = QProcess(self)
        self.import_process.finished.connect(self._import_finished)
        self.import_process.errorOccurred.connect(self._import_error)
        self.import_process.readyReadStandardOutput.connect(
            self._read_import_progress
        )

        self.import_output_buffer = ""
        self.import_total_bytes = None
        self.import_copied_bytes = 0
        self.failed_import_name = None
        self.active_import_name = None

        self.recovery_process = QProcess(self)
        self.recovery_process.finished.connect(self._recovery_finished)
        self.recovery_process.errorOccurred.connect(self._recovery_error)

        self.status = QLabel("Select Scan to discover model source volumes.")

        self.volumes = QListWidget()
        self.volumes.setSelectionMode(
            QListWidget.SelectionMode.SingleSelection
        )

        self.output = QTextEdit()
        self.output.setReadOnly(True)
        self.output.hide()

        self.scan_button = QPushButton("Scan source volumes")
        self.scan_button.clicked.connect(self.scan)

        self.models = QListWidget()
        self.models.setSelectionMode(
            QListWidget.SelectionMode.SingleSelection
        )

        self.models_button = QPushButton("Find GGUF models")
        self.models_button.setEnabled(False)
        self.models_button.clicked.connect(self.find_models)

        self.model_name = QLineEdit()
        self.model_name.setPlaceholderText("Ollama model name")
        self.model_name.setEnabled(False)
        self.model_name.textChanged.connect(self._update_import_button)

        self.import_button = QPushButton("Review GGUF import")
        self.import_button.setEnabled(False)
        self.import_button.clicked.connect(self.review_import)

        self.recovery_button = QPushButton("Retry Ollama registration")
        self.recovery_button.setEnabled(False)
        self.recovery_button.hide()
        self.recovery_button.clicked.connect(self.review_recovery)

        self.progress = QProgressBar()
        self.progress.setRange(0, 100)
        self.progress.setValue(0)
        self.progress.hide()

        self.models.currentItemChanged.connect(
            self._model_selection_changed
        )
        self.volumes.currentItemChanged.connect(
            self._volume_selection_changed
        )

        layout = QVBoxLayout(self)
        layout.addWidget(self.status)
        layout.addWidget(self.volumes)
        layout.addWidget(self.models)
        layout.addWidget(self.output)
        layout.addWidget(self.scan_button)
        layout.addWidget(self.models_button)
        layout.addWidget(self.model_name)
        layout.addWidget(self.import_button)
        layout.addWidget(self.recovery_button)
        layout.addWidget(self.progress)

    def _clear_recovery_offer(self):
        self.failed_import_name = None
        self.recovery_button.setEnabled(False)
        self.recovery_button.hide()

    def _model_selection_changed(self, current, previous):
        self.model_name.setEnabled(
            current is not None
            and self.process.state() == QProcess.ProcessState.NotRunning
            and self.model_process.state() == QProcess.ProcessState.NotRunning
            and self.import_process.state() == QProcess.ProcessState.NotRunning
            and self.recovery_process.state() == QProcess.ProcessState.NotRunning
        )
        self._update_import_button()

    def _update_import_button(self, *args):
        valid_name = re.fullmatch(
            r"[A-Za-z0-9][A-Za-z0-9._-]*",
            self.model_name.text(),
        ) is not None

        self.import_button.setEnabled(
            valid_name
            and self.volumes.currentItem() is not None
            and self.models.currentItem() is not None
            and self.process.state() == QProcess.ProcessState.NotRunning
            and self.model_process.state() == QProcess.ProcessState.NotRunning
            and self.import_process.state() == QProcess.ProcessState.NotRunning
            and self.recovery_process.state() == QProcess.ProcessState.NotRunning
        )

    def review_import(self):
        if not self.import_button.isEnabled():
            return

        device = self.volumes.currentItem()
        model = self.models.currentItem()
        name = self.model_name.text()

        if device is None or model is None:
            return

        confirmation = QMessageBox.question(
            self,
            "Review GGUF import",
            "Source: " + device.text()
            + "\nGGUF file: " + model.text()
            + "\nOllama name: " + name
            + "\n\nContinue with model import?",
            QMessageBox.StandardButton.Yes
            | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No,
        )

        if confirmation == QMessageBox.StandardButton.Yes:
            self.start_import(device.text(), model.text(), name)

    def start_import(self, device, filename, name):
        if (
            self.process.state() != QProcess.ProcessState.NotRunning
            or self.model_process.state() != QProcess.ProcessState.NotRunning
            or self.import_process.state() != QProcess.ProcessState.NotRunning
            or self.recovery_process.state() != QProcess.ProcessState.NotRunning
        ):
            return

        self._clear_recovery_offer()
        self.active_import_name = name
        self.output.clear()
        self.output.hide()
        self.import_output_buffer = ""
        self.import_total_bytes = None
        self.import_copied_bytes = 0
        self.progress.setRange(0, 0)
        self.progress.show()
        self.status.setText("Waiting for authorization to import GGUF...")
        self.scan_button.setEnabled(False)
        self.models_button.setEnabled(False)
        self.import_button.setEnabled(False)
        self.volumes.setEnabled(False)
        self.models.setEnabled(False)
        self.model_name.setEnabled(False)

        self.import_process.start(
            "pkexec",
            [
                "/usr/bin/daia-model-import",
                device,
                filename,
                name,
            ],
        )

    def _read_import_progress(self):
        chunk = bytes(self.import_process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        )
        self.import_output_buffer += chunk

        while "\n" in self.import_output_buffer:
            line, self.import_output_buffer = (
                self.import_output_buffer.split("\n", 1)
            )
            self._handle_import_progress_line(line.strip())

    def _handle_import_progress_line(self, line):
        if line.startswith("DAIA_PROGRESS_TOTAL_BYTES="):
            value = line.partition("=")[2]
            if value.isdecimal():
                self.import_total_bytes = int(value)
                self.progress.setRange(0, 100)
                self._update_copy_progress()

        elif line.startswith("DAIA_PROGRESS_BYTES="):
            value = line.partition("=")[2]
            if value.isdecimal():
                self.import_copied_bytes = int(value)
                self._update_copy_progress()

        elif line == "DAIA_PROGRESS_PHASE=registering":
            self.progress.setRange(0, 0)
            self.status.setText("Registering model with Ollama...")

    def _update_copy_progress(self):
        total = self.import_total_bytes

        if total is None or total <= 0:
            return

        percent = min(
            100,
            self.import_copied_bytes * 100 // total,
        )
        self.progress.setValue(percent)
        self.status.setText(f"Copying GGUF model: {percent}%")

    def _import_finished(self, exit_code, exit_status):
        self._read_import_progress()

        if self.import_output_buffer:
            self._handle_import_progress_line(
                self.import_output_buffer.strip()
            )
            self.import_output_buffer = ""

        stdout = ""
        stderr = bytes(self.import_process.readAllStandardError()).decode(
            "utf-8", errors="replace"
        )

        self.progress.hide()
        self.scan_button.setEnabled(True)
        self.volumes.setEnabled(True)
        self.models.setEnabled(True)
        self.models_button.setEnabled(
            self.volumes.currentItem() is not None
        )
        self.model_name.setEnabled(
            self.models.currentItem() is not None
        )
        self._update_import_button()

        if (
            exit_status == QProcess.ExitStatus.NormalExit
            and exit_code == 0
        ):
            self.failed_import_name = None
            self.status.setText("GGUF model imported successfully.")
        else:
            self.status.setText("GGUF model import failed.")
            if (
                exit_status == QProcess.ExitStatus.NormalExit
                and exit_code not in (126, 127)
                and self.active_import_name is not None
            ):
                self.failed_import_name = self.active_import_name
                self.recovery_button.setEnabled(True)
                self.recovery_button.show()
            self.output.setPlainText(
                stderr or stdout or
                "Authorization was cancelled or the import failed."
            )
            self.output.show()

    def _import_error(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self.progress.hide()
            self.active_import_name = None
            self._clear_recovery_offer()
            self.status.setText("Unable to start import authorization.")
            self.output.setPlainText(self.import_process.errorString())
            self.output.show()
            self.scan_button.setEnabled(True)
            self.volumes.setEnabled(True)
            self.models.setEnabled(True)
            self.models_button.setEnabled(
                self.volumes.currentItem() is not None
            )
            self.model_name.setEnabled(
                self.models.currentItem() is not None
            )
            self._update_import_button()

    def review_recovery(self):
        name = self.failed_import_name

        if (
            name is None
            or not self.recovery_button.isEnabled()
            or self.import_process.state() != QProcess.ProcessState.NotRunning
            or self.recovery_process.state() != QProcess.ProcessState.NotRunning
        ):
            return

        confirmation = QMessageBox.question(
            self,
            "Retry Ollama registration",
            "Retry registration for: " + name
            + "\n\nThis will not copy the GGUF file again."
            + "\n\nContinue?",
            QMessageBox.StandardButton.Yes
            | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No,
        )

        if confirmation != QMessageBox.StandardButton.Yes:
            return

        self.start_recovery(name)

    def start_recovery(self, name):
        if (
            name != self.failed_import_name
            or self.process.state() != QProcess.ProcessState.NotRunning
            or self.model_process.state() != QProcess.ProcessState.NotRunning
            or self.import_process.state() != QProcess.ProcessState.NotRunning
            or self.recovery_process.state() != QProcess.ProcessState.NotRunning
        ):
            return

        self.output.clear()
        self.output.hide()
        self.status.setText("Waiting for recovery authorization...")
        self.scan_button.setEnabled(False)
        self.models_button.setEnabled(False)
        self.import_button.setEnabled(False)
        self.recovery_button.setEnabled(False)
        self.volumes.setEnabled(False)
        self.models.setEnabled(False)
        self.model_name.setEnabled(False)

        self.recovery_process.start(
            "pkexec",
            [
                "/usr/bin/daia-model-import",
                "resume-model",
                name,
            ],
        )

    def _recovery_finished(self, exit_code, exit_status):
        stdout = bytes(self.recovery_process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        )
        stderr = bytes(self.recovery_process.readAllStandardError()).decode(
            "utf-8", errors="replace"
        )

        self.scan_button.setEnabled(True)
        self.volumes.setEnabled(True)
        self.models.setEnabled(True)
        self.models_button.setEnabled(
            self.volumes.currentItem() is not None
        )
        self.model_name.setEnabled(
            self.models.currentItem() is not None
        )
        self._update_import_button()

        if (
            exit_status == QProcess.ExitStatus.NormalExit
            and exit_code == 0
        ):
            self.failed_import_name = None
            self.recovery_button.hide()
            self.recovery_button.setEnabled(False)
            self.status.setText("Model recovery completed successfully.")
        else:
            self.status.setText("Model recovery failed.")
            self.output.setPlainText(
                stderr or stdout or
                "Authorization was cancelled or recovery failed."
            )
            self.output.show()
            self.recovery_button.setEnabled(
                exit_status == QProcess.ExitStatus.NormalExit
                and exit_code not in (126, 127)
            )

    def _recovery_error(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self.status.setText("Unable to start recovery authorization.")
            self.output.setPlainText(self.recovery_process.errorString())
            self.output.show()
            self.scan_button.setEnabled(True)
            self.volumes.setEnabled(True)
            self.models.setEnabled(True)
            self.models_button.setEnabled(
                self.volumes.currentItem() is not None
            )
            self.model_name.setEnabled(
                self.models.currentItem() is not None
            )
            self.recovery_button.setEnabled(False)
            self._update_import_button()

    def _volume_selection_changed(self, current, previous):
        self._clear_recovery_offer()
        self.models.clear()
        self.model_name.clear()
        self.model_name.setEnabled(False)
        self.import_button.setEnabled(False)
        self.models_button.setEnabled(
            current is not None
            and self.process.state() == QProcess.ProcessState.NotRunning
            and self.model_process.state() == QProcess.ProcessState.NotRunning
            and self.import_process.state() == QProcess.ProcessState.NotRunning
            and self.recovery_process.state() == QProcess.ProcessState.NotRunning
        )

    def find_models(self):
        if (
            self.process.state() != QProcess.ProcessState.NotRunning
            or self.model_process.state() != QProcess.ProcessState.NotRunning
            or self.import_process.state() != QProcess.ProcessState.NotRunning
            or self.recovery_process.state() != QProcess.ProcessState.NotRunning
        ):
            return

        selected = self.volumes.currentItem()
        if selected is None:
            return

        self._clear_recovery_offer()
        self.models.clear()
        self.model_name.clear()
        self.model_name.setEnabled(False)
        self.import_button.setEnabled(False)
        self.output.clear()
        self.output.hide()
        self.status.setText("Discovering GGUF models...")
        self.scan_button.setEnabled(False)
        self.models_button.setEnabled(False)
        self.volumes.setEnabled(False)

        self.model_process.start(
            "pkexec",
            [
                "/usr/bin/daia-model-import",
                "list-models",
                selected.text(),
            ],
        )

    def _models_finished(self, exit_code, exit_status):
        stdout = bytes(self.model_process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        )
        stderr = bytes(self.model_process.readAllStandardError()).decode(
            "utf-8", errors="replace"
        )

        self.scan_button.setEnabled(True)
        self.volumes.setEnabled(True)
        self.models_button.setEnabled(
            self.volumes.currentItem() is not None
        )

        if (
            exit_status == QProcess.ExitStatus.NormalExit
            and exit_code == 0
        ):
            filenames = [
                line.strip()
                for line in stdout.splitlines()
                if line.strip()
            ]

            if any(
                "/" in name
                or "\\\\" in name
                or not name.lower().endswith(".gguf")
                or name in (".", "..")
                for name in filenames
            ):
                self.status.setText("Invalid GGUF model listing.")
                self.output.setPlainText(
                    "Restricted helper returned an invalid GGUF filename."
                )
                self.output.show()
                return

            self.models.addItems(filenames)
            self.status.setText(
                f"GGUF models found: {len(filenames)}"
            )
        else:
            self.status.setText("Unable to discover GGUF models.")
            self.output.setPlainText(
                stderr or stdout or
                "Authorization was cancelled or model discovery failed."
            )
            self.output.show()

    def _models_error(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self.status.setText("Unable to start authorization helper.")
            self.output.setPlainText(
                self.model_process.errorString()
            )
            self.output.show()
            self.scan_button.setEnabled(True)
            self.volumes.setEnabled(True)
            self.models_button.setEnabled(
                self.volumes.currentItem() is not None
            )

    def scan(self):
        if self.process.state() != QProcess.ProcessState.NotRunning:
            return

        if self.model_process.state() != QProcess.ProcessState.NotRunning:
            return

        if self.import_process.state() != QProcess.ProcessState.NotRunning:
            return

        if self.recovery_process.state() != QProcess.ProcessState.NotRunning:
            return

        self._clear_recovery_offer()
        self.volumes.clear()
        self.models.clear()
        self.model_name.clear()
        self.model_name.setEnabled(False)
        self.import_button.setEnabled(False)
        self.models_button.setEnabled(False)
        self.output.clear()
        self.output.hide()
        self.status.setText("Scanning model source volumes...")
        self.scan_button.setEnabled(False)

        command = os.environ.get("DAIA_COMMAND", "/usr/bin/daia")
        self.process.start(command, ["model", "list-volumes"])

    def _finished(self, exit_code, exit_status):
        stdout = bytes(self.process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        )
        stderr = bytes(self.process.readAllStandardError()).decode(
            "utf-8", errors="replace"
        )

        self.scan_button.setEnabled(True)

        if (
            exit_status == QProcess.ExitStatus.NormalExit
            and exit_code == 0
        ):
            paths = [line.strip() for line in stdout.splitlines() if line.strip()]

            if any(
                not path.startswith("/dev/")
                or path in ("/dev/", "/dev/.", "/dev/..")
                or "/" in path[len("/dev/"):]
                for path in paths
            ):
                self.status.setText("Invalid source-volume listing.")
                self.output.setPlainText(
                    "DAIA CLI returned an invalid device path."
                )
                self.output.show()
                return

            self.volumes.addItems(paths)
            self.status.setText(
                f"Model source volumes: {len(paths)}"
            )
        else:
            self.status.setText("Unable to discover model source volumes.")
            self.output.setPlainText(
                stderr or stdout or "Unknown CLI error."
            )
            self.output.show()

    def _process_error(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self.status.setText("Unable to start DAIA CLI.")
            self.output.setPlainText(self.process.errorString())
            self.output.show()
            self.scan_button.setEnabled(True)


class BenchmarkTab(QWidget):
    """Run the existing Rust model benchmark asynchronously."""

    def __init__(self, parent=None):
        super().__init__(parent)

        self.process = QProcess(self)
        self.process.finished.connect(self._finished)
        self.process.errorOccurred.connect(self._process_error)

        self.status = QLabel("Enter an installed Ollama model name to benchmark.")
        self.model_name = QLineEdit()
        self.model_name.setPlaceholderText("Model name, e.g. example:latest")
        self.model_name.textChanged.connect(self._update_button)

        self.run_button = QPushButton("Run Benchmark")
        self.run_button.setEnabled(False)
        self.run_button.clicked.connect(self.run_benchmark)

        self.output = QTextEdit()
        self.output.setReadOnly(True)

        layout = QVBoxLayout(self)
        layout.addWidget(self.status)
        layout.addWidget(self.model_name)
        layout.addWidget(self.run_button)
        layout.addWidget(self.output)

    def _update_button(self):
        self.run_button.setEnabled(
            bool(self.model_name.text().strip())
            and self.process.state() == QProcess.ProcessState.NotRunning
        )

    def run_benchmark(self):
        if self.process.state() != QProcess.ProcessState.NotRunning:
            return

        name = self.model_name.text().strip()
        if not name:
            return

        self.output.clear()
        self.status.setText(f"Benchmarking {name}...")
        self.model_name.setEnabled(False)
        self.run_button.setEnabled(False)

        command = os.environ.get("DAIA_COMMAND", "/usr/bin/daia")
        self.process.start(command, ["model", "benchmark", name])

    def _finished(self, exit_code, exit_status):
        stdout = bytes(self.process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        )
        stderr = bytes(self.process.readAllStandardError()).decode(
            "utf-8", errors="replace"
        )

        self.model_name.setEnabled(True)
        self._update_button()

        if (
            exit_status == QProcess.ExitStatus.NormalExit
            and exit_code == 0
        ):
            self.status.setText("Benchmark completed.")
            self.output.setPlainText(stdout)
        else:
            self.status.setText("Benchmark failed.")
            self.output.setPlainText(stderr or stdout or "Unknown CLI error.")

    def _process_error(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self.status.setText("Unable to start DAIA benchmark CLI.")
            self.output.setPlainText(self.process.errorString())
            self.model_name.setEnabled(True)
            self._update_button()

    def closeEvent(self, event):
        if self.process.state() != QProcess.ProcessState.NotRunning:
            self.process.terminate()
            if not self.process.waitForFinished(3000):
                self.process.kill()
                self.process.waitForFinished(3000)

        if self.process.state() != QProcess.ProcessState.NotRunning:
            event.ignore()
            return

        super().closeEvent(event)


class ModelManager(QMainWindow):
    def __init__(self):
        super().__init__()

        self.setWindowTitle("DAIA Model Manager")
        self.resize(850, 560)

        tabs = QTabWidget()
        tabs.addTab(InstalledTab(), "Installed")
        tabs.addTab(ImportTab(), "Import")

        tabs.addTab(BenchmarkTab(), "Benchmark")

        placeholder = QWidget()
        layout = QVBoxLayout(placeholder)
        layout.addWidget(QLabel("Export integration is coming soon."))
        tabs.addTab(placeholder, "Export")

        self.setCentralWidget(tabs)

    def closeEvent(self, event):
        tabs = self.centralWidget()

        for index in range(tabs.count()):
            if tabs.tabText(index) != "Benchmark":
                continue

            benchmark = tabs.widget(index)

            if not benchmark.close():
                event.ignore()
                return

            if benchmark.process.state() != QProcess.ProcessState.NotRunning:
                event.ignore()
                return

            break

        super().closeEvent(event)


def main():
    app = QApplication(sys.argv)
    window = ModelManager()
    window.show()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
