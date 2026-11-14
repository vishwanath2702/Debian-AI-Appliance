"""Read-only registered Open WebUI recovery verification."""

import contextlib
import importlib.machinery
import importlib.util
import io
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

RUNTIME = Path(__file__).resolve().parents[1]
SOURCE = RUNTIME / "daia-openwebui-register-container"
IDENTITY = "a" * 64
POSTGRESQL = "b" * 64
OTHER = "c" * 64

with patch.object(sys, "path", [str(RUNTIME), *sys.path]):
    loader = importlib.machinery.SourceFileLoader(
        "pragna_openwebui_running_recovery", str(SOURCE)
    )
    spec = importlib.util.spec_from_loader(loader.name, loader)
    if spec is None:
        raise RuntimeError("Unable to load registration helper")
    helper = importlib.util.module_from_spec(spec)
    loader.exec_module(helper)


class RunningRecoveryTests(unittest.TestCase):
    def test_identity_verification_checks_registration_and_security(self):
        events = []

        def registration(path):
            events.append("registration")
            self.assertEqual(path, "/var/lib/daia/openwebui")
            return IDENTITY

        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                side_effect=registration,
            ),
            patch.object(
                helper, "inspect_container_identity",
                side_effect=lambda identity: (
                    events.append("identity"), identity
                )[1],
            ),
            patch.object(
                helper, "inspect_container_security",
                side_effect=lambda identity: events.append("security"),
            ),
            patch.object(
                helper, "inspect_container_mounts",
                side_effect=lambda identity: events.append("mounts"),
            ),
            patch.object(
                helper, "inspect_volume_ownership",
                side_effect=lambda: events.append("volume"),
            ),
        ):
            self.assertEqual(
                helper.verify_registered_container_identity(IDENTITY),
                IDENTITY,
            )

        self.assertEqual(
            events,
            [
                "registration", "identity", "security",
                "mounts", "volume", "registration",
            ],
        )

    def test_identity_rejects_unregistered_container(self):
        with (
            patch.object(helper.os, "geteuid", return_value=0),
            patch.object(
                helper, "read_trusted_registration",
                return_value=OTHER,
            ),
            patch.object(helper, "inspect_container_identity") as inspect,
        ):
            with self.assertRaises(PermissionError):
                helper.verify_registered_container_identity(IDENTITY)
            inspect.assert_not_called()

    def test_identity_rejects_nonroot(self):
        with (
            patch.object(helper.os, "geteuid", return_value=1000),
            patch.object(helper, "read_trusted_registration") as read,
        ):
            with self.assertRaises(PermissionError):
                helper.verify_registered_container_identity(IDENTITY)
            read.assert_not_called()

    def test_running_state_rejects_stopped_container(self):
        for state in (
            "false|false|false|created\n",
            "false|false|false|exited\n",
            "true|true|false|paused\n",
            "true|false|true|restarting\n",
        ):
            with self.subTest(state=state):
                result = subprocess.CompletedProcess([], 0, state, "")
                with patch.object(
                    helper.subprocess, "run", return_value=result
                ):
                    with self.assertRaises(PermissionError):
                        helper.inspect_container_running(IDENTITY)

    def test_running_state_rejects_literal_backslash_n(self):
        result = subprocess.CompletedProcess(
            [], 0, r"true|false|false|running\n", ""
        )
        with patch.object(
            helper.subprocess, "run", return_value=result
        ):
            with self.assertRaises(PermissionError):
                helper.inspect_container_running(IDENTITY)

    def test_running_state_accepts_only_exact_running_state(self):
        result = subprocess.CompletedProcess(
            [], 0, "true|false|false|running\n", ""
        )
        with patch.object(
            helper.subprocess, "run", return_value=result
        ) as runner:
            helper.inspect_container_running(IDENTITY)
            self.assertEqual(
                runner.call_args.args[0][-1], IDENTITY
            )

    def test_recovery_verification_order(self):
        events = []

        with (
            patch.object(
                helper, "verify_registered_container_identity",
                side_effect=lambda identity: (
                    events.append("identity"), identity
                )[1],
            ),
            patch.object(
                helper, "inspect_container_running",
                side_effect=lambda identity: events.append("running"),
            ),
            patch.object(
                helper, "read_registered_postgresql_id",
                side_effect=lambda: (
                    events.append("postgresql"), POSTGRESQL
                )[1],
            ),
            patch.object(
                helper, "verify_openwebui_runtime_network",
                side_effect=lambda *args: events.append("network"),
            ),
            patch.object(
                helper, "verify_running_rag_role",
                side_effect=lambda identity: events.append("rag"),
            ),
        ):
            self.assertEqual(
                helper.verify_registered_running_container(IDENTITY),
                IDENTITY,
            )

        self.assertEqual(
            events,
            [
                "identity", "running", "postgresql",
                "network", "rag", "identity",
                "running", "postgresql", "network",
            ],
        )

    def test_network_failure_blocks_role_proof(self):
        with (
            patch.object(
                helper, "verify_registered_container_identity",
                return_value=IDENTITY,
            ),
            patch.object(helper, "inspect_container_running"),
            patch.object(
                helper, "read_registered_postgresql_id",
                return_value=POSTGRESQL,
            ),
            patch.object(
                helper, "verify_openwebui_runtime_network",
                side_effect=PermissionError("network"),
            ),
            patch.object(helper, "verify_running_rag_role") as rag,
        ):
            with self.assertRaises(PermissionError):
                helper.verify_registered_running_container(IDENTITY)
            rag.assert_not_called()

    def test_rag_failure_blocks_final_verification(self):
        with (
            patch.object(
                helper, "verify_registered_container_identity",
                return_value=IDENTITY,
            ) as identity,
            patch.object(helper, "inspect_container_running"),
            patch.object(
                helper, "read_registered_postgresql_id",
                return_value=POSTGRESQL,
            ),
            patch.object(helper, "verify_openwebui_runtime_network"),
            patch.object(
                helper, "verify_running_rag_role",
                side_effect=PermissionError("role"),
            ),
        ):
            with self.assertRaises(PermissionError):
                helper.verify_registered_running_container(IDENTITY)
            self.assertEqual(identity.call_count, 1)

    def test_postgresql_identity_change_fails_closed(self):
        with (
            patch.object(
                helper, "verify_registered_container_identity",
                return_value=IDENTITY,
            ),
            patch.object(helper, "inspect_container_running"),
            patch.object(
                helper, "read_registered_postgresql_id",
                side_effect=[POSTGRESQL, OTHER],
            ),
            patch.object(helper, "verify_openwebui_runtime_network"),
            patch.object(helper, "verify_running_rag_role"),
        ):
            with self.assertRaisesRegex(
                PermissionError, "PostgreSQL registration changed"
            ):
                helper.verify_registered_running_container(IDENTITY)

    def test_rag_nonzero_and_output_fail_closed(self):
        for code, stdout in ((1, ""), (0, "unexpected\\n")):
            result = subprocess.CompletedProcess([], code, stdout, "")
            with patch.object(
                helper.subprocess, "run", return_value=result
            ):
                with self.assertRaises(PermissionError):
                    helper.verify_running_rag_role(IDENTITY)

    def test_rag_timeout_fails_closed(self):
        with patch.object(
            helper.subprocess, "run",
            side_effect=subprocess.TimeoutExpired("rag", 90),
        ):
            with self.assertRaises(PermissionError):
                helper.verify_running_rag_role(IDENTITY)

    def test_cli_running_and_identity_modes(self):
        for flag, target in (
            ("--verify-running", "verify_registered_running_container"),
            ("--verify-identity", "verify_registered_container_identity"),
        ):
            with self.subTest(flag=flag):
                output = io.StringIO()
                with (
                    patch.object(
                        helper.sys, "argv",
                        [str(SOURCE), flag, IDENTITY],
                    ),
                    patch.object(
                        helper.shutil, "which",
                        return_value="/usr/bin/docker",
                    ),
                    patch.object(
                        helper.os, "geteuid", return_value=0,
                    ),
                    patch.object(
                        helper, target, return_value=IDENTITY
                    ) as verifier,
                    contextlib.redirect_stdout(output),
                ):
                    self.assertEqual(helper.main(), 0)
                    verifier.assert_called_once_with(IDENTITY)
                self.assertEqual(output.getvalue(), IDENTITY + "\n")

    def test_no_mutation_in_recovery_verification(self):
        import inspect

        for function in (
            helper.verify_registered_container_identity,
            helper.verify_registered_running_container,
            helper.verify_openwebui_runtime_network,
            helper.verify_running_rag_role,
        ):
            code = inspect.getsource(function)
            self.assertNotIn("docker start", code)
            self.assertNotIn("docker stop", code)
            self.assertNotIn("docker create", code)
            self.assertNotIn("publish_trusted_registration(", code)


if __name__ == "__main__":
    unittest.main()
