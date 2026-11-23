"""Pure SearXNG first-boot execution planning.

Plans describe required next steps; they never authorize or execute them.
No filesystem, Docker, systemd, or network mutation occurs here.
"""

from dataclasses import dataclass
from enum import Enum, auto

from searxng_firstboot_adapter import Disposition


class Requirement(Enum):
    NONE = auto()
    AUTHORIZE_INITIALIZATION = auto()
    AUTHORIZE_PROVISIONING = auto()
    VERIFY_EXISTING = auto()
    REFUSE = auto()


@dataclass(frozen=True)
class Plan:
    requirement: Requirement
    may_mark_complete: bool


def plan(disposition):
    """Return a strictly typed, non-authorizing execution plan."""
    if type(disposition) is not Disposition:
        return Plan(Requirement.REFUSE, False)

    if disposition is Disposition.SKIPPED:
        return Plan(Requirement.NONE, True)

    if disposition is Disposition.VERIFIED:
        return Plan(Requirement.VERIFY_EXISTING, False)

    if disposition is Disposition.NEEDS_INITIALIZATION_AUTHORIZATION:
        return Plan(Requirement.AUTHORIZE_INITIALIZATION, False)

    if disposition is Disposition.NEEDS_PROVISION_AUTHORIZATION:
        return Plan(Requirement.AUTHORIZE_PROVISIONING, False)

    return Plan(Requirement.REFUSE, False)


def assess_plan(assessor, registration):
    """Classify using the read-only adapter and derive a plan."""
    try:
        from searxng_firstboot_adapter import assess
        return plan(assess(assessor, registration))
    except Exception:
        return Plan(Requirement.REFUSE, False)
