"""Tests for the non-mutating SearXNG first-boot adapter."""

import ast
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "runtime/modules"

sys.path.insert(0, str(MODULES))

from searxng_firstboot_policy import Action

spec = importlib.util.spec_from_file_location(
    "searxng_firstboot_adapter",
    MODULES / "searxng_firstboot_adapter.py",
)
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)

Disposition = adapter.Disposition


class AdapterTests(unittest.TestCase):

    def test_disabled_is_skipped(self):
        self.assertIs(
            adapter.classify(Action.SKIP),
            Disposition.SKIPPED,
        )

    def test_verified_is_preserved(self):
        self.assertIs(
            adapter.classify(Action.PRESERVE_VERIFIED),
            Disposition.VERIFIED,
        )

    def test_initialization_requires_authorization(self):
        self.assertIs(
            adapter.classify(Action.INITIALIZE_ELIGIBLE),
            Disposition.NEEDS_INITIALIZATION_AUTHORIZATION,
        )

    def test_provision_requires_authorization(self):
        self.assertIs(
            adapter.classify(Action.PROVISION_ELIGIBLE),
            Disposition.NEEDS_PROVISION_AUTHORIZATION,
        )

    def test_refusal_is_preserved(self):
        self.assertIs(
            adapter.classify(Action.REFUSE),
            Disposition.REFUSED,
        )

    def test_invalid_values_refused(self):
        for value in (None, True, False, 0, 1, "SKIP", object()):
            with self.subTest(value=repr(value)):
                self.assertIs(
                    adapter.classify(value),
                    Disposition.REFUSED,
                )

    def test_all_actions_have_explicit_results(self):
        expected = {
            Action.SKIP: Disposition.SKIPPED,
            Action.PRESERVE_VERIFIED: Disposition.VERIFIED,
            Action.INITIALIZE_ELIGIBLE:
                Disposition.NEEDS_INITIALIZATION_AUTHORIZATION,
            Action.PROVISION_ELIGIBLE:
                Disposition.NEEDS_PROVISION_AUTHORIZATION,
            Action.REFUSE: Disposition.REFUSED,
        }

        self.assertEqual(set(Action), set(expected))

        for action, result in expected.items():
            with self.subTest(action=action):
                self.assertIs(adapter.classify(action), result)

    def test_evaluate_skipped(self):
        with patch(
            "searxng_firstboot_decision.evaluate",
            return_value=Action.SKIP,
        ) as evaluate:
            result = adapter.assess(object(), object())

        self.assertIs(result, Disposition.SKIPPED)
        evaluate.assert_called_once()

    def test_evaluate_verified(self):
        with patch(
            "searxng_firstboot_decision.evaluate",
            return_value=Action.PRESERVE_VERIFIED,
        ):
            self.assertIs(
                adapter.assess(object(), object()),
                Disposition.VERIFIED,
            )

    def test_evaluate_initialization(self):
        with patch(
            "searxng_firstboot_decision.evaluate",
            return_value=Action.INITIALIZE_ELIGIBLE,
        ):
            self.assertIs(
                adapter.assess(object(), object()),
                Disposition.NEEDS_INITIALIZATION_AUTHORIZATION,
            )

    def test_evaluate_provision(self):
        with patch(
            "searxng_firstboot_decision.evaluate",
            return_value=Action.PROVISION_ELIGIBLE,
        ):
            self.assertIs(
                adapter.assess(object(), object()),
                Disposition.NEEDS_PROVISION_AUTHORIZATION,
            )

    def test_evaluate_refusal(self):
        with patch(
            "searxng_firstboot_decision.evaluate",
            return_value=Action.REFUSE,
        ):
            self.assertIs(
                adapter.assess(object(), object()),
                Disposition.REFUSED,
            )

    def test_evaluate_failure_refused(self):
        with patch(
            "searxng_firstboot_decision.evaluate",
            side_effect=RuntimeError("inspection failed"),
        ):
            self.assertIs(
                adapter.assess(object(), object()),
                Disposition.REFUSED,
            )

    def test_invalid_evaluate_result_refused(self):
        with patch(
            "searxng_firstboot_decision.evaluate",
            return_value="PROVISION_ELIGIBLE",
        ):
            self.assertIs(
                adapter.assess(object(), object()),
                Disposition.REFUSED,
            )

    def test_no_mutating_calls(self):
        source = (
            MODULES / "searxng_firstboot_adapter.py"
        ).read_text()

        tree = ast.parse(source)

        forbidden = {
            "subprocess", "docker", "systemctl",
            "mkdir", "makedirs", "unlink",
            "remove", "rename", "replace",
            "chmod", "chown", "open",
            "initialize", "provision",
            "start", "stop",
        }

        calls = [
            node.func
            for node in ast.walk(tree)
            if isinstance(node, ast.Call)
        ]

        for call in calls:
            if isinstance(call, ast.Name):
                self.assertNotIn(call.id, forbidden)
            elif isinstance(call, ast.Attribute):
                self.assertNotIn(call.attr, forbidden)

    def test_not_wired_into_firstboot(self):
        source = (ROOT / "runtime/firstboot.sh").read_text()
        self.assertNotIn(
            "searxng_firstboot_adapter",
            source,
        )

    def test_not_wired_into_runtime(self):
        source = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn(
            "searxng_firstboot_adapter",
            source,
        )

    def test_no_automatic_provisioning(self):
        source = (
            MODULES / "searxng_firstboot_adapter.py"
        ).read_text()

        self.assertNotIn("docker run", source)
        self.assertNotIn("docker create", source)
        self.assertNotIn("systemctl start", source)


class RetryAdapterTests(unittest.TestCase):

    def test_fresh_requires_independent_authorization(self):
        from searxng_retry_policy import Observation

        result = adapter.classify_retry(
            Observation("ABSENT", "ABSENT", "TRUSTED_EMPTY")
        )
        self.assertIs(
            result,
            Disposition.NEEDS_INITIALIZATION_AUTHORIZATION,
        )

    def test_registered_verified_is_observational(self):
        from searxng_retry_policy import Observation

        result = adapter.classify_retry(
            Observation(
                "REGISTERED_VERIFIED",
                "TRUSTED_ATTACHED",
                "TRUSTED_PRESENT",
            )
        )
        self.assertIs(result, Disposition.VERIFIED)

    def test_incomplete_and_untrusted_states_refused(self):
        from searxng_retry_policy import Observation

        cases = (
            ("ABSENT", "TRUSTED_EMPTY", "TRUSTED_EMPTY"),
            ("UNREGISTERED_STOPPED", "TRUSTED_EMPTY", "TRUSTED_EMPTY"),
            ("UNREGISTERED_STOPPED", "TRUSTED_ATTACHED", "TRUSTED_EMPTY"),
            ("ABSENT", "UNTRUSTED", "TRUSTED_EMPTY"),
            ("ABSENT", "ABSENT", "UNTRUSTED"),
        )

        for case in cases:
            with self.subTest(case=case):
                self.assertIs(
                    adapter.classify_retry(Observation(*case)),
                    Disposition.REFUSED,
                )

    def test_untyped_input_refused(self):
        for value in (None, "FRESH", {}, object()):
            with self.subTest(value=repr(value)):
                self.assertIs(
                    adapter.classify_retry(value),
                    Disposition.REFUSED,
                )


class TrustedObservationTests(unittest.TestCase):

    def observe(self, assessment, egress, registration):
        return adapter.observe_retry(
            lambda: assessment,
            lambda: egress,
            lambda: registration,
        )

    def test_fresh_requires_authorization(self):
        self.assertIs(
            self.observe("ABSENT", "ABSENT", "TRUSTED_EMPTY"),
            Disposition.NEEDS_INITIALIZATION_AUTHORIZATION,
        )

    def test_verified_is_observational(self):
        self.assertIs(
            self.observe(
                "REGISTERED_VERIFIED",
                "TRUSTED_ATTACHED",
                "TRUSTED_PRESENT",
            ),
            Disposition.VERIFIED,
        )

    def test_incomplete_network_refused(self):
        self.assertIs(
            self.observe(
                "ABSENT", "TRUSTED_EMPTY", "TRUSTED_EMPTY"
            ),
            Disposition.REFUSED,
        )

    def test_incomplete_container_refused(self):
        self.assertIs(
            self.observe(
                "UNREGISTERED_STOPPED",
                "TRUSTED_ATTACHED",
                "TRUSTED_EMPTY",
            ),
            Disposition.REFUSED,
        )

    def test_untrusted_state_refused(self):
        self.assertIs(
            self.observe("UNTRUSTED", "ABSENT", "TRUSTED_EMPTY"),
            Disposition.REFUSED,
        )

    def test_malformed_observation_refused(self):
        self.assertIs(
            self.observe(None, "ABSENT", "TRUSTED_EMPTY"),
            Disposition.REFUSED,
        )

    def test_provider_failure_refused(self):
        def fail():
            raise RuntimeError("Observation failed")

        self.assertIs(
            adapter.observe_retry(
                fail, lambda: "ABSENT",
                lambda: "TRUSTED_EMPTY",
            ),
            Disposition.REFUSED,
        )

    def test_no_observation_authorizes_execution(self):
        for assessment, egress, registration in (
            ("ABSENT", "ABSENT", "TRUSTED_EMPTY"),
            ("REGISTERED_VERIFIED", "TRUSTED_ATTACHED",
             "TRUSTED_PRESENT"),
            ("UNREGISTERED_STOPPED", "TRUSTED_EMPTY",
             "TRUSTED_EMPTY"),
        ):
            result = self.observe(
                assessment, egress, registration
            )
            self.assertNotEqual(
                result,
                Disposition.NEEDS_PROVISION_AUTHORIZATION,
            )


if __name__ == "__main__":
    unittest.main()
