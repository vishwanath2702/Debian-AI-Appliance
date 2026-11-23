"""Read-only SearXNG first-boot decision orchestration.

An Action expresses eligibility, never permission to mutate resources.
The assessor and registration dependencies are supplied explicitly.
"""

import os

import searxng_config
import searxng_bootstrap
import searxng_observation

from searxng_bootstrap import BootstrapDecision
from searxng_firstboot_policy import Action, decide


def evaluate(assessor, registration):
    """Evaluate trusted installed state without changing it."""
    if os.geteuid() != 0:
        return Action.REFUSE

    try:
        enabled = searxng_config.enabled()

        if type(enabled) is not bool:
            return Action.REFUSE

        if not enabled:
            return Action.SKIP

        bootstrap = searxng_bootstrap.decide()

        if type(bootstrap) is not BootstrapDecision:
            return Action.REFUSE

        if bootstrap is BootstrapDecision.INITIALIZE_ELIGIBLE:
            return decide(True, bootstrap)

        if bootstrap is not BootstrapDecision.OBSERVE_EXISTING:
            return Action.REFUSE

        observation = searxng_observation.observe(
            assessor, registration
        )

        return decide(True, bootstrap, observation)

    except Exception:
        return Action.REFUSE
