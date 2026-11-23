"""Read-only SearXNG first-boot authority assessment.

Results are advisory, never execution capabilities.
A caller-supplied grant is not an authenticated credential.
"""
import os
from dataclasses import dataclass
from enum import Enum, auto

from searxng_firstboot_plan import Plan, Requirement, assess_plan
from searxng_firstboot_authorization import Grant, Verdict, authorize


class Status(Enum):
    SKIPPED = auto()
    VERIFIED_CANDIDATE = auto()
    INITIALIZATION_CANDIDATE = auto()
    PROVISIONING_CANDIDATE = auto()
    REFUSED = auto()


@dataclass(frozen=True)
class Assessment:
    status: Status
    execution_authorized: bool = False
    may_mark_complete: bool = False


REFUSAL = Assessment(Status.REFUSED)


def evaluate(assessor, registration, grant=Grant.NONE):
    """Obtain a fresh, non-authorizing first-boot assessment."""
    if os.geteuid() != 0 or type(grant) is not Grant:
        return REFUSAL

    try:
        plan = assess_plan(assessor, registration)

        if type(plan) is not Plan:
            return REFUSAL
        if type(plan.requirement) is not Requirement:
            return REFUSAL
        if type(plan.may_mark_complete) is not bool:
            return REFUSAL

        result = authorize(plan, grant)

        if result.verdict is Verdict.SKIP:
            return Assessment(Status.SKIPPED, False, True)
        if result.verdict is Verdict.AUTHORIZED_VERIFICATION:
            return Assessment(Status.VERIFIED_CANDIDATE)
        if result.verdict is Verdict.AUTHORIZED_INITIALIZATION:
            return Assessment(Status.INITIALIZATION_CANDIDATE)
        if result.verdict is Verdict.AUTHORIZED_PROVISIONING:
            return Assessment(Status.PROVISIONING_CANDIDATE)

        return REFUSAL
    except Exception:
        return REFUSAL
