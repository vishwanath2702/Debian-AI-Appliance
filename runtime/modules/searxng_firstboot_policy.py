"""Pure, typed, fail-closed SearXNG first-boot decision policy.

Eligibility never authorizes filesystem, Docker, or service mutations.
"""

from enum import Enum

from searxng_bootstrap import BootstrapDecision
from searxng_retry_policy import Decision


class Action(Enum):
    SKIP = "skip"
    INITIALIZE_ELIGIBLE = "initialize_eligible"
    PROVISION_ELIGIBLE = "provision_eligible"
    PRESERVE_VERIFIED = "preserve_verified"
    REFUSE = "refuse"


def decide(enabled, bootstrap, observation=None):
    """Evaluate typed bootstrap and retry-policy decisions."""
    if type(enabled) is not bool:
        return Action.REFUSE

    if not enabled:
        return Action.SKIP

    if type(bootstrap) is not BootstrapDecision:
        return Action.REFUSE

    if bootstrap is BootstrapDecision.INITIALIZE_ELIGIBLE:
        if observation is None:
            return Action.INITIALIZE_ELIGIBLE
        return Action.REFUSE

    if bootstrap is not BootstrapDecision.OBSERVE_EXISTING:
        return Action.REFUSE

    if type(observation) is not Decision:
        return Action.REFUSE

    if observation is Decision.FRESH:
        return Action.PROVISION_ELIGIBLE

    if observation is Decision.REGISTERED_VERIFIED:
        return Action.PRESERVE_VERIFIED

    return Action.REFUSE
