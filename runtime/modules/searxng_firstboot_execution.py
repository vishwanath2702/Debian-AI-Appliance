"""Fail-closed SearXNG firstboot execution gate.

This is not an executor. No caller-provided grant or advisory assessment
can authorize filesystem, Docker, service, or registration mutations.
"""

import os
from dataclasses import dataclass
from enum import Enum, auto

from searxng_firstboot_authority import Assessment, Status


class Decision(Enum):
    SKIP = auto()
    REFUSE = auto()


@dataclass(frozen=True)
class Result:
    decision: Decision
    execution_authorized: bool = False
    may_mark_searxng_complete: bool = False


REFUSAL = Result(Decision.REFUSE)


def evaluate(assessment):
    """Classify an advisory assessment without granting execution."""
    if os.geteuid() != 0:
        return REFUSAL

    if type(assessment) is not Assessment:
        return REFUSAL

    if type(assessment.status) is not Status:
        return REFUSAL

    if type(assessment.execution_authorized) is not bool:
        return REFUSAL

    if type(assessment.may_mark_complete) is not bool:
        return REFUSAL

    if assessment.execution_authorized:
        return REFUSAL

    if (
        assessment.status is Status.SKIPPED
        and assessment.may_mark_complete
    ):
        return Result(Decision.SKIP)

    return REFUSAL
