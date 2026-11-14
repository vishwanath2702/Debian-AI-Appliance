"""Behavioral tests for Open WebUI registration stopped-state verification."""

import importlib.machinery
import importlib.util
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

RUNTIME = Path(__file__).resolve().parents[1]
HELPER = RUNTIME / "daia-openwebui-register-container"
sys.path.insert(0, str(RUNTIME))

loader = importlib.machinery.SourceFileLoader(
    "daia_openwebui_stopped_state_under_test", str(HELPER)
)
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)

CONTAINER_ID = "a" * 64


class OpenWebUIStoppedStateTests(unittest.TestCase):
    def inspect(self, response):
        with patch.object(helper.subprocess, "run") as runner:
            runner.return_value.stdout = response
            helper.inspect_container_stopped(CONTAINER_ID)
            runner.assert_called_once()
            args = runner.call_args.args[0]
            self.assertEqual(args[:3], ["docker", "container", "inspect"])
            self.assertEqual(args[-1], CONTAINER_ID)
            self.assertTrue(runner.call_args.kwargs["check"])
            self.assertIn("timeout", runner.call_args.kwargs)

    def test_accepts_created_container(self):
        self.inspect("false|false|false|created\n")

    def test_accepts_exited_container(self):
        self.inspect("false|false|false|exited\n")

    def test_rejects_running_container(self):
        with self.assertRaises(PermissionError):
            self.inspect("true|false|false|running\n")

    def test_rejects_paused_container(self):
        with self.assertRaises(PermissionError):
            self.inspect("true|true|false|paused\n")

    def test_rejects_restarting_container(self):
        with self.assertRaises(PermissionError):
            self.inspect("true|false|true|restarting\n")

    def test_rejects_unexpected_status(self):
        with self.assertRaises(PermissionError):
            self.inspect("false|false|false|dead\n")

    def test_rejects_malformed_response(self):
        with self.assertRaises(PermissionError):
            self.inspect("false|false|false\n")

    def test_rejects_multiple_response_lines(self):
        with self.assertRaises(PermissionError):
            self.inspect("false|false|false|created\nextra\n")

    def test_rejects_invalid_id_before_docker(self):
        with patch.object(helper.subprocess, "run") as runner:
            with self.assertRaises(ValueError):
                helper.inspect_container_stopped("invalid")
            runner.assert_not_called()

    def test_docker_inspection_failure_is_fatal(self):
        with patch.object(
            helper.subprocess,
            "run",
            side_effect=subprocess.CalledProcessError(1, ["docker"]),
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                helper.inspect_container_stopped(CONTAINER_ID)


if __name__ == "__main__":
    unittest.main()
