"""Mock-only SearXNG initializer tests."""

from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "modules"))

import searxng_firstboot_initializer as target
from searxng_bootstrap import BootstrapDecision

ELIGIBLE = BootstrapDecision.INITIALIZE_ELIGIBLE


class InitializerTests(unittest.TestCase):

    def execute(self, uid=0, enabled=True, bootstrap=ELIGIBLE,
                inventory=None, docker_rc=0):
        calls = []

        def fake_run(command, **kwargs):
            calls.append(tuple(command))
            if command[1] == "info":
                return SimpleNamespace(
                    returncode=docker_rc,
                    stdout="daemon-id\n",
                )
            if inventory is not None:
                return inventory(command, calls)
            return SimpleNamespace(returncode=0, stdout="")

        with (
            patch.object(target.os, "geteuid", return_value=uid),
            patch.object(target.searxng_config, "enabled",
                         return_value=enabled) as config,
            patch.object(target.searxng_bootstrap, "decide",
                         return_value=bootstrap) as state,
            patch.object(target.subprocess, "run",
                         side_effect=fake_run),
            patch.object(target.searxng_initialize,
                         "initialize") as initialize,
        ):
            try:
                target.initialize_from_firstboot()
                outcome = "success"
            except target.InitializationRefused:
                outcome = "refused"

            return (
                outcome, calls, initialize.call_count,
                config.call_count, state.call_count,
            )

    def test_success_rechecks_before_mutation(self):
        outcome, calls, count, config, state = self.execute()
        self.assertEqual(outcome, "success")
        self.assertEqual(count, 1)
        self.assertEqual(config, 2)
        self.assertEqual(state, 2)
        self.assertEqual(
            [command[1] for command in calls],
            ["info", "container", "network", "container", "network"],
        )

    def test_nonroot_refused(self):
        outcome, calls, count, _, _ = self.execute(uid=1000)
        self.assertEqual((outcome, calls, count), ("refused", [], 0))

    def test_disabled_refused(self):
        outcome, calls, count, _, _ = self.execute(enabled=False)
        self.assertEqual((outcome, calls, count), ("refused", [], 0))

    def test_existing_registration_refused(self):
        outcome, calls, count, _, _ = self.execute(
            bootstrap=BootstrapDecision.OBSERVE_EXISTING
        )
        self.assertEqual((outcome, calls, count), ("refused", [], 0))

    def test_docker_failure_refused(self):
        outcome, _, count, _, _ = self.execute(docker_rc=1)
        self.assertEqual((outcome, count), ("refused", 0))

    def test_existing_container_refused(self):
        def inventory(command, calls):
            value = "daia-searxng\n" if command[1] == "container" else ""
            return SimpleNamespace(returncode=0, stdout=value)

        outcome, _, count, _, _ = self.execute(inventory=inventory)
        self.assertEqual((outcome, count), ("refused", 0))

    def test_existing_network_refused(self):
        def inventory(command, calls):
            value = (
                "daia-searxng-egress\n"
                if command[1] == "network" else ""
            )
            return SimpleNamespace(returncode=0, stdout=value)

        outcome, _, count, _, _ = self.execute(inventory=inventory)
        self.assertEqual((outcome, count), ("refused", 0))

    def test_inventory_error_refused(self):
        def inventory(command, calls):
            return SimpleNamespace(returncode=1, stdout="")

        outcome, _, count, _, _ = self.execute(inventory=inventory)
        self.assertEqual((outcome, count), ("refused", 0))

    def test_inventory_drift_refused(self):
        def inventory(command, calls):
            container_checks = sum(
                item[1] == "container" for item in calls
            )
            value = (
                "daia-searxng\n"
                if command[1] == "container" and container_checks == 2
                else ""
            )
            return SimpleNamespace(returncode=0, stdout=value)

        outcome, _, count, _, _ = self.execute(inventory=inventory)
        self.assertEqual((outcome, count), ("refused", 0))

    def test_not_wired_into_firstboot(self):
        source = (ROOT / "firstboot.sh").read_text()
        main = source[source.index("\nmain()\n"):]
        self.assertIn("provision_searxng_firstboot()", source)
        self.assertIn(
            "\n    provision_searxng_firstboot\n", main
        )

    def test_not_wired_into_runtime(self):
        self.assertNotIn(
            "searxng_firstboot_initializer",
            (ROOT / "runtime.sh").read_text(),
        )


if __name__ == "__main__":
    unittest.main()
