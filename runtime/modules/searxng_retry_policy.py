#!/usr/bin/env python3
"""Pure, fail-closed SearXNG first-boot retry classification.

This module does not inspect or mutate Docker, the filesystem,
registrations, or services. Trusted callers must independently
obtain and validate observations before calling classify().

No decision authorizes deletion, adoption, startup or repair.
"""

from dataclasses import dataclass
from enum import Enum


class Decision(str, Enum):
    FRESH = "FRESH"
    REGISTERED_VERIFIED = "REGISTERED_VERIFIED"
    INCOMPLETE_NETWORK = "INCOMPLETE_NETWORK"
    INCOMPLETE_CONTAINER = "INCOMPLETE_CONTAINER"
    UNTRUSTED = "UNTRUSTED"


@dataclass(frozen=True)
class Observation:
    assessment: str
    egress_state: str
    registration_directory: str


ASSESSMENTS = frozenset({
    "ABSENT",
    "REGISTERED_VERIFIED",
    "UNREGISTERED_STOPPED",
    "UNTRUSTED",
})

EGRESS_STATES = frozenset({
    "ABSENT",
    "TRUSTED_EMPTY",
    "TRUSTED_ATTACHED",
    "UNTRUSTED",
})

REGISTRATION_DIRECTORIES = frozenset({
    "TRUSTED_EMPTY",
    "TRUSTED_PRESENT",
    "UNTRUSTED",
})


def classify(observation: Observation) -> Decision:
    """Classify verified observations without taking any action.

    FRESH is the only state eligible for a future create-only
    provisioning path, subject to independent reauthorization.
    Even FRESH does not itself authorize mutation.
    """

    if type(observation) is not Observation:
        return Decision.UNTRUSTED

    assessment = observation.assessment
    egress = observation.egress_state
    registration = observation.registration_directory

    if (
        type(assessment) is not str
        or type(egress) is not str
        or type(registration) is not str
    ):
        return Decision.UNTRUSTED

    if (
        assessment not in ASSESSMENTS
        or egress not in EGRESS_STATES
        or registration not in REGISTRATION_DIRECTORIES
    ):
        return Decision.UNTRUSTED

    if (
        assessment == "UNTRUSTED"
        or egress == "UNTRUSTED"
        or registration == "UNTRUSTED"
    ):
        return Decision.UNTRUSTED

    if assessment == "ABSENT":
        if registration != "TRUSTED_EMPTY":
            return Decision.UNTRUSTED
        if egress == "ABSENT":
            return Decision.FRESH
        if egress == "TRUSTED_EMPTY":
            return Decision.INCOMPLETE_NETWORK
        return Decision.UNTRUSTED

    if assessment == "REGISTERED_VERIFIED":
        if (
            registration == "TRUSTED_PRESENT"
            and egress == "TRUSTED_ATTACHED"
        ):
            return Decision.REGISTERED_VERIFIED
        return Decision.UNTRUSTED

    if assessment == "UNREGISTERED_STOPPED":
        if (
            registration == "TRUSTED_EMPTY"
            and egress in ("TRUSTED_EMPTY", "TRUSTED_ATTACHED")
        ):
            return Decision.INCOMPLETE_CONTAINER
        return Decision.UNTRUSTED

    return Decision.UNTRUSTED


def may_create_fresh(decision: Decision) -> bool:
    """Eligibility only; callers must reauthorize before mutation."""
    return type(decision) is Decision and decision is Decision.FRESH
