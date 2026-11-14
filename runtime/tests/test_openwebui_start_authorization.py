"""Independent, read-only Open WebUI startup authorization tests."""

import contextlib
import importlib.machinery
import importlib.util
import io
import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

RUNTIME = Path(__file__).resolve().parents[1]
SOURCE = RUNTIME / "daia-openwebui-register-container"
IDENTITY = "a" * 64
OTHER = "b" * 64

# Test-only import setup; the installed executable uses its trusted loader.
with patch.object(sys, "path", [str(RUNTIME), *sys.path]):
    loader = importlib.machinery.SourceFileLoader(
        "pragna_openwebui_start_authorization", str(SOURCE)
    )
    spec = importlib.util.spec_from_loader(loader.name, loader)
    if spec is None:
        raise RuntimeError("Unable to load registration helper")
    helper = importlib.util.module_from_spec(spec)
    loader.exec_module(helper)


def completed(code=0, output=""):
    return subprocess.CompletedProcess([], code, output, "")


class StartupAuthorizationTests(unittest.TestCase):
    def test_authorization_order_and_immutable_identity(self):
        events = []

        def registration(path):
            events.append("registration")
            self.assertEqual(path, "/var/lib/daia/openwebui")
            return IDENTITY

        def inspection(identity):
            events.append("inspection")
            self.assertEqual(identity, IDENTITY)
            return IDENTITY

        def role(arguments, **kwargs):
            events.append("rag")
            self.assertEqual(
                arguments,
                ["/usr/libexec/daia-openwebui-rag-authorize"],
            )
            self.assertEqual(kwargs["timeout"], 90)
            self.assertFalse(kwargs.get("shell", False))
            return completed()

        def final(identity):
            events.append("revalidate")
            return identity

        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                side_effect=registration,
            ),
            patch.object(
                helper, "verify_container_for_registration",
                side_effect=inspection,
            ),
            patch.object(
                helper.subprocess, "run", side_effect=role,
            ),
            patch.object(
                helper, "revalidate_container_before_registration",
                side_effect=final,
            ),
            patch.object(helper, "register_authorized_container") as publish,
            patch.object(helper, "provision_registration_directory") as create,
        ):
            self.assertEqual(
                helper.authorize_registered_container_for_start(IDENTITY),
                IDENTITY,
            )
            publish.assert_not_called()
            create.assert_not_called()

        self.assertEqual(
            events,
            [
                "registration", "inspection", "rag",
                "revalidate", "registration",
            ],
        )

    def test_nonroot_rejected_before_registration(self):
        with (
            patch.object(helper.os, "geteuid", return_value=1000),
            patch.object(helper, "read_trusted_registration") as read,
        ):
            with self.assertRaises(PermissionError):
                helper.authorize_registered_container_for_start(IDENTITY)
            read.assert_not_called()

    def test_registration_mismatch_blocks_docker_inspection(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                return_value=OTHER,
            ),
            patch.object(
                helper, "verify_container_for_registration"
            ) as inspect,
        ):
            with self.assertRaisesRegex(
                PermissionError, "registration identity mismatch"
            ):
                helper.authorize_registered_container_for_start(IDENTITY)
            inspect.assert_not_called()

    def test_failed_inspection_blocks_role_authorization(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                return_value=IDENTITY,
            ),
            patch.object(
                helper, "verify_container_for_registration",
                side_effect=PermissionError("unsafe container"),
            ),
            patch.object(helper.subprocess, "run") as run,
        ):
            with self.assertRaises(PermissionError):
                helper.authorize_registered_container_for_start(IDENTITY)
            run.assert_not_called()

    def test_failed_role_blocks_final_authorization(self):
        for result in (completed(1), completed(0, "unexpected\n")):
            with self.subTest(result=result):
                with (
                    patch.object(helper.os, "geteuid", return_value=0),
                    patch.object(
                        helper, "read_trusted_registration",
                        return_value=IDENTITY,
                    ),
                    patch.object(
                        helper, "verify_container_for_registration",
                        return_value=IDENTITY,
                    ),
                    patch.object(
                        helper.subprocess, "run", return_value=result,
                    ),
                    patch.object(
                        helper, "revalidate_container_before_registration"
                    ) as final,
                ):
                    with self.assertRaises(PermissionError):
                        helper.authorize_registered_container_for_start(
                            IDENTITY
                        )
                    final.assert_not_called()

    def test_role_timeout_fails_closed(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                return_value=IDENTITY,
            ),
            patch.object(
                helper, "verify_container_for_registration",
                return_value=IDENTITY,
            ),
            patch.object(
                helper.subprocess, "run",
                side_effect=subprocess.TimeoutExpired("rag", 90),
            ),
        ):
            with self.assertRaises(PermissionError):
                helper.authorize_registered_container_for_start(IDENTITY)

    def test_registration_change_after_role_proof_fails(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                side_effect=[IDENTITY, OTHER],
            ),
            patch.object(
                helper, "verify_container_for_registration",
                return_value=IDENTITY,
            ),
            patch.object(
                helper.subprocess, "run", return_value=completed(),
            ),
            patch.object(
                helper, "revalidate_container_before_registration",
                return_value=IDENTITY,
            ),
        ):
            with self.assertRaisesRegex(
                PermissionError, "registration changed"
            ):
                helper.authorize_registered_container_for_start(IDENTITY)

    def test_changed_container_identity_fails(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                return_value=IDENTITY,
            ),
            patch.object(
                helper, "verify_container_for_registration",
                return_value=IDENTITY,
            ),
            patch.object(
                helper.subprocess, "run", return_value=completed(),
            ),
            patch.object(
                helper, "revalidate_container_before_registration",
                return_value=OTHER,
            ),
        ):
            with self.assertRaisesRegex(
                PermissionError, "identity changed"
            ):
                helper.authorize_registered_container_for_start(IDENTITY)

    def test_cli_outputs_id_only_after_success(self):
        output = io.StringIO()
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper.sys, "argv",
                [str(SOURCE), "--authorize-start", IDENTITY],
            ),
            patch.object(
                helper.shutil, "which", return_value="/usr/bin/docker",
            ),
            patch.object(
                helper, "authorize_registered_container_for_start",
                return_value=IDENTITY,
            ) as authorize,
            patch.object(helper, "register_authorized_container") as publish,
            contextlib.redirect_stdout(output),
        ):
            self.assertEqual(helper.main(), 0)
            authorize.assert_called_once_with(IDENTITY)
            publish.assert_not_called()

        self.assertEqual(output.getvalue(), IDENTITY + "\n")

    def test_invalid_cli_identity_blocks_authorization(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper.sys, "argv",
                [str(SOURCE), "--authorize-start", "invalid"],
            ),
            patch.object(
                helper, "authorize_registered_container_for_start"
            ) as authorize,
        ):
            with self.assertRaises(ValueError):
                helper.main()
            authorize.assert_not_called()

    def test_start_authorization_never_mutates_docker(self):
        import inspect
        function_source = inspect.getsource(
            helper.authorize_registered_container_for_start
        )
        self.assertNotIn("docker start", function_source)
        self.assertNotIn("docker create", function_source)
        self.assertNotIn("publish_trusted_registration", function_source)
        self.assertNotIn("register_authorized_container(", function_source)


if __name__ == "__main__":
    unittest.main()
