#!/usr/bin/env python3
"""Trusted persistent container registration for DAIA Open WebUI."""

from pgvector_registration import (
    publish_trusted_registration,
    verify_trusted_registration,
    read_trusted_registration,
    validate_container_id,
)

STATE_DIRECTORY = "/var/lib/daia/openwebui"


def provision_registration_directory() -> None:
    """Create or verify the private registration directory.

    The parent must already exist and satisfy the trusted-directory
    requirements enforced by pgvector_credentials.
    """
    from pgvector_credentials import create_state_directory

    create_state_directory(STATE_DIRECTORY)


def read_registered_container_id() -> str:
    """Read the Open WebUI identity from its private state directory."""
    return read_trusted_registration(STATE_DIRECTORY)


def authorize_container_registration(
    container_id: str,
    inspected_image: str,
    expected_image: str,
    labels: dict,
    network_mode: str,
    security_options: list,
    privileged: bool,
) -> str:
    """Validate immutable Docker identity and required security metadata."""
    validated_id = validate_container_id(container_id)

    if (
        not isinstance(expected_image, str)
        or not expected_image.startswith("sha256:")
        or len(expected_image) != 71
        or any(char not in "0123456789abcdef"
               for char in expected_image[7:])
        or inspected_image != expected_image
    ):
        raise PermissionError("Open WebUI image identity mismatch")

    if not isinstance(labels, dict) or (
        labels.get("org.daia.component") != "openwebui"
        or labels.get("org.daia.managed") != "true"
    ):
        raise PermissionError("Open WebUI ownership labels mismatch")

    if network_mode != "daia-rag-internal":
        raise PermissionError("Open WebUI network mode mismatch")

    if (
        not isinstance(security_options, list)
        or security_options != ["no-new-privileges:true"]
        or privileged is not False
    ):
        raise PermissionError("Open WebUI privilege contract mismatch")

    return validated_id


def register_authorized_container(
    container_id: str,
    inspected_image: str,
    expected_image: str,
    labels: dict,
    network_mode: str,
    security_options: list,
    privileged: bool,
) -> None:
    """Publish only after all supplied inspection metadata is validated."""
    authorized_id = authorize_container_registration(
        container_id,
        inspected_image,
        expected_image,
        labels,
        network_mode,
        security_options,
        privileged,
    )
    publish_trusted_registration(STATE_DIRECTORY, authorized_id)


def verify_registered_container(container_id: str) -> None:
    """Require a match with the trusted Open WebUI registration."""
    validated_id = validate_container_id(container_id)
    verify_trusted_registration(STATE_DIRECTORY, validated_id)
