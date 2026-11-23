"""Inert SearXNG firstboot runner regression tests."""

import contextlib
import inspect
import os
import sys
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

MODULES = Path(__file__).resolve().parents[1] / "modules"
sys.path.insert(0, str(MODULES))

import searxng_firstboot_runner as runner


class RunnerTests(unittest.TestCase):

    def test_success_holds_lock_during_execution(self):
        events = []

        @contextlib.contextmanager
        def locked():
            events.append("acquire")
            try:
                yield
            finally:
                events.append("release")

        def execute(command, **kwargs):
            self.assertEqual(
                command,
                runner.PROVISION_COMMAND,
            )
            self.assertEqual(events, ["acquire"])
            self.assertTrue(kwargs["close_fds"])
            events.append("execute")
            return subprocess.CompletedProcess(command, 0)

        with (
            patch.object(runner.os, "geteuid", return_value=0),
            patch.object(
                runner, "exclusive_firstboot_lock", locked
            ),
            patch.object(
                runner.subprocess, "run", side_effect=execute
            ),
        ):
            self.assertTrue(runner.run())

        self.assertEqual(
            events,
            ["acquire", "execute", "release"],
        )

    def test_nonroot_refused_before_lock(self):
        with (
            patch.object(
                runner.os, "geteuid", return_value=1000
            ),
            patch.object(
                runner, "exclusive_firstboot_lock"
            ) as lock,
            patch.object(
                runner.subprocess, "run"
            ) as execute,
        ):
            with self.assertRaises(runner.RunnerRefused):
                runner.run()

            lock.assert_not_called()
            execute.assert_not_called()

    def test_lock_refusal_prevents_execution(self):
        def refuse():
            raise RuntimeError("busy")

        with (
            patch.object(
                runner.os, "geteuid", return_value=0
            ),
            patch.object(
                runner,
                "exclusive_firstboot_lock",
                side_effect=refuse,
            ),
            patch.object(
                runner.subprocess, "run"
            ) as execute,
        ):
            with self.assertRaises(RuntimeError):
                runner.run()

            execute.assert_not_called()

    def test_nonzero_exit_refused(self):
        with (
            patch.object(
                runner.os, "geteuid", return_value=0
            ),
            patch.object(
                runner,
                "exclusive_firstboot_lock",
                contextlib.nullcontext,
            ),
            patch.object(
                runner.subprocess,
                "run",
                return_value=subprocess.CompletedProcess(
                    runner.PROVISION_COMMAND, 1
                ),
            ),
        ):
            with self.assertRaises(runner.RunnerRefused):
                runner.run()

    def test_timeout_refused(self):
        with (
            patch.object(
                runner.os, "geteuid", return_value=0
            ),
            patch.object(
                runner,
                "exclusive_firstboot_lock",
                contextlib.nullcontext,
            ),
            patch.object(
                runner.subprocess,
                "run",
                side_effect=subprocess.TimeoutExpired(
                    runner.PROVISION_COMMAND, 300
                ),
            ),
        ):
            with self.assertRaises(runner.RunnerRefused):
                runner.run()

    def test_missing_command_refused(self):
        with (
            patch.object(
                runner.os, "geteuid", return_value=0
            ),
            patch.object(
                runner,
                "exclusive_firstboot_lock",
                contextlib.nullcontext,
            ),
            patch.object(
                runner.subprocess,
                "run",
                side_effect=FileNotFoundError(),
            ),
        ):
            with self.assertRaises(runner.RunnerRefused):
                runner.run()

    def test_no_caller_selected_command(self):
        self.assertEqual(
            list(inspect.signature(runner.run).parameters),
            [],
        )
        self.assertEqual(
            runner.PROVISION_COMMAND,
            (
                "/usr/libexec/daia-searxng-firstboot-provision",
            ),
        )

    def test_firstboot_remains_deferred(self):
        source = (
            MODULES.parent / "firstboot.sh"
        ).read_text()

        main = source[source.index("\nmain()\n"):]

        self.assertIn(
            "\n    provision_searxng_firstboot\n",
            main,
        )

    def test_no_direct_docker_or_service_mutations(self):
        source = (
            MODULES / "searxng_firstboot_runner.py"
        ).read_text()

        self.assertNotIn("docker ", source)
        self.assertNotIn("systemctl", source)
        self.assertNotIn("shell=True", source)


if __name__ == "__main__":
    unittest.main()
