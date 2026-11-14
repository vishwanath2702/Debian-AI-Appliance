"""Behavioral tests for Open WebUI registration authorization."""

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
MODULE = RUNTIME / "openwebui_registration.py"

sys.path.insert(0, str(RUNTIME))

spec = importlib.util.spec_from_file_location(
    "daia_openwebui_registration_behavior", MODULE
)
registration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(registration)

CONTAINER_ID = "a" * 64
IMAGE = "sha256:" + "b" * 64
LABELS = {
    "org.daia.component": "openwebui",
    "org.daia.managed": "true",
}


class OpenWebUIRegistrationBehaviorTests(unittest.TestCase):
    def authorize(self, **changes):
        values = {
            "container_id": CONTAINER_ID,
            "inspected_image": IMAGE,
            "expected_image": IMAGE,
            "labels": LABELS.copy(),
            "network_mode": "daia-rag-internal",
            "security_options": ["no-new-privileges:true"],
            "privileged": False,
        }
        values.update(changes)
        return registration.authorize_container_registration(**values)

    def test_accepts_matching_metadata(self):
        self.assertEqual(self.authorize(), CONTAINER_ID)

    def test_rejects_invalid_container_id(self):
        with self.assertRaises(ValueError):
            self.authorize(container_id="invalid")

    def test_rejects_image_mismatch(self):
        with self.assertRaises(PermissionError):
            self.authorize(inspected_image="sha256:" + "c" * 64)

    def test_rejects_invalid_expected_image(self):
        with self.assertRaises(PermissionError):
            self.authorize(expected_image="untrusted:latest")

    def test_rejects_missing_ownership_label(self):
        with self.assertRaises(PermissionError):
            self.authorize(labels={"org.daia.managed": "true"})

    def test_rejects_unmanaged_container(self):
        with self.assertRaises(PermissionError):
            self.authorize(labels={
                "org.daia.component": "openwebui",
                "org.daia.managed": "false",
            })

    def test_rejects_host_network(self):
        with self.assertRaises(PermissionError):
            self.authorize(network_mode="host")

    def test_rejects_privileged_container(self):
        with self.assertRaises(PermissionError):
            self.authorize(privileged=True)

    def test_rejects_missing_no_new_privileges(self):
        with self.assertRaises(PermissionError):
            self.authorize(security_options=[])

    def test_rejects_extra_security_options(self):
        with self.assertRaises(PermissionError):
            self.authorize(security_options=[
                "no-new-privileges:true", "seccomp=unconfined"
            ])

    def test_publishes_only_after_authorization(self):
        with patch.object(
            registration, "publish_trusted_registration"
        ) as publish:
            registration.register_authorized_container(
                CONTAINER_ID, IMAGE, IMAGE, LABELS.copy(),
                "daia-rag-internal",
                ["no-new-privileges:true"], False,
            )
            publish.assert_called_once_with(
                registration.STATE_DIRECTORY, CONTAINER_ID
            )

    def test_never_publishes_rejected_container(self):
        with patch.object(
            registration, "publish_trusted_registration"
        ) as publish:
            with self.assertRaises(PermissionError):
                registration.register_authorized_container(
                    CONTAINER_ID, IMAGE, IMAGE, LABELS.copy(),
                    "host", ["no-new-privileges:true"], False,
                )
            publish.assert_not_called()


if __name__ == "__main__":
    unittest.main()
