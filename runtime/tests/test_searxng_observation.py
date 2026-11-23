"""Mocked tests for read-only SearXNG observations."""

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

CID = "a" * 64
NID = "b" * 64


def network(members=None, **overrides):
    result = {
        "Name": "daia-searxng-egress",
        "Driver": "bridge",
        "Internal": False,
        "Id": NID,
        "Labels": {
            "org.daia.component": "searxng",
            "org.daia.managed": "true",
        },
        "Containers": members if members is not None else {},
    }
    result.update(overrides)
    return result


class FakeRegistration:
    DEFAULT_DIRECTORY = "/unused"
    REGISTRATION_NAME = "container-id"

    def __init__(self, identity=None, failure=None):
        self.identity = identity
        self.failure = failure

    def _open_trusted_directory(self, directory):
        if self.failure == "directory":
            raise PermissionError("Untrusted directory")
        return 100

    def _read_registration_fd(self, fd):
        if self.failure == "file":
            raise PermissionError("Untrusted registration")
        if self.identity is None:
            raise FileNotFoundError(
                2, "Missing registration", self.REGISTRATION_NAME
            )
        return self.identity


class FakeAssessor:
    def __init__(
        self,
        assessment="ABSENT",
        egress=None,
        inventory_failure=False,
        inspect_failure=False,
        changed=False,
    ):
        self.assessment = assessment
        self.egress = egress
        self.inventory_failure = inventory_failure
        self.inspect_failure = inspect_failure
        self.changed = changed
        self.assess_count = 0
        self.calls = []

    def assess(self, registration):
        self.assess_count += 1
        if self.changed and self.assess_count > 1:
            return "UNTRUSTED"
        return self.assessment

    def docker(self, *args):
        self.calls.append(args)

        if args == (
            "network", "ls", "--format", "{{.Name}}"
        ):
            if self.inventory_failure:
                raise RuntimeError("Docker inventory failed")
            return (
                "daia-searxng-egress\n"
                if self.egress is not None else ""
            )

        if args == (
            "network", "inspect", "daia-searxng-egress"
        ):
            if self.inspect_failure:
                raise RuntimeError("Docker inspect failed")
            return json.dumps([self.egress])

        raise AssertionError("Unexpected Docker operation")


class ObservationTests(unittest.TestCase):

    def setUp(self):
        self.root = patch.object(
            adapter.os, "geteuid", return_value=0
        )
        self.close = patch.object(
            adapter.os, "close", return_value=None
        )
        self.root.start()
        self.close.start()
        self.addCleanup(self.root.stop)
        self.addCleanup(self.close.stop)

    def decision(self, assessor, registration=None):
        return adapter.observe(
            assessor,
            registration or FakeRegistration(),
        )

    def test_fresh(self):
        self.assertEqual(
            self.decision(FakeAssessor()),
            Decision.FRESH,
        )

    def test_trusted_empty_network(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(egress=network())
            ),
            Decision.INCOMPLETE_NETWORK,
        )

    def test_verified_registered(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    assessment="REGISTERED_VERIFIED",
                    egress=network({CID: {}}),
                ),
                FakeRegistration(CID),
            ),
            Decision.REGISTERED_VERIFIED,
        )

    def test_verified_stopped_membership_omitted(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    assessment="REGISTERED_VERIFIED",
                    egress=network(),
                ),
                FakeRegistration(CID),
            ),
            Decision.REGISTERED_VERIFIED,
        )

    def test_unregistered_created_container(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    assessment="UNREGISTERED_STOPPED",
                    egress=network(),
                )
            ),
            Decision.INCOMPLETE_CONTAINER,
        )

    def test_missing_registration_directory(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(),
                FakeRegistration(failure="directory"),
            ),
            Decision.UNTRUSTED,
        )

    def test_untrusted_registration_file(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(),
                FakeRegistration(failure="file"),
            ),
            Decision.UNTRUSTED,
        )

    def test_registration_without_container(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(),
                FakeRegistration(CID),
            ),
            Decision.UNTRUSTED,
        )

    def test_inventory_failure(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(inventory_failure=True)
            ),
            Decision.UNTRUSTED,
        )

    def test_network_inspection_failure(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    egress=network(),
                    inspect_failure=True,
                )
            ),
            Decision.UNTRUSTED,
        )

    def test_unmanaged_network(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    egress=network(
                        Labels={"org.daia.managed": "false"}
                    )
                )
            ),
            Decision.UNTRUSTED,
        )

    def test_wrong_network_driver(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    egress=network(Driver="host")
                )
            ),
            Decision.UNTRUSTED,
        )

    def test_wrong_isolation(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    egress=network(Internal=True)
                )
            ),
            Decision.UNTRUSTED,
        )

    def test_unexpected_network_member(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    egress=network({"c" * 64: {}})
                )
            ),
            Decision.UNTRUSTED,
        )

    def test_registered_foreign_member(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    assessment="REGISTERED_VERIFIED",
                    egress=network({"c" * 64: {}}),
                ),
                FakeRegistration(CID),
            ),
            Decision.UNTRUSTED,
        )

    def test_malformed_network_identity(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(
                    egress=network(Id="invalid")
                )
            ),
            Decision.UNTRUSTED,
        )

    def test_assessment_drift(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(changed=True)
            ),
            Decision.UNTRUSTED,
        )

    def test_assessor_refusal(self):
        self.assertEqual(
            self.decision(
                FakeAssessor(assessment="UNTRUSTED")
            ),
            Decision.UNTRUSTED,
        )

    def test_nonroot_refused(self):
        with patch.object(
            adapter.os, "geteuid", return_value=1000
        ):
            self.assertEqual(
                self.decision(FakeAssessor()),
                Decision.UNTRUSTED,
            )

    def test_no_mutation_commands(self):
        assessor = FakeAssessor(egress=network())
        self.decision(assessor)

        for command in assessor.calls:
            self.assertEqual(command[0], "network")
            self.assertIn(command[1], ("ls", "inspect"))

    def test_runtime_remains_unwired(self):
        source = (ROOT / "runtime.sh").read_text()
        self.assertNotIn(
            'RUNTIME_MODULES+=("searxng")',
            source,
        )


if __name__ == "__main__":
    unittest.main()
