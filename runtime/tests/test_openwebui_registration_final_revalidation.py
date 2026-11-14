"""Tests for final Open WebUI registration revalidation."""

import importlib.machinery
import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

RUNTIME = Path(__file__).resolve().parents[1]
HELPER = RUNTIME / "daia-openwebui-register-container"

sys.path.insert(0, str(RUNTIME))

loader = importlib.machinery.SourceFileLoader(
    "daia_openwebui_final_revalidation_under_test",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)

CONTAINER_ID = "a" * 64


class OpenWebUIFinalRevalidationTests(unittest.TestCase):

    def test_rechecks_identity_and_stopped_state(self):
        with (
            patch.object(
                helper,
                "inspect_container_identity",
                return_value=CONTAINER_ID,
            ) as identity,
            patch.object(
                helper,
                "inspect_container_stopped",
            ) as stopped,
        ):
            result = helper.revalidate_container_before_registration(
                CONTAINER_ID
            )

        self.assertEqual(result, CONTAINER_ID)
        identity.assert_called_once_with(CONTAINER_ID)
        stopped.assert_called_once_with(CONTAINER_ID)

    def test_rejects_changed_identity(self):
        with (
            patch.object(
                helper,
                "inspect_container_identity",
                return_value="b" * 64,
            ),
            patch.object(
                helper,
                "inspect_container_stopped",
            ) as stopped,
        ):
            with self.assertRaises(PermissionError):
                helper.revalidate_container_before_registration(
                    CONTAINER_ID
                )

        stopped.assert_not_called()

    def test_rejects_running_container(self):
        with (
            patch.object(
                helper,
                "inspect_container_identity",
                return_value=CONTAINER_ID,
            ),
            patch.object(
                helper,
                "inspect_container_stopped",
                side_effect=PermissionError("running"),
            ),
        ):
            with self.assertRaises(PermissionError):
                helper.revalidate_container_before_registration(
                    CONTAINER_ID
                )

    def test_rejects_invalid_id_before_inspection(self):
        with (
            patch.object(
                helper,
                "inspect_container_identity",
            ) as identity,
            patch.object(
                helper,
                "inspect_container_stopped",
            ) as stopped,
        ):
            with self.assertRaises(ValueError):
                helper.revalidate_container_before_registration(
                    "invalid"
                )

        identity.assert_not_called()
        stopped.assert_not_called()


if __name__ == "__main__":
    unittest.main()
