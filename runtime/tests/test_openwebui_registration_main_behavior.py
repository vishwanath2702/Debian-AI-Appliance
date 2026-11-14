"""Mocked privileged Open WebUI registration entry-point tests."""

import importlib.machinery
import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch, call

RUNTIME = Path(__file__).resolve().parents[1]
HELPER = RUNTIME / "daia-openwebui-register-container"
sys.path.insert(0, str(RUNTIME))

loader = importlib.machinery.SourceFileLoader(
    "daia_openwebui_registration_main_under_test",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)

CONTAINER_ID = "a" * 64
VERIFIED_ID = CONTAINER_ID
IMAGE = helper.EXPECTED_IMAGE
LABELS = {
    "org.daia.component": "openwebui",
    "org.daia.managed": "true",
}


class OpenWebUIRegistrationMainTests(unittest.TestCase):

    def test_successful_registration_order(self):
        events = []

        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(helper.sys, "argv", ["register", CONTAINER_ID]),
            patch.object(helper.shutil, "which", return_value="/usr/bin/docker"),
            patch.object(
                helper,
                "verify_container_for_registration",
                side_effect=lambda cid: (
                    events.append(("verify", cid)) or VERIFIED_ID
                ),
            ),
            patch.object(
                helper, "provision_registration_directory",
                side_effect=lambda: events.append(("provision",)),
            ),
            patch.object(
                helper, "register_authorized_container",
                side_effect=lambda *args: events.append(
                    ("register", *args)
                ),
            ),
        ):
            result = helper.main()

        self.assertEqual(result, 0)
        self.assertEqual(
            [event[0] for event in events],
            ["verify", "provision", "register"],
        )
        self.assertEqual(events[0], ("verify", CONTAINER_ID))
        self.assertEqual(
            events[2],
            (
                "register",
                VERIFIED_ID,
                IMAGE,
                IMAGE,
                LABELS,
                "daia-rag-internal",
                ["no-new-privileges:true"],
                False,
            ),
        )

    def test_verification_failure_prevents_provision_and_publication(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(helper.sys, "argv", ["register", CONTAINER_ID]),
            patch.object(helper.shutil, "which", return_value="/usr/bin/docker"),
            patch.object(
                helper,
                "verify_container_for_registration",
                side_effect=PermissionError("verification failed"),
            ),
            patch.object(
                helper, "provision_registration_directory"
            ) as provision,
            patch.object(
                helper, "register_authorized_container"
            ) as register,
        ):
            with self.assertRaises(PermissionError):
                helper.main()

        provision.assert_not_called()
        register.assert_not_called()

    def test_provisioning_failure_prevents_publication(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(helper.sys, "argv", ["register", CONTAINER_ID]),
            patch.object(helper.shutil, "which", return_value="/usr/bin/docker"),
            patch.object(
                helper,
                "verify_container_for_registration",
                return_value=VERIFIED_ID,
            ),
            patch.object(
                helper, "provision_registration_directory",
                side_effect=PermissionError("unsafe state directory"),
            ),
            patch.object(
                helper, "register_authorized_container"
            ) as register,
        ):
            with self.assertRaises(PermissionError):
                helper.main()

        register.assert_not_called()

    def test_rejects_non_root_before_verification(self):
        with (
            patch.object(helper.os, "geteuid", return_value=1000),
            patch.object(helper.sys, "argv", ["register", CONTAINER_ID]),
            patch.object(
                helper, "verify_container_for_registration"
            ) as verify,
        ):
            with self.assertRaises(PermissionError):
                helper.main()

        verify.assert_not_called()

    def test_rejects_invalid_id_before_verification(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(helper.sys, "argv", ["register", "invalid"]),
            patch.object(
                helper, "verify_container_for_registration"
            ) as verify,
        ):
            with self.assertRaises(ValueError):
                helper.main()

        verify.assert_not_called()

    def test_rejects_missing_docker_before_verification(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(helper.sys, "argv", ["register", CONTAINER_ID]),
            patch.object(helper.shutil, "which", return_value=None),
            patch.object(
                helper, "verify_container_for_registration"
            ) as verify,
        ):
            with self.assertRaises(RuntimeError):
                helper.main()

        verify.assert_not_called()

    def test_rejects_changed_verified_id_without_publication(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(helper.sys, "argv", ["register", CONTAINER_ID]),
            patch.object(helper.shutil, "which", return_value="/usr/bin/docker"),
            patch.object(
                helper,
                "verify_container_for_registration",
                return_value="b" * 64,
            ),
            patch.object(
                helper, "provision_registration_directory"
            ) as provision,
            patch.object(
                helper, "register_authorized_container"
            ) as register,
        ):
            with self.assertRaises(PermissionError):
                helper.main()

        provision.assert_not_called()
        register.assert_not_called()


if __name__ == "__main__":
    unittest.main()
