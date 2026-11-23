"""Fail-closed SearXNG retry-policy tests."""

import importlib.util
from pathlib import Path
import sys
import unittest

MODULE = (
    Path(__file__).resolve().parents[1]
    / "modules/searxng_retry_policy.py"
)

spec = importlib.util.spec_from_file_location(
    "searxng_retry_policy", MODULE
)
policy = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = policy
spec.loader.exec_module(policy)

Observation = policy.Observation
Decision = policy.Decision


class RetryPolicyTests(unittest.TestCase):

    def decide(self, assessment, egress, registration):
        return policy.classify(
            Observation(assessment, egress, registration)
        )

    def test_fresh(self):
        self.assertEqual(
            self.decide("ABSENT", "ABSENT", "TRUSTED_EMPTY"),
            Decision.FRESH,
        )

    def test_existing_verified(self):
        self.assertEqual(
            self.decide(
                "REGISTERED_VERIFIED",
                "TRUSTED_ATTACHED",
                "TRUSTED_PRESENT",
            ),
            Decision.REGISTERED_VERIFIED,
        )

    def test_interrupted_after_network_creation(self):
        self.assertEqual(
            self.decide(
                "ABSENT", "TRUSTED_EMPTY", "TRUSTED_EMPTY"
            ),
            Decision.INCOMPLETE_NETWORK,
        )

    def test_interrupted_after_container_creation(self):
        self.assertEqual(
            self.decide(
                "UNREGISTERED_STOPPED",
                "TRUSTED_ATTACHED",
                "TRUSTED_EMPTY",
            ),
            Decision.INCOMPLETE_CONTAINER,
        )

    def test_unregistered_stopped_without_attachment(self):
        self.assertEqual(
            self.decide(
                "UNREGISTERED_STOPPED",
                "TRUSTED_EMPTY",
                "TRUSTED_EMPTY",
            ),
            Decision.INCOMPLETE_CONTAINER,
        )

    def test_registration_without_container(self):
        self.assertEqual(
            self.decide(
                "ABSENT", "ABSENT", "TRUSTED_PRESENT"
            ),
            Decision.UNTRUSTED,
        )

    def test_untrusted_egress(self):
        self.assertEqual(
            self.decide(
                "ABSENT", "UNTRUSTED", "TRUSTED_EMPTY"
            ),
            Decision.UNTRUSTED,
        )

    def test_untrusted_registration_directory(self):
        self.assertEqual(
            self.decide(
                "ABSENT", "ABSENT", "UNTRUSTED"
            ),
            Decision.UNTRUSTED,
        )

    def test_registered_without_egress_attachment(self):
        self.assertEqual(
            self.decide(
                "REGISTERED_VERIFIED",
                "TRUSTED_EMPTY",
                "TRUSTED_PRESENT",
            ),
            Decision.UNTRUSTED,
        )

    def test_registered_without_registration(self):
        self.assertEqual(
            self.decide(
                "REGISTERED_VERIFIED",
                "TRUSTED_ATTACHED",
                "TRUSTED_EMPTY",
            ),
            Decision.UNTRUSTED,
        )

    def test_unknown_assessment(self):
        self.assertEqual(
            self.decide(
                "UNKNOWN", "ABSENT", "TRUSTED_EMPTY"
            ),
            Decision.UNTRUSTED,
        )

    def test_unknown_egress(self):
        self.assertEqual(
            self.decide(
                "ABSENT", "UNKNOWN", "TRUSTED_EMPTY"
            ),
            Decision.UNTRUSTED,
        )

    def test_unknown_registration(self):
        self.assertEqual(
            self.decide(
                "ABSENT", "ABSENT", "UNKNOWN"
            ),
            Decision.UNTRUSTED,
        )

    def test_all_combinations_fail_closed_except_known(self):
        expected = {
            (
                "ABSENT", "ABSENT", "TRUSTED_EMPTY"
            ): Decision.FRESH,
            (
                "ABSENT", "TRUSTED_EMPTY", "TRUSTED_EMPTY"
            ): Decision.INCOMPLETE_NETWORK,
            (
                "REGISTERED_VERIFIED",
                "TRUSTED_ATTACHED",
                "TRUSTED_PRESENT",
            ): Decision.REGISTERED_VERIFIED,
            (
                "UNREGISTERED_STOPPED",
                "TRUSTED_EMPTY",
                "TRUSTED_EMPTY",
            ): Decision.INCOMPLETE_CONTAINER,
            (
                "UNREGISTERED_STOPPED",
                "TRUSTED_ATTACHED",
                "TRUSTED_EMPTY",
            ): Decision.INCOMPLETE_CONTAINER,
        }

        for assessment in policy.ASSESSMENTS:
            for egress in policy.EGRESS_STATES:
                for registration in policy.REGISTRATION_DIRECTORIES:
                    key = (assessment, egress, registration)
                    with self.subTest(state=key):
                        self.assertEqual(
                            self.decide(*key),
                            expected.get(
                                key, Decision.UNTRUSTED
                            ),
                        )

    def test_only_fresh_is_eligible(self):
        for decision in Decision:
            self.assertEqual(
                policy.may_create_fresh(decision),
                decision is Decision.FRESH,
            )

    def test_invalid_input_rejected(self):
        for value in (
            None, {}, "FRESH", 42,
            Observation(None, "ABSENT", "TRUSTED_EMPTY"),
        ):
            with self.subTest(value=value):
                self.assertEqual(
                    policy.classify(value),
                    Decision.UNTRUSTED,
                )

    def test_no_runtime_activation(self):
        runtime = (
            Path(__file__).resolve().parents[1]
            / "runtime.sh"
        ).read_text()
        self.assertNotIn(
            'RUNTIME_MODULES+=("searxng")',
            runtime,
        )

    def test_policy_contains_no_docker_or_service_calls(self):
        source = MODULE.read_text()
        tree = __import__("ast").parse(source)
        imports = [
            node for node in tree.body
            if isinstance(
                node, (__import__("ast").Import,
                       __import__("ast").ImportFrom)
            )
        ]
        self.assertEqual(len(imports), 2)
        self.assertNotIn("subprocess", source)
        self.assertNotIn("os.system", source)
        self.assertNotIn("docker ", source)
        self.assertNotIn("systemctl ", source)


if __name__ == "__main__":
    unittest.main()
