
"""Firstboot initialization/recovery boundary regressions."""

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

MODULES = Path(__file__).resolve().parents[1] / "modules"
sys.path.insert(0, str(MODULES))

import searxng_firstboot_decision as decision
from searxng_bootstrap import BootstrapDecision
from searxng_firstboot_policy import Action
from searxng_retry_policy import Decision


class FirstbootBoundaryTests(unittest.TestCase):

    def evaluate(self, bootstrap, observation=None):
        with (
            patch.object(decision.os, "geteuid", return_value=0),
            patch.object(
                decision.searxng_config, "enabled",
                return_value=True,
            ),
            patch.object(
                decision.searxng_bootstrap, "decide",
                return_value=bootstrap,
            ),
            patch.object(
                decision.searxng_observation, "observe",
                return_value=observation,
            ) as observe,
        ):
            result = decision.evaluate(object(), object())
        return result, observe

    def test_missing_directory_only_initialization_eligible(self):
        result, observe = self.evaluate(
            BootstrapDecision.INITIALIZE_ELIGIBLE
        )
        self.assertIs(result, Action.INITIALIZE_ELIGIBLE)
        observe.assert_not_called()

    def test_existing_fresh_directory_provision_eligible(self):
        result, observe = self.evaluate(
            BootstrapDecision.OBSERVE_EXISTING,
            Decision.FRESH,
        )
        self.assertIs(result, Action.PROVISION_ELIGIBLE)
        observe.assert_called_once()

    def test_existing_verified_registration_preserved(self):
        result, _ = self.evaluate(
            BootstrapDecision.OBSERVE_EXISTING,
            Decision.REGISTERED_VERIFIED,
        )
        self.assertIs(result, Action.PRESERVE_VERIFIED)

    def test_interrupted_network_refused(self):
        result, _ = self.evaluate(
            BootstrapDecision.OBSERVE_EXISTING,
            Decision.INCOMPLETE_NETWORK,
        )
        self.assertIs(result, Action.REFUSE)

    def test_interrupted_container_refused(self):
        result, _ = self.evaluate(
            BootstrapDecision.OBSERVE_EXISTING,
            Decision.INCOMPLETE_CONTAINER,
        )
        self.assertIs(result, Action.REFUSE)

    def test_untrusted_bootstrap_refused_without_observation(self):
        result, observe = self.evaluate(
            BootstrapDecision.UNTRUSTED
        )
        self.assertIs(result, Action.REFUSE)
        observe.assert_not_called()

    def test_untrusted_observation_refused(self):
        result, _ = self.evaluate(
            BootstrapDecision.OBSERVE_EXISTING,
            Decision.UNTRUSTED,
        )
        self.assertIs(result, Action.REFUSE)

    def test_observation_exception_refused(self):
        with (
            patch.object(decision.os, "geteuid", return_value=0),
            patch.object(
                decision.searxng_config, "enabled",
                return_value=True,
            ),
            patch.object(
                decision.searxng_bootstrap, "decide",
                return_value=BootstrapDecision.OBSERVE_EXISTING,
            ),
            patch.object(
                decision.searxng_observation, "observe",
                side_effect=RuntimeError("inspection failed"),
            ),
        ):
            self.assertIs(
                decision.evaluate(object(), object()),
                Action.REFUSE,
            )

    def test_nonroot_refused(self):
        with patch.object(
            decision.os, "geteuid", return_value=1000
        ):
            self.assertIs(
                decision.evaluate(object(), object()),
                Action.REFUSE,
            )


if __name__ == "__main__":
    unittest.main()
