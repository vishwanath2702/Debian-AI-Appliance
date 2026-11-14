"""Isolated Open WebUI registration directory provisioning tests."""

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
sys.path.insert(0, str(RUNTIME))

spec = importlib.util.spec_from_file_location(
    "daia_openwebui_state_directory",
    RUNTIME / "openwebui_registration.py",
)
registration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(registration)


class OpenWebUIStateDirectoryTests(unittest.TestCase):

    def test_uses_private_openwebui_directory(self):
        self.assertEqual(
            registration.STATE_DIRECTORY,
            "/var/lib/daia/openwebui",
        )

    def test_delegates_to_secure_directory_creator(self):
        with patch(
            "pgvector_credentials.create_state_directory"
        ) as create:
            registration.provision_registration_directory()

        create.assert_called_once_with(
            "/var/lib/daia/openwebui"
        )

    def test_propagates_untrusted_parent_failure(self):
        with patch(
            "pgvector_credentials.create_state_directory",
            side_effect=PermissionError("untrusted parent"),
        ):
            with self.assertRaises(PermissionError):
                registration.provision_registration_directory()

    def test_provisioning_does_not_publish_registration(self):
        with (
            patch(
                "pgvector_credentials.create_state_directory"
            ) as create,
            patch.object(
                registration,
                "publish_trusted_registration",
            ) as publish,
        ):
            registration.provision_registration_directory()

        create.assert_called_once()
        publish.assert_not_called()


if __name__ == "__main__":
    unittest.main()
