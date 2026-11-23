"""SearXNG egress inventory drift regression tests."""

import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
MODULES = ROOT / "modules"

sys.path.insert(0, str(MODULES))

from searxng_retry_policy import Decision

spec = importlib.util.spec_from_file_location(
    "searxng_observation",
    MODULES / "searxng_observation.py",
)
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)

NETWORK_NAME = "daia-searxng-egress"
NETWORK_ID = "b" * 64


class Registration:
    DEFAULT_DIRECTORY = "/unused"
    REGISTRATION_NAME = "container-id"

    def _open_trusted_directory(self, directory):
        return 100

    def _read_registration_fd(self, fd):
        raise FileNotFoundError(
            2, "Missing registration", self.REGISTRATION_NAME
        )


def network():
    return {
        "Name": NETWORK_NAME,
        "Driver": "bridge",
        "Internal": False,
        "Id": NETWORK_ID,
        "Labels": {
            "org.daia.component": "searxng",
            "org.daia.managed": "true",
        },
        "Containers": {},
    }


class Assessor:
    def __init__(self, inventories, inspections=None):
        self.inventories = list(inventories)
        self.inspections = list(inspections or [])
        self.commands = []

    def assess(self, registration):
        return "ABSENT"

    def docker(self, *arguments):
        self.commands.append(arguments)

        if arguments == (
            "network", "ls", "--format", "{{.Name}}"
        ):
            if not self.inventories:
                raise RuntimeError("Unexpected inventory query")
            return self.inventories.pop(0)

        if arguments == (
            "network", "inspect", NETWORK_NAME
        ):
            if not self.inspections:
                raise RuntimeError("Unexpected network inspection")
            return json.dumps([self.inspections.pop(0)])

        raise AssertionError(
            "Unexpected Docker command: " + str(arguments)
        )


class DriftTests(unittest.TestCase):

    def setUp(self):
        root = patch.object(
            adapter.os, "geteuid", return_value=0
        )
        close = patch.object(
            adapter.os, "close", return_value=None
        )
        root.start()
        close.start()
        self.addCleanup(root.stop)
        self.addCleanup(close.stop)

    def decision(self, inventories, inspections=None):
        assessor = Assessor(inventories, inspections)
        decision = adapter.observe(
            assessor,
            Registration(),
        )
        return decision, assessor

    def test_absent_then_present_is_untrusted(self):
        decision, _ = self.decision(
            ["", NETWORK_NAME + "\n"]
        )
        self.assertEqual(decision, Decision.UNTRUSTED)

    def test_present_then_absent_is_untrusted(self):
        decision, _ = self.decision(
            [NETWORK_NAME + "\n", ""],
            [network(), network()],
        )
        self.assertEqual(decision, Decision.UNTRUSTED)

    def test_stable_absence_is_fresh(self):
        decision, assessor = self.decision(["", ""])
        self.assertEqual(decision, Decision.FRESH)
        self.assertEqual(
            sum(
                command[1] == "ls"
                for command in assessor.commands
            ),
            2,
        )

    def test_stable_presence_is_incomplete_network(self):
        decision, assessor = self.decision(
            [NETWORK_NAME + "\n", NETWORK_NAME + "\n"],
            [network(), network()],
        )
        self.assertEqual(
            decision,
            Decision.INCOMPLETE_NETWORK,
        )
        self.assertEqual(
            sum(
                command[1] == "ls"
                for command in assessor.commands
            ),
            2,
        )

    def test_second_inventory_failure_is_untrusted(self):
        decision, _ = self.decision([""])
        self.assertEqual(decision, Decision.UNTRUSTED)

    def test_duplicate_second_inventory_is_untrusted(self):
        decision, _ = self.decision(
            ["", NETWORK_NAME + "\n" + NETWORK_NAME + "\n"]
        )
        self.assertEqual(decision, Decision.UNTRUSTED)

    def test_no_mutating_docker_commands(self):
        _, assessor = self.decision(["", ""])
        for command in assessor.commands:
            self.assertEqual(command[0], "network")
            self.assertIn(command[1], ("ls", "inspect"))


if __name__ == "__main__":
    unittest.main()
