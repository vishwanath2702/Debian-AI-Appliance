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
            '    printf \'%s\\n\' \'{"schema_version":1,"event":"started","model":"example:latest"}\'\n'
            '    printf \'%s\\n\' \'{"schema_version":1,"event":"completed","model":"example:latest","runs":[{"kind":"measured","eval_count":100,"eval_duration_ns":4000000000,"total_duration_ns":5000000000}]}\'\n'
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

    def test_success_uses_jsonl_and_opens_completed_browser_report(self):
        self.tab.model_name.setText(" example:latest ")
        self.assertTrue(self.tab.run_button.isEnabled())

        with patch.object(MODULE.webbrowser, "open", return_value=True) as browser:
            self.tab.run_benchmark()
            self.finish()
            browser.assert_called_once()
            report_uri = browser.call_args.args[0]

        self.assertEqual(
            self.args.read_text().splitlines(),
            ["model", "benchmark", "example:latest", "--events-jsonl"],
        )
        self.assertEqual(
            self.tab.status.text(),
            "Benchmark completed. Browser report opened.",
        )
        self.assertTrue(report_uri.startswith("file:"))
        report_path = Path(__import__("urllib.parse", fromlist=["urlparse"]).urlparse(report_uri).path)
        html = report_path.read_text(encoding="utf-8")
        self.assertIn("example:latest", html)
        self.assertIn("Performance", html)
        self.assertIn("Hardware", html)
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
            ["model", "benchmark", "example:latest", "--events-jsonl"],
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
        self.assertEqual(
            [tabs.tabText(i) for i in range(tabs.count())],
            ["Installed", "Import", "Export"],
        )
        benchmark = tabs.widget(0).benchmark_tab
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


    def test_browser_failure_removes_report(self):
        from pathlib import Path
        from unittest.mock import patch

        events = (
            '{"schema_version":1,"event":"started","model":"example:latest"}\n'
            '{"schema_version":1,"event":"completed","model":"example:latest",'
            '"runs":[{"kind":"measured","eval_count":100,'
            '"eval_duration_ns":4000000000,"total_duration_ns":5000000000}]}\n'
        )

        class FakeProcess:
            def readAllStandardOutput(self):
                return events.encode()

            def readAllStandardError(self):
                return b""

        original_process = self.tab.process
        self.tab.process = FakeProcess()
        successful_path = None

        try:
            with patch.object(MODULE.webbrowser, "open", return_value=False) as browser:
                self.tab._finished(0, MODULE.QProcess.ExitStatus.NormalExit)
                self.assertEqual(browser.call_count, 1)
                failed_url = browser.call_args.args[0]
                from urllib.parse import urlparse, unquote
                failed_path = Path(unquote(urlparse(failed_url).path))
                self.assertFalse(failed_path.exists())
                self.assertEqual(
                    self.tab.status.text(), "Benchmark report unavailable."
                )

            with patch.object(MODULE.webbrowser, "open", return_value=True):
                self.tab._finished(0, MODULE.QProcess.ExitStatus.NormalExit)
                successful_path = Path(self.tab.output.toPlainText())
                self.assertTrue(successful_path.is_file())
                self.assertEqual(
                    successful_path.stat().st_mode & 0o777, 0o600
                )
                self.assertEqual(
                    self.tab.status.text(),
                    "Benchmark completed. Browser report opened.",
                )
        finally:
            self.tab.process = original_process
            if successful_path is not None:
                successful_path.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
