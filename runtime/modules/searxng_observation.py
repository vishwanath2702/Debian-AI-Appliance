#!/usr/bin/env python3
"""Read-only SearXNG observation adapter.

Uses independently supplied, trusted assessor and registration
implementations. No provisioning, adoption, deletion, or startup.

The returned decision is informational only.
"""

import json
import os
import re

from searxng_retry_policy import (
    Observation,
    Decision,
    classify,
)

EGRESS = "daia-searxng-egress"
VALID_ID = re.compile(r"[0-9a-f]{64}\Z")


class ObservationError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise ObservationError(message)


def trusted_registration_state(registration):
    """Use existing descriptor-based directory and file checks."""

    fd = registration._open_trusted_directory(
        registration.DEFAULT_DIRECTORY
    )

    try:
        try:
            identity = registration._read_registration_fd(fd)
        except FileNotFoundError as error:
            require(
                error.filename == registration.REGISTRATION_NAME,
                "Ambiguous registration absence",
            )
            return "TRUSTED_EMPTY", None

        require(
            isinstance(identity, str)
            and VALID_ID.fullmatch(identity) is not None,
            "Invalid registered identity",
        )

        return "TRUSTED_PRESENT", identity

    finally:
        os.close(fd)


def egress_state(assessor, assessment, registered_id):
    """Inspect network inventory and independently verify metadata.

    A failed inventory or malformed inspection is never absence.
    """

    inventory = assessor.docker(
        "network", "ls", "--format", "{{.Name}}"
    )

    require(
        isinstance(inventory, str),
        "Malformed network inventory",
    )

    names = inventory.splitlines()

    require(
        len(names) == len(set(names)),
        "Duplicate network inventory entries",
    )

    initially_present = EGRESS in names

    def verify_inventory_stability():
        repeated = assessor.docker(
            "network", "ls", "--format", "{{.Name}}"
        )

        require(
            isinstance(repeated, str),
            "Malformed repeated network inventory",
        )

        repeated_names = repeated.splitlines()

        require(
            len(repeated_names) == len(set(repeated_names)),
            "Duplicate repeated network entries",
        )

        require(
            (EGRESS in repeated_names) == initially_present,
            "Egress network presence changed during observation",
        )

    if not initially_present:
        verify_inventory_stability()
        return "ABSENT"

    def inspect():
        result = json.loads(
            assessor.docker("network", "inspect", EGRESS)
        )

        require(
            isinstance(result, list) and len(result) == 1,
            "Malformed egress inspection",
        )

        network = result[0]

        require(
            isinstance(network, dict),
            "Malformed egress network",
        )

        return network

    network = inspect()

    require(
        network.get("Name") == EGRESS,
        "Egress name mismatch",
    )

    require(
        network.get("Driver") == "bridge",
        "Egress driver mismatch",
    )

    require(
        network.get("Internal") is False,
        "Egress isolation mismatch",
    )

    labels = network.get("Labels")

    require(
        isinstance(labels, dict)
        and labels.get("org.daia.component") == "searxng"
        and labels.get("org.daia.managed") == "true",
        "Egress ownership mismatch",
    )

    identity = network.get("Id")

    require(
        isinstance(identity, str)
        and VALID_ID.fullmatch(identity) is not None,
        "Invalid egress identity",
    )

    members = network.get("Containers")

    require(
        isinstance(members, dict),
        "Malformed egress membership",
    )

    for member_id in members:
        require(
            isinstance(member_id, str)
            and VALID_ID.fullmatch(member_id) is not None,
            "Malformed egress member identity",
        )

    if assessment == "REGISTERED_VERIFIED":
        require(
            registered_id is not None,
            "Missing verified registration",
        )

        require(
            set(members).issubset({registered_id}),
            "Unexpected egress member",
        )

        # The assessor has already verified the registered
        # container's network attachment. Docker may omit stopped
        # containers from active network membership.
        state = "TRUSTED_ATTACHED"

    else:
        require(
            not members,
            "Unexpected unregistered egress member",
        )

        state = "TRUSTED_EMPTY"

    # Reject metadata changes during observation.
    require(
        inspect() == network,
        "Egress metadata changed during inspection",
    )

    verify_inventory_stability()

    return state


def observe(assessor, registration):
    """Produce a conservative classification without mutation."""

    if os.geteuid() != 0:
        return Decision.UNTRUSTED

    try:
        assessment = assessor.assess(registration)

        require(
            assessment in (
                "ABSENT",
                "REGISTERED_VERIFIED",
                "UNREGISTERED_STOPPED",
            ),
            "Recovery assessor refused",
        )

        directory_state, registered_id = (
            trusted_registration_state(registration)
        )

        if assessment == "REGISTERED_VERIFIED":
            require(
                registered_id is not None,
                "Verified container lacks registration",
            )
        else:
            require(
                registered_id is None,
                "Unexpected registration",
            )

        network_state = egress_state(
            assessor,
            assessment,
            registered_id,
        )

        # Repeat trusted observations to reject detectable drift.
        again = assessor.assess(registration)

        require(
            again == assessment,
            "Recovery assessment changed",
        )

        directory_again, identity_again = (
            trusted_registration_state(registration)
        )

        require(
            (directory_again, identity_again)
            == (directory_state, registered_id),
            "Registration changed during observation",
        )

        observation = Observation(
            assessment=assessment,
            egress_state=network_state,
            registration_directory=directory_state,
        )

        return classify(observation)

    except (
        ObservationError,
        OSError,
        ValueError,
        TypeError,
        KeyError,
        AttributeError,
        RuntimeError,
    ):
        return Decision.UNTRUSTED
