"""Typed SearXNG first-boot policy integration tests."""

from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "runtime/modules"

sys.path.insert(0, str(MODULES))

from searxng_bootstrap import BootstrapDecision
from searxng_retry_policy import Decision
from searxng_firstboot_policy import Action, decide


class PolicyTests(unittest.TestCase):

    def test_disabled_skips(self):
        self.assertIs(
            decide(False, BootstrapDecision.UNTRUSTED),
            Action.SKIP,
        )

    def test_initialization_eligible(self):
        self.assertIs(
            decide(True, BootstrapDecision.INITIALIZE_ELIGIBLE),
            Action.INITIALIZE_ELIGIBLE,
        )

    def test_existing_fresh_eligible(self):
        self.assertIs(
            decide(
                True,
                BootstrapDecision.OBSERVE_EXISTING,
                Decision.FRESH,
            ),
            Action.PROVISION_ELIGIBLE,
        )

    def test_existing_verified_preserved(self):
        self.assertIs(
            decide(
                True,
                BootstrapDecision.OBSERVE_EXISTING,
                Decision.REGISTERED_VERIFIED,
            ),
            Action.PRESERVE_VERIFIED,
        )

    def test_untrusted_refused(self):
        self.assertIs(
            decide(True, BootstrapDecision.UNTRUSTED),
            Action.REFUSE,
        )

    def test_incomplete_network_refused(self):
        self.assertIs(
            decide(
                True,
                BootstrapDecision.OBSERVE_EXISTING,
                Decision.INCOMPLETE_NETWORK,
            ),
            Action.REFUSE,
        )

    def test_incomplete_container_refused(self):
        self.assertIs(
            decide(
                True,
                BootstrapDecision.OBSERVE_EXISTING,
                Decision.INCOMPLETE_CONTAINER,
            ),
            Action.REFUSE,
        )

    def test_missing_observation_refused(self):
        self.assertIs(
            decide(True, BootstrapDecision.OBSERVE_EXISTING),
            Action.REFUSE,
        )

    def test_initialization_with_observation_refused(self):
        self.assertIs(
            decide(
                True,
                BootstrapDecision.INITIALIZE_ELIGIBLE,
                Decision.FRESH,
            ),
            Action.REFUSE,
        )

    def test_nonboolean_flag_refused(self):
        for value in (None, 1, 0, "true", "false"):
            self.assertIs(
                decide(
                    value,
                    BootstrapDecision.OBSERVE_EXISTING,
                    Decision.FRESH,
                ),
                Action.REFUSE,
            )

    def test_string_bootstrap_refused(self):
        self.assertIs(
            decide(True, "OBSERVE_EXISTING", Decision.FRESH),
            Action.REFUSE,
        )

    def test_string_observation_refused(self):
        self.assertIs(
            decide(
                True,
                BootstrapDecision.OBSERVE_EXISTING,
                "FRESH",
            ),
            Action.REFUSE,
        )

    def test_foreign_enum_refused(self):
        from enum import Enum

        class Foreign(Enum):
            FRESH = "fresh"

        self.assertIs(
            decide(
                True,
                BootstrapDecision.OBSERVE_EXISTING,
                Foreign.FRESH,
            ),
            Action.REFUSE,
        )

    def test_every_retry_decision(self):
        expected = {
            Decision.FRESH: Action.PROVISION_ELIGIBLE,
            Decision.REGISTERED_VERIFIED: Action.PRESERVE_VERIFIED,
        }

        for value in Decision:
            with self.subTest(decision=value):
                self.assertIs(
                    decide(
                        True,
                        BootstrapDecision.OBSERVE_EXISTING,
                        value,
                    ),
                    expected.get(value, Action.REFUSE),
                )

    def test_every_bootstrap_decision(self):
        for value in BootstrapDecision:
            with self.subTest(bootstrap=value):
                expected = (
                    Action.INITIALIZE_ELIGIBLE
                    if value is BootstrapDecision.INITIALIZE_ELIGIBLE
                    else Action.REFUSE
                )
                self.assertIs(
                    decide(True, value),
                    expected,
                )

    def test_no_mutation_calls(self):
        text = (
            MODULES / "searxng_firstboot_policy.py"
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
            self.assertNotIn(forbidden, text)

    def test_firstboot_not_activated(self):
        source = (ROOT / "runtime/firstboot.sh").read_text()
        self.assertNotIn("searxng_firstboot_policy", source)

    def test_runtime_not_activated(self):
        source = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn("searxng_firstboot_policy", source)


if __name__ == "__main__":
    unittest.main()
