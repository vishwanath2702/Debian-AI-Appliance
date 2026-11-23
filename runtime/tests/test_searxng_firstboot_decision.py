"""Tests for read-only SearXNG first-boot decisions."""

from pathlib import Path
from unittest import mock
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "runtime/modules"
sys.path.insert(0, str(MODULES))

import searxng_firstboot_decision as target

from searxng_bootstrap import BootstrapDecision
from searxng_retry_policy import Decision
from searxng_firstboot_policy import Action


class DecisionTests(unittest.TestCase):

    def evaluate(self, enabled, bootstrap, observation=None):
        assessor = object()
        registration = object()

        with (
            mock.patch.object(
                target.os, "geteuid", return_value=0
            ),
            mock.patch.object(
                target.searxng_config,
                "enabled", return_value=enabled
            ) as config,
            mock.patch.object(
                target.searxng_bootstrap,
                "decide", return_value=bootstrap
            ) as boot,
            mock.patch.object(
                target.searxng_observation,
                "observe", return_value=observation
            ) as observe,
        ):
            result = target.evaluate(assessor, registration)

        return result, config, boot, observe, assessor, registration

    def test_disabled_skips(self):
        result, config, boot, observe, _, _ = self.evaluate(
            False, BootstrapDecision.UNTRUSTED
        )
        self.assertIs(result, Action.SKIP)
        config.assert_called_once_with()
        boot.assert_not_called()
        observe.assert_not_called()

    def test_initialization_eligible(self):
        result, _, boot, observe, _, _ = self.evaluate(
            True, BootstrapDecision.INITIALIZE_ELIGIBLE
        )
        self.assertIs(result, Action.INITIALIZE_ELIGIBLE)
        boot.assert_called_once_with()
        observe.assert_not_called()

    def test_existing_fresh_eligible(self):
        result, _, _, observe, assessor, registration = (
            self.evaluate(
                True, BootstrapDecision.OBSERVE_EXISTING,
                Decision.FRESH
            )
        )
        self.assertIs(result, Action.PROVISION_ELIGIBLE)
        observe.assert_called_once_with(
            assessor, registration
        )

    def test_existing_verified_preserved(self):
        result, _, _, _, _, _ = self.evaluate(
            True, BootstrapDecision.OBSERVE_EXISTING,
            Decision.REGISTERED_VERIFIED
        )
        self.assertIs(result, Action.PRESERVE_VERIFIED)

    def test_incomplete_network_refused(self):
        result, _, _, _, _, _ = self.evaluate(
            True, BootstrapDecision.OBSERVE_EXISTING,
            Decision.INCOMPLETE_NETWORK
        )
        self.assertIs(result, Action.REFUSE)

    def test_incomplete_container_refused(self):
        result, _, _, _, _, _ = self.evaluate(
            True, BootstrapDecision.OBSERVE_EXISTING,
            Decision.INCOMPLETE_CONTAINER
        )
        self.assertIs(result, Action.REFUSE)

    def test_untrusted_observation_refused(self):
        result, _, _, _, _, _ = self.evaluate(
            True, BootstrapDecision.OBSERVE_EXISTING,
            Decision.UNTRUSTED
        )
        self.assertIs(result, Action.REFUSE)

    def test_untrusted_bootstrap_refused(self):
        result, _, _, observe, _, _ = self.evaluate(
            True, BootstrapDecision.UNTRUSTED
        )
        self.assertIs(result, Action.REFUSE)
        observe.assert_not_called()

    def test_invalid_config_refused(self):
        result, _, boot, _, _, _ = self.evaluate(
            "true", BootstrapDecision.OBSERVE_EXISTING
        )
        self.assertIs(result, Action.REFUSE)
        boot.assert_not_called()

    def test_invalid_bootstrap_refused(self):
        result, _, _, observe, _, _ = self.evaluate(
            True, "OBSERVE_EXISTING", Decision.FRESH
        )
        self.assertIs(result, Action.REFUSE)
        observe.assert_not_called()

    def test_invalid_observation_refused(self):
        result, _, _, _, _, _ = self.evaluate(
            True, BootstrapDecision.OBSERVE_EXISTING,
            "FRESH"
        )
        self.assertIs(result, Action.REFUSE)

    def test_nonroot_refused_before_config(self):
        with (
            mock.patch.object(
                target.os, "geteuid", return_value=1000
            ),
            mock.patch.object(
                target.searxng_config, "enabled"
            ) as config,
        ):
            self.assertIs(
                target.evaluate(object(), object()),
                Action.REFUSE
            )
            config.assert_not_called()

    def test_config_failure_refused(self):
        with (
            mock.patch.object(
                target.os, "geteuid", return_value=0
            ),
            mock.patch.object(
                target.searxng_config, "enabled",
                side_effect=ValueError("unsafe")
            ),
        ):
            self.assertIs(
                target.evaluate(object(), object()),
                Action.REFUSE
            )

    def test_bootstrap_failure_refused(self):
        with (
            mock.patch.object(
                target.os, "geteuid", return_value=0
            ),
            mock.patch.object(
                target.searxng_config, "enabled",
                return_value=True
            ),
            mock.patch.object(
                target.searxng_bootstrap, "decide",
                side_effect=OSError("unsafe")
            ),
        ):
            self.assertIs(
                target.evaluate(object(), object()),
                Action.REFUSE
            )

    def test_observation_failure_refused(self):
        with (
            mock.patch.object(
                target.os, "geteuid", return_value=0
            ),
            mock.patch.object(
                target.searxng_config, "enabled",
                return_value=True
            ),
            mock.patch.object(
                target.searxng_bootstrap, "decide",
                return_value=BootstrapDecision.OBSERVE_EXISTING
            ),
            mock.patch.object(
                target.searxng_observation, "observe",
                side_effect=OSError("unsafe")
            ),
        ):
            self.assertIs(
                target.evaluate(object(), object()),
                Action.REFUSE
            )

    def test_no_mutation_interfaces(self):
        source = MODULE.read_text() if False else (
            MODULES / "searxng_firstboot_decision.py"
        ).read_text()

        for forbidden in (
            "subprocess",
            "os.system",
            "docker ",
            "mkdir(",
            "unlink(",
            "rmtree(",
            "systemctl",
        ):
            self.assertNotIn(forbidden, source)

    def test_firstboot_not_activated(self):
        self.assertNotIn(
            "searxng_firstboot_decision",
            (ROOT / "runtime/firstboot.sh").read_text()
        )

    def test_runtime_not_activated(self):
        self.assertNotIn(
            "searxng_firstboot_decision",
            (ROOT / "runtime/runtime.sh").read_text()
        )


if __name__ == "__main__":
    unittest.main()
