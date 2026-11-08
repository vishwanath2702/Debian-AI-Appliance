"""Headless selection tests for DAIA installed models."""

import importlib.util
import os
import unittest
from pathlib import Path
from unittest.mock import patch

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PyQt6.QtCore import QProcess
from PyQt6.QtWidgets import QApplication

MANAGER = Path(__file__).resolve().parents[1] / "daia-model-manager.py"
SPEC = importlib.util.spec_from_file_location("daia_installed_selection", MANAGER)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class InstalledSelectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.window = MODULE.ModelManager()
        self.addCleanup(self.window.close)

        self.tabs = self.window.centralWidget()
        self.installed = self.tabs.widget(0)

        self.benchmark = self.installed.benchmark_tab
        self.assertEqual(
            [self.tabs.tabText(i) for i in range(self.tabs.count())],
            ["Installed", "Import", "Export"],
        )

    def inventory(self, payload):
        with patch.object(
            self.installed.process,
            "readAllStandardOutput",
            return_value=payload,
        ), patch.object(
            self.installed.process,
            "readAllStandardError",
            return_value=b"",
        ):
            self.installed._finished(
                0, QProcess.ExitStatus.NormalExit
            )

    def test_initial_action_disabled(self):
        self.assertFalse(self.installed.benchmark_button.isEnabled())
        self.assertIsNone(self.installed.selected_model_name())

    def test_selection_enables_action(self):
        self.inventory(
            b'[{"name":"example:latest","size_bytes":1024}]'
        )
        self.installed.table.selectRow(0)

        self.assertEqual(
            self.installed.selected_model_name(),
            "example:latest",
        )
        self.assertTrue(self.installed.benchmark_button.isEnabled())

    def test_exact_name_transfer_launches_benchmark_without_embedded_ui(self):
        self.inventory(
            b'[{"name":"example:q4_k_m","size_bytes":1024}]'
        )
        self.installed.table.selectRow(0)

        with patch.object(self.benchmark, "run_benchmark") as launch:
            self.installed.benchmark_selected()
            launch.assert_called_once_with()

        self.assertEqual(
            self.benchmark.model_name.text(),
            "example:q4_k_m",
        )
        self.assertIs(self.tabs.currentWidget(), self.installed)
        self.assertFalse(self.benchmark.isVisibleTo(self.installed))
        self.assertEqual(
            self.benchmark.process.state(),
            QProcess.ProcessState.NotRunning,
        )

    def test_cleared_selection_disables_action(self):
        self.inventory(
            b'[{"name":"example:latest","size_bytes":1024}]'
        )
        self.installed.table.selectRow(0)
        self.installed.table.clearSelection()

        self.assertIsNone(self.installed.selected_model_name())
        self.assertFalse(self.installed.benchmark_button.isEnabled())

    def test_invalid_inventory_does_not_enable_action(self):
        self.inventory(b'{"invalid":true}')

        self.assertEqual(self.installed.table.rowCount(), 0)
        self.assertFalse(self.installed.benchmark_button.isEnabled())

    def test_refresh_clears_selection(self):
        self.inventory(
            b'[{"name":"example:latest","size_bytes":1024}]'
        )
        self.installed.table.selectRow(0)

        with patch.object(self.installed.process, "start"):
            self.installed.refresh()

        self.assertEqual(self.installed.table.rowCount(), 0)
        self.assertIsNone(self.installed.selected_model_name())
        self.assertFalse(self.installed.benchmark_button.isEnabled())


    def test_benchmark_status_visible_in_installed_tab(self):
        cases = (
            "Benchmarking example:latest...",
            "Benchmark failed.",
            "Benchmark report unavailable.",
            "Benchmark completed. Browser report opened.",
        )
        for message in cases:
            with self.subTest(message=message):
                self.benchmark._set_benchmark_status(message)
                self.assertEqual(self.installed.status.text(), message)

        self.assertFalse(self.benchmark.isVisibleTo(self.installed))


if __name__ == "__main__":
    unittest.main()
