"""Behavioral tests for Open WebUI registration authorization order."""

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
    "daia_openwebui_registration_sequence_under_test",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)

CONTAINER_ID = "a" * 64
POSTGRESQL_ID = "b" * 64

CHECKS = (
    "inspect_container_identity",
    "inspect_container_stopped",
    "inspect_container_security",
    "inspect_container_mounts",
    "inspect_volume_ownership",
    "read_registered_postgresql_id",
    "verify_registration_network",
    "revalidate_container_before_registration",
)

EXPECTED_ORDER = [
    "inspect_container_identity",
    "inspect_container_stopped",
    "inspect_container_security",
    "inspect_container_mounts",
    "inspect_volume_ownership",
    "read_registered_postgresql_id",
    "verify_registration_network",
    "revalidate_container_before_registration",
]


class OpenWebUIRegistrationSequenceTests(unittest.TestCase):

    def run_sequence(self, failure=None):
        events = []

        def make_mock(name):
            def action(*args):
                events.append((name, args))
                if name == failure:
                    raise PermissionError("injected verification failure")
                if name in (
                    "inspect_container_identity",
                    "revalidate_container_before_registration",
                ):
                    return CONTAINER_ID
                if name == "read_registered_postgresql_id":
                    return POSTGRESQL_ID
                return None
            return action

        patches = [
            patch.object(helper, name, side_effect=make_mock(name))
            for name in CHECKS
        ]

        for item in patches:
            item.start()

        try:
            if failure is None:
                result = helper.verify_container_for_registration(
                    CONTAINER_ID
                )
            else:
                with self.assertRaises(PermissionError):
                    helper.verify_container_for_registration(
                        CONTAINER_ID
                    )
                result = None
        finally:
            for item in reversed(patches):
                item.stop()

        return result, events

    def test_successful_verification_order(self):
        result, events = self.run_sequence()

        self.assertEqual(result, CONTAINER_ID)
        self.assertEqual(
            [name for name, _ in events],
            EXPECTED_ORDER,
        )
        self.assertEqual(
            events[6],
            ("verify_registration_network", (POSTGRESQL_ID,)),
        )
        self.assertEqual(
            events[-1],
            (
                "revalidate_container_before_registration",
                (CONTAINER_ID,),
            ),
        )

    def test_every_failed_check_stops_later_checks(self):
        for index, name in enumerate(EXPECTED_ORDER):
            with self.subTest(failed_check=name):
                _, events = self.run_sequence(failure=name)
                self.assertEqual(
                    [event_name for event_name, _ in events],
                    EXPECTED_ORDER[:index + 1],
                )

    def test_invalid_id_is_rejected_before_inspection(self):
        with patch.object(
            helper, "inspect_container_identity"
        ) as identity:
            with self.assertRaises(ValueError):
                helper.verify_container_for_registration("invalid")
        identity.assert_not_called()

    def test_identity_change_aborts_before_other_checks(self):
        with (
            patch.object(
                helper,
                "inspect_container_identity",
                return_value="c" * 64,
            ),
            patch.object(
                helper,
                "inspect_container_stopped",
            ) as stopped,
        ):
            with self.assertRaises(PermissionError):
                helper.verify_container_for_registration(CONTAINER_ID)
        stopped.assert_not_called()

    def test_no_publication_in_verification_function(self):
        with (
            patch.object(
                helper,
                "inspect_container_identity",
                return_value=CONTAINER_ID,
            ),
            patch.object(helper, "inspect_container_stopped"),
            patch.object(helper, "inspect_container_security"),
            patch.object(helper, "inspect_container_mounts"),
            patch.object(helper, "inspect_volume_ownership"),
            patch.object(
                helper,
                "read_registered_postgresql_id",
                return_value=POSTGRESQL_ID,
            ),
            patch.object(helper, "verify_registration_network"),
            patch.object(
                helper,
                "revalidate_container_before_registration",
                return_value=CONTAINER_ID,
            ),
            patch(
                "openwebui_registration.publish_trusted_registration"
            ) as publish,
        ):
            helper.verify_container_for_registration(CONTAINER_ID)
        publish.assert_not_called()


if __name__ == "__main__":
    unittest.main()
