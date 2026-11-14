#!/usr/bin/env python3
"""Trusted persistent container registration for DAIA Apache Tika.

This module provides registration primitives only. Callers must
independently verify Docker identity, ownership, security settings,
network configuration, and stopped state before publication.
"""

from pgvector_registration import (
    publish_trusted_registration,
    read_trusted_registration,
    validate_container_id,
    verify_trusted_registration,
)

STATE_DIRECTORY = "/var/lib/daia/tika"

EXPECTED_IMAGE = (
    "sha256:3f1999aaaa706a5427e8c0d3139b30ba5973e5e0489a43f4f7fd2491a4172d6a"
)


def authorize_container_registration(
    container_id: str,
    inspected_image: str,
    labels: dict,
    network_mode: str,
    security_options: list,
    privileged: bool,
) -> str:
    """Validate independently inspected, immutable Docker metadata."""
    validated_id = validate_container_id(container_id)

    if inspected_image != EXPECTED_IMAGE:
        raise PermissionError("Tika immutable image identity mismatch")

    if not isinstance(labels, dict) or (
        labels.get("org.daia.component") != "tika"
        or labels.get("org.daia.managed") != "true"
    ):
        raise PermissionError("Tika ownership labels mismatch")

    if network_mode != "daia-rag-internal":
        raise PermissionError("Tika network isolation mismatch")

    if security_options != ["no-new-privileges:true"]:
        raise PermissionError("Tika security options mismatch")

    if privileged is not False:
        raise PermissionError("Privileged Tika containers are forbidden")

    return validated_id


def register_authorized_container(
    container_id: str,
    inspected_image: str,
    labels: dict,
    network_mode: str,
    security_options: list,
    privileged: bool,
) -> None:
    """Publish only metadata that passes the registration contract."""
    authorized_id = authorize_container_registration(
        container_id,
        inspected_image,
        labels,
        network_mode,
        security_options,
        privileged,
    )

    publish_trusted_registration(STATE_DIRECTORY, authorized_id)


def provision_registration_directory() -> None:
    """Create or verify the private root-owned registration directory."""
    from pgvector_credentials import create_state_directory

    create_state_directory(STATE_DIRECTORY)


def read_registered_container_id() -> str:
    """Read the trusted Tika container identity."""
    return read_trusted_registration(STATE_DIRECTORY)


def verify_registered_container(container_id: str) -> None:
    """Require the inspected ID to match trusted persistent registration."""
    verify_trusted_registration(
        STATE_DIRECTORY,
        validate_container_id(container_id),
    )
