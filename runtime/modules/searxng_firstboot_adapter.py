"""Read-only SearXNG first-boot integration boundary.

This module classifies typed first-boot decisions.
It never initializes directories, provisions Docker resources,
starts containers, or authorizes those operations.

Eligibility is not authorization.
"""

from enum import Enum, auto

from searxng_firstboot_policy import Action


class Disposition(Enum):
    SKIPPED = auto()
    VERIFIED = auto()
    NEEDS_INITIALIZATION_AUTHORIZATION = auto()
    NEEDS_PROVISION_AUTHORIZATION = auto()
    REFUSED = auto()


def classify(action):
    """Classify an exact Action value without side effects."""

    if type(action) is not Action:
        return Disposition.REFUSED

    if action is Action.SKIP:
        return Disposition.SKIPPED

    if action is Action.PRESERVE_VERIFIED:
        return Disposition.VERIFIED

    if action is Action.INITIALIZE_ELIGIBLE:
        return Disposition.NEEDS_INITIALIZATION_AUTHORIZATION

    if action is Action.PROVISION_ELIGIBLE:
        return Disposition.NEEDS_PROVISION_AUTHORIZATION

    return Disposition.REFUSED


def assess(assessor, registration):
    """Evaluate the existing policy and classify its result.

    The decision module remains the sole policy authority.
    No mutation is permitted by this adapter.
    """
    try:
        from searxng_firstboot_decision import evaluate

        return classify(evaluate(assessor, registration))
    except Exception:
        return Disposition.REFUSED


def classify_retry(observation):
    """Classify a trusted observation without granting recovery actions.

    Incomplete and untrusted states always require separate recovery
    authorization. A verified state is observational only.
    """
    try:
        from searxng_retry_policy import (
            Decision, Observation, classify,
        )

        if type(observation) is not Observation:
            return Disposition.REFUSED

        decision = classify(observation)

        if decision is Decision.REGISTERED_VERIFIED:
            return Disposition.VERIFIED

        if decision is Decision.FRESH:
            return Disposition.NEEDS_INITIALIZATION_AUTHORIZATION

        return Disposition.REFUSED

    except Exception:
        return Disposition.REFUSED


def observe_retry(assessment_provider, egress_provider,
                  registration_provider):
    """Classify independently obtained recovery observations.

    Providers are observation-only dependencies supplied by the trusted
    caller. They are not grants and never authorize resource mutation.
    Any observation failure is refused.
    """
    try:
        from searxng_retry_policy import Observation

        assessment = assessment_provider()
        egress = egress_provider()
        registration = registration_provider()

        if not all(
            type(value) is str
            for value in (assessment, egress, registration)
        ):
            return Disposition.REFUSED

        return classify_retry(
            Observation(assessment, egress, registration)
        )
    except Exception:
        return Disposition.REFUSED
