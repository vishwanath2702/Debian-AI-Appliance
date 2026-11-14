import ast
import re
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
PG = "a" * 64
WEBUI = "b" * 64
TIKA = "c" * 64


class AuthorizationError(Exception):
    pass


class ProvisioningError(Exception):
    pass


def validate(value):
    if not isinstance(value, str) or not re.fullmatch(
        r"[0-9a-f]{64}", value
    ):
        raise ProvisioningError("Invalid container ID")
    return value


def load_functions(path, names, bindings):
    tree = ast.parse(path.read_text(encoding="utf-8"))
    nodes = [
        node for node in tree.body
        if isinstance(node, ast.FunctionDef)
        and node.name in names
    ]
    if {node.name for node in nodes} != set(names):
        raise AssertionError("Expected functions unavailable")
    namespace = dict(bindings)
    exec(
        compile(
            ast.Module(body=nodes, type_ignores=[]),
            str(path), "exec",
        ),
        namespace,
    )
    return namespace


def successful(code=0, output=""):
    return SimpleNamespace(returncode=code, stdout=output)


class TikaNestedAuthorizationTests(unittest.TestCase):
    def auth(self):
        calls = []

        def checked(command, timeout=None):
            calls.append(list(command))
            if command[0] == "pg-auth":
                return successful(0, PG + "\n")
            return successful()

        namespace = load_functions(
            ROOT / "daia-openwebui-rag-authorize",
            ("preflight", "main"),
            {
                "os": SimpleNamespace(geteuid=lambda: 0),
                "sys": sys,
                "re": re,
                "PG_AUTHORIZER": "pg-auth",
                "ROLE_VERIFIER": "role-verify",
                "checked_command": checked,
                "AuthorizationError": AuthorizationError,
            },
        )
        return namespace, calls

    def provision(self):
        namespace = load_functions(
            ROOT / "daia-openwebui-rag-provision.py",
            ("authorize_container", "verify_existing_role", "main"),
            {
                "subprocess": subprocess,
                "sys": sys,
                "AUTHORIZER": "pg-auth",
                "ProvisioningError": ProvisioningError,
                "validate_container_id": validate,
            },
        )
        return namespace

    def test_legacy_postgresql_and_webui_commands(self):
        namespace, calls = self.auth()
        self.assertTrue(namespace["preflight"]())
        self.assertEqual(calls[0], ["pg-auth", "runtime"])
        self.assertEqual(
            calls[2], ["role-verify", "--verify-only", PG]
        )
        calls.clear()
        self.assertTrue(namespace["preflight"](WEBUI))
        self.assertEqual(
            calls[0], ["pg-auth", "runtime", WEBUI]
        )
        self.assertEqual(
            calls[2], ["role-verify", "--verify-only", PG, WEBUI]
        )

    def test_tika_only_authorization(self):
        namespace, calls = self.auth()
        self.assertTrue(namespace["preflight"](tika_id=TIKA))
        self.assertEqual(
            calls[0],
            ["pg-auth", "runtime", "--tika-id", TIKA],
        )
        self.assertEqual(
            calls[2],
            ["role-verify", "--verify-only", PG, "--tika-id", TIKA],
        )

    def test_combined_webui_tika_authorization(self):
        namespace, calls = self.auth()
        self.assertTrue(namespace["preflight"](WEBUI, TIKA))
        self.assertEqual(
            calls[0],
            ["pg-auth", "runtime", WEBUI, "--tika-id", TIKA],
        )
        self.assertEqual(
            calls[2],
            [
                "role-verify", "--verify-only", PG, WEBUI,
                "--tika-id", TIKA,
            ],
        )

    def test_invalid_tika_fails_before_postgresql(self):
        namespace, calls = self.auth()
        with self.assertRaises(AuthorizationError):
            namespace["preflight"](tika_id="invalid")
        self.assertEqual(calls, [])

    def test_authorizer_cli_modes(self):
        namespace, _ = self.auth()
        preflight = mock.Mock()
        namespace["preflight"] = preflight

        cases = (
            (["helper"], (None,)),
            (["helper", "--webui-id", WEBUI], (WEBUI,)),
            (["helper", "--tika-id", TIKA], (None, TIKA)),
            (
                ["helper", "--webui-id", WEBUI, "--tika-id", TIKA],
                (WEBUI, TIKA),
            ),
        )
        for argv, expected in cases:
            with self.subTest(argv=argv):
                preflight.reset_mock()
                with mock.patch.object(sys, "argv", argv):
                    self.assertEqual(namespace["main"](), 0)
                preflight.assert_called_once_with(*expected)

        with mock.patch.object(
            sys, "argv",
            ["helper", "--tika-id", TIKA, "--webui-id", WEBUI],
        ):
            self.assertEqual(namespace["main"](), 1)

    def test_provisioner_pg_command_with_tika(self):
        namespace = self.provision()
        runner = mock.Mock(return_value=successful(0, PG + "\n"))
        self.assertEqual(
            namespace["authorize_container"](
                runner=runner, webui_id=WEBUI, tika_id=TIKA
            ),
            PG,
        )
        self.assertEqual(
            runner.call_args.args[0],
            ["pg-auth", "runtime", WEBUI, "--tika-id", TIKA],
        )

    def test_provisioner_rejects_bad_tika(self):
        namespace = self.provision()
        runner = mock.Mock()
        with self.assertRaises(ProvisioningError):
            namespace["authorize_container"](
                runner=runner, tika_id="bad"
            )
        runner.assert_not_called()

    def test_role_verifier_propagates_both_ids(self):
        namespace = self.provision()
        authorize = mock.Mock(return_value=PG)
        namespace.update({
            "os": SimpleNamespace(geteuid=lambda: 0),
            "authorize_container": authorize,
            "validate_application_credential": lambda: "secret",
            "verify_postgres_ready": mock.Mock(),
            "read_role_verifier": lambda _: "scram",
            "verify_role_password": mock.Mock(),
            "verify_role_state": mock.Mock(),
        })
        self.assertTrue(
            namespace["verify_existing_role"](
                PG, webui_id=WEBUI, tika_id=TIKA
            )
        )
        authorize.assert_called_once_with(
            webui_id=WEBUI, tika_id=TIKA
        )

    def test_provisioner_cli_tika_modes(self):
        namespace = self.provision()
        verifier = mock.Mock()
        namespace["verify_existing_role"] = verifier

        self.assertEqual(
            namespace["main"](
                ["--verify-only", PG, "--tika-id", TIKA]
            ),
            0,
        )
        verifier.assert_called_with(PG, tika_id=TIKA)

        self.assertEqual(
            namespace["main"](
                ["--verify-only", PG, WEBUI, "--tika-id", TIKA]
            ),
            0,
        )
        verifier.assert_called_with(
            PG, WEBUI, tika_id=TIKA
        )

        self.assertEqual(
            namespace["main"](
                ["--verify-only", PG, "--tika-id", "bad"]
            ),
            1,
        )


if __name__ == "__main__":
    unittest.main()
