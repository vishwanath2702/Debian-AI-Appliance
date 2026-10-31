"""Headless tests for the DAIA Model Manager Benchmark tab."""

import importlib.util
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PyQt6.QtCore import QProcess
from PyQt6.QtWidgets import QApplication


GUI = Path(__file__).resolve().parents[1] / "daia-model-manager.py"
SPEC = importlib.util.spec_from_file_location("daia_model_manager_benchmark", GUI)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class BenchmarkTabTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.command = self.root / "mock-daia"

        self.command.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$@" > "$DAIA_TEST_ARGS"\n'
            'case "$DAIA_TEST_MODE" in\n'
            '  success)\n'
            '    printf "DAIA Model Benchmark\\nModel: example:latest\\n"\n'
            '    printf "Measured average generation throughput: 28.24 tok/s\\n"\n'
            '    ;;\n'
            '  failure)\n'
            '    echo "Model benchmark failed: synthetic failure" >&2\n'
            '    exit 1\n'
            '    ;;\n'
            '  wait)\n'
            '    sleep 2\n'
            '    echo "DAIA Model Benchmark"\n'
            '    ;;\n'
            'esac\n'
        )
        self.command.chmod(self.command.stat().st_mode | stat.S_IXUSR)

        self.args = self.root / "arguments.txt"
        self.environment = patch.dict(
            os.environ,
            {
                "DAIA_COMMAND": str(self.command),
                "DAIA_TEST_ARGS": str(self.args),
                "DAIA_TEST_MODE": "success",
                "QT_QPA_PLATFORM": "offscreen",
            },
        )
        self.environment.start()
        self.addCleanup(self.environment.stop)

        self.tab = MODULE.BenchmarkTab()
        self.addCleanup(self.tab.close)

    def finish(self):
        self.assertTrue(
            self.tab.process.waitForFinished(5000),
            "Mock benchmark process did not finish",
        )
        self.app.processEvents()

    def test_empty_model_name_cannot_start(self):
        self.assertFalse(self.tab.run_button.isEnabled())
        self.tab.run_benchmark()
        self.assertEqual(
            self.tab.process.state(), QProcess.ProcessState.NotRunning
        )
        self.assertFalse(self.args.exists())

    def test_success_uses_exact_cli_arguments_and_shows_results(self):
        self.tab.model_name.setText(" example:latest ")
        self.assertTrue(self.tab.run_button.isEnabled())

        self.tab.run_benchmark()
        self.finish()

        self.assertEqual(
            self.args.read_text().splitlines(),
            ["model", "benchmark", "example:latest"],
        )
        self.assertEqual(self.tab.status.text(), "Benchmark completed.")
        self.assertIn(
            "Measured average generation throughput: 28.24 tok/s",
            self.tab.output.toPlainText(),
        )
        self.assertTrue(self.tab.model_name.isEnabled())
        self.assertTrue(self.tab.run_button.isEnabled())

    def test_cli_failure_displays_stderr_and_restores_controls(self):
        os.environ["DAIA_TEST_MODE"] = "failure"
        self.tab.model_name.setText("example:latest")
        self.tab.run_benchmark()
        self.finish()

        self.assertEqual(self.tab.status.text(), "Benchmark failed.")
        self.assertIn(
            "synthetic failure", self.tab.output.toPlainText()
        )
        self.assertTrue(self.tab.model_name.isEnabled())
        self.assertTrue(self.tab.run_button.isEnabled())

    def test_running_process_blocks_duplicate_benchmark(self):
        os.environ["DAIA_TEST_MODE"] = "wait"
        self.tab.model_name.setText("example:latest")
        self.tab.run_benchmark()

        self.assertTrue(self.tab.process.waitForStarted(2000))
        self.assertFalse(self.tab.run_button.isEnabled())
        self.assertFalse(self.tab.model_name.isEnabled())

        self.tab.run_benchmark()
        self.assertEqual(
            self.tab.process.state(), QProcess.ProcessState.Running
        )

        self.finish()
        self.assertEqual(
            self.args.read_text().splitlines(),
            ["model", "benchmark", "example:latest"],
        )
        self.assertTrue(self.tab.run_button.isEnabled())

    def test_closing_tab_stops_running_benchmark(self):
        import signal

        os.environ["DAIA_TEST_MODE"] = "wait"
        self.tab.model_name.setText("example:latest")
        self.tab.run_benchmark()

        self.assertTrue(self.tab.process.waitForStarted(2000))
        pid = int(self.tab.process.processId())
        self.assertGreater(pid, 0)

        self.assertTrue(self.tab.close())
        self.app.processEvents()

        self.assertEqual(
            self.tab.process.state(),
            QProcess.ProcessState.NotRunning,
        )

        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)

    def test_closing_main_window_stops_running_benchmark(self):
        os.environ["DAIA_TEST_MODE"] = "wait"

        window = MODULE.ModelManager()
        self.addCleanup(window.close)

        tabs = window.centralWidget()
        benchmark = next(
            (
                tabs.widget(index)
                for index in range(tabs.count())
                if tabs.tabText(index) == "Benchmark"
            ),
            None,
        )
        self.assertIsNotNone(benchmark)

        benchmark.model_name.setText("example:latest")
        benchmark.run_benchmark()
        self.assertTrue(benchmark.process.waitForStarted(2000))

        pid = int(benchmark.process.processId())
        self.assertGreater(pid, 0)

        self.assertTrue(window.close())
        self.app.processEvents()

        try:
            self.assertEqual(
                benchmark.process.state(),
                QProcess.ProcessState.NotRunning,
            )
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)
        finally:
            if benchmark.process.state() != QProcess.ProcessState.NotRunning:
                benchmark.process.kill()
                benchmark.process.waitForFinished(3000)

    def test_missing_cli_reports_start_failure(self):
        os.environ["DAIA_COMMAND"] = str(self.root / "missing-daia")
        self.tab.model_name.setText("example:latest")
        self.tab.run_benchmark()

        self.assertFalse(self.tab.process.waitForStarted(3000))
        self.app.processEvents()

        self.assertEqual(
            self.tab.process.error(),
            QProcess.ProcessError.FailedToStart,
        )
        self.assertEqual(
            self.tab.status.text(),
            "Unable to start DAIA benchmark CLI.",
        )
        self.assertTrue(self.tab.model_name.isEnabled())
        self.assertTrue(self.tab.run_button.isEnabled())


if __name__ == "__main__":
    unittest.main()
