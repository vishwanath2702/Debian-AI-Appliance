import ast
import contextlib
import io
import re
import subprocess
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "daia-openwebui-register-container"
PG = "a" * 64
WEBUI = "b" * 64
TIKA = "c" * 64


def validate(value):
    if not isinstance(value, str) or not re.fullmatch(
        r"[0-9a-f]{64}", value
    ):
        raise ValueError("Invalid container ID")
    return value


def load(name, bindings):
    tree = ast.parse(SOURCE.read_text(encoding="utf-8"))
    nodes = [
        n for n in tree.body
        if isinstance(n, ast.FunctionDef) and n.name == name
    ]
    if len(nodes) != 1:
        raise AssertionError("Function not unique")
    namespace = dict(bindings)
    exec(compile(
        ast.Module(body=nodes, type_ignores=[]),
        str(SOURCE), "exec",
    ), namespace)
    return namespace[name]


class TikaRunningTests(unittest.TestCase):
    def test_legacy_network_uses_two_identities(self):
        runner = mock.Mock(return_value=SimpleNamespace(
            returncode=0, stdout=""
        ))
        verify = load("verify_openwebui_runtime_network", {
            "validate_container_id": validate,
            "subprocess": SimpleNamespace(run=runner),
        })
        verify(PG, WEBUI)
        runner.assert_called_once()
        self.assertEqual(runner.call_args.args[0], [
            "/usr/libexec/daia-rag-verify-network",
            "runtime", PG, WEBUI,
        ])

    def test_combined_network_proves_tika_then_membership(self):
        runner = mock.Mock(side_effect=[
            SimpleNamespace(returncode=0, stdout=TIKA + "\n"),
            SimpleNamespace(returncode=0, stdout=""),
        ])
        verify = load("verify_openwebui_runtime_network", {
            "validate_container_id": validate,
            "subprocess": SimpleNamespace(run=runner),
        })
        verify(PG, WEBUI, tika_id=TIKA)
        self.assertEqual(runner.call_count, 2)
        self.assertEqual(runner.call_args_list[0].args[0], [
            "/usr/libexec/daia-tika-register-container",
            "--verify-identity", TIKA,
        ])
        self.assertEqual(runner.call_args_list[1].args[0], [
            "/usr/libexec/daia-rag-verify-network",
            "runtime", PG, WEBUI, TIKA,
        ])

    def test_untrusted_tika_blocks_network(self):
        for result in (
            SimpleNamespace(returncode=1, stdout=""),
            SimpleNamespace(returncode=0, stdout=PG + "\n"),
        ):
            with self.subTest(result=result):
                runner = mock.Mock(return_value=result)
                verify = load("verify_openwebui_runtime_network", {
                    "validate_container_id": validate,
                    "subprocess": SimpleNamespace(run=runner),
                })
                with self.assertRaises(PermissionError):
                    verify(PG, WEBUI, tika_id=TIKA)
                runner.assert_called_once()

    def test_duplicate_tika_blocks_all_commands(self):
        runner = mock.Mock()
        verify = load("verify_openwebui_runtime_network", {
            "validate_container_id": validate,
            "subprocess": SimpleNamespace(run=runner),
        })
        for identity in (PG, WEBUI):
            with self.subTest(identity=identity):
                with self.assertRaises(PermissionError):
                    verify(PG, WEBUI, tika_id=identity)
        runner.assert_not_called()

    def test_combined_role_propagates_both_ids(self):
        runner = mock.Mock(return_value=SimpleNamespace(
            returncode=0, stdout=""
        ))
        verify = load("verify_running_rag_role", {
            "validate_container_id": validate,
            "subprocess": SimpleNamespace(
                run=runner,
                TimeoutExpired=subprocess.TimeoutExpired,
            ),
        })
        verify(WEBUI, tika_id=TIKA)
        self.assertEqual(runner.call_args.args[0], [
            "/usr/libexec/daia-openwebui-rag-authorize",
            "--webui-id", WEBUI, "--tika-id", TIKA,
        ])

    def test_combined_recovery_rechecks_network(self):
        network = mock.Mock()
        role = mock.Mock()
        identity = mock.Mock(side_effect=lambda x: x)
        verify = load("verify_registered_running_container", {
            "validate_container_id": validate,
            "verify_registered_container_identity": identity,
            "inspect_container_running": mock.Mock(),
            "read_registered_postgresql_id": lambda: PG,
            "verify_openwebui_runtime_network": network,
            "verify_running_rag_role": role,
        })
        self.assertEqual(verify(WEBUI, tika_id=TIKA), WEBUI)
        self.assertEqual(network.call_args_list, [
            mock.call(PG, WEBUI, tika_id=TIKA),
            mock.call(PG, WEBUI, tika_id=TIKA),
        ])
        role.assert_called_once_with(WEBUI, tika_id=TIKA)
        self.assertEqual(identity.call_count, 2)

    def test_combined_recovery_fails_before_role(self):
        network = mock.Mock(side_effect=PermissionError("bad network"))
        role = mock.Mock()
        verify = load("verify_registered_running_container", {
            "validate_container_id": validate,
            "verify_registered_container_identity": lambda x: x,
            "inspect_container_running": mock.Mock(),
            "read_registered_postgresql_id": lambda: PG,
            "verify_openwebui_runtime_network": network,
            "verify_running_rag_role": role,
        })
        with self.assertRaises(PermissionError):
            verify(WEBUI, tika_id=TIKA)
        role.assert_not_called()

    def test_combined_cli_dispatch_and_invalid_id(self):
        running = mock.Mock(return_value=WEBUI)
        docker = mock.Mock(return_value="/usr/bin/docker")
        main = load("main", {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "sys": sys,
            "shutil": SimpleNamespace(which=docker),
            "validate_container_id": validate,
            "verify_registered_running_container": running,
        })
        with mock.patch.object(sys, "argv", [
            "helper", "--verify-running", WEBUI,
            "--tika-id", TIKA,
        ]):
            with contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(main(), 0)
        self.assertEqual(out.getvalue(), WEBUI + "\n")
        running.assert_called_once_with(WEBUI, tika_id=TIKA)

        docker.reset_mock()
        running.reset_mock()
        with mock.patch.object(sys, "argv", [
            "helper", "--verify-running", WEBUI,
            "--tika-id", "invalid",
        ]):
            with self.assertRaises(ValueError):
                main()
        docker.assert_not_called()
        running.assert_not_called()


if __name__ == "__main__":
    unittest.main()
