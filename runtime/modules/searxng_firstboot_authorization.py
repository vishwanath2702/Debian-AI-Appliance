"""Pure SearXNG first-boot authorization policy.

Eligibility is not authorization.

This module accepts only explicitly typed authorization decisions.
It does not obtain credentials, grant authorization, or mutate resources.
"""

from dataclasses import dataclass
from enum import Enum, auto

from searxng_firstboot_plan import Plan, Requirement


class Grant(Enum):
    NONE = auto()
    INITIALIZE = auto()
    PROVISION = auto()
    VERIFY = auto()


class Verdict(Enum):
    SKIP = auto()
    AUTHORIZED_INITIALIZATION = auto()
    AUTHORIZED_PROVISIONING = auto()
    AUTHORIZED_VERIFICATION = auto()
    REFUSE = auto()


@dataclass(frozen=True)
class Authorization:
    verdict: Verdict
    may_mark_complete: bool


def authorize(plan, grant):
    """Evaluate a plan and independently supplied grant.

    A Grant is a typed input, not proof of an authenticated principal.
    The eventual execution boundary must validate its own authority.
    """
    refusal = Authorization(Verdict.REFUSE, False)

    if type(plan) is not Plan:
        return refusal

    if type(plan.requirement) is not Requirement:
        return refusal

    if type(plan.may_mark_complete) is not bool:
        return refusal

    if type(grant) is not Grant:
        return refusal

    if plan.requirement is Requirement.NONE:
        if plan.may_mark_complete and grant is Grant.NONE:
            return Authorization(Verdict.SKIP, True)
        return refusal

    if plan.may_mark_complete:
        return refusal

    if (
        plan.requirement is Requirement.AUTHORIZE_INITIALIZATION
        and grant is Grant.INITIALIZE
    ):
        return Authorization(
            Verdict.AUTHORIZED_INITIALIZATION, False
        )

    if (
        plan.requirement is Requirement.AUTHORIZE_PROVISIONING
        and grant is Grant.PROVISION
    ):
        return Authorization(
            Verdict.AUTHORIZED_PROVISIONING, False
        )

    if (
        plan.requirement is Requirement.VERIFY_EXISTING
        and grant is Grant.VERIFY
    ):
        return Authorization(
            Verdict.AUTHORIZED_VERIFICATION, False
        )

    return refusal
