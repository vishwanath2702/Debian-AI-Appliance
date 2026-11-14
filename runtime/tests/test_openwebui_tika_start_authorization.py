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
WEBUI = "a" * 64
TIKA = "b" * 64


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
    exec(
        compile(
            ast.Module(body=nodes, type_ignores=[]),
            str(SOURCE), "exec",
        ),
        namespace,
    )
    return namespace[name]


class TikaStartupTests(unittest.TestCase):
    def setUp(self):
        self.registration = mock.Mock(return_value=WEBUI)
        self.commands = mock.Mock(
            return_value=SimpleNamespace(returncode=0, stdout="")
        )
        self.verify = load(
            "authorize_registered_container_for_start",
            {
                "os": SimpleNamespace(geteuid=lambda: 0),
                "validate_container_id": validate,
                "read_trusted_registration": lambda _: WEBUI,
                "verify_container_for_registration": self.registration,
                "revalidate_container_before_registration": lambda x: x,
                "subprocess": SimpleNamespace(
                    run=self.commands,
                    TimeoutExpired=subprocess.TimeoutExpired,
                ),
            },
        )

    def test_legacy_start_keeps_original_calls(self):
        self.assertEqual(self.verify(WEBUI), WEBUI)
        self.registration.assert_called_once_with(WEBUI)
        self.assertEqual(
            self.commands.call_args.args[0],
            ["/usr/libexec/daia-openwebui-rag-authorize"],
        )

    def test_combined_start_checks_tika_twice(self):
        self.assertEqual(self.verify(WEBUI, tika_id=TIKA), WEBUI)
        self.assertEqual(
            self.registration.call_args_list,
            [
                mock.call(WEBUI, tika_id=TIKA),
                mock.call(WEBUI, tika_id=TIKA),
            ],
        )
        self.assertEqual(
            self.commands.call_args.args[0],
            [
                "/usr/libexec/daia-openwebui-rag-authorize",
                "--tika-id", TIKA,
            ],
        )

    def test_invalid_tika_never_authorizes_role(self):
        with self.assertRaises(ValueError):
            self.verify(WEBUI, tika_id="invalid")
        self.commands.assert_not_called()

    def test_failed_combined_registration_blocks_role(self):
        self.registration.side_effect = PermissionError("Untrusted Tika")
        with self.assertRaises(PermissionError):
            self.verify(WEBUI, tika_id=TIKA)
        self.commands.assert_not_called()

    def test_role_failure_blocks_final_revalidation(self):
        self.commands.return_value = SimpleNamespace(
            returncode=1, stdout=""
        )
        with self.assertRaises(PermissionError):
            self.verify(WEBUI, tika_id=TIKA)
        self.assertEqual(self.registration.call_count, 1)

    def test_changed_membership_after_role_fails_closed(self):
        self.registration.side_effect = [WEBUI, "c" * 64]
        with self.assertRaises(PermissionError):
            self.verify(WEBUI, tika_id=TIKA)
        self.assertEqual(self.registration.call_count, 2)

    def test_cli_preserves_legacy_and_accepts_combined(self):
        authorize = mock.Mock(return_value=WEBUI)
        main = load("main", {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "sys": sys,
            "shutil": SimpleNamespace(which=lambda _: "/usr/bin/docker"),
            "validate_container_id": validate,
            "authorize_registered_container_for_start": authorize,
        })
        cases = (
            (["helper", "--authorize-start", WEBUI], None),
            (
                ["helper", "--authorize-start", WEBUI,
                 "--tika-id", TIKA],
                TIKA,
            ),
        )
        for argv, tika_id in cases:
            with self.subTest(argv=argv):
                authorize.reset_mock()
                with mock.patch.object(sys, "argv", argv):
                    with contextlib.redirect_stdout(io.StringIO()) as out:
                        self.assertEqual(main(), 0)
                self.assertEqual(out.getvalue(), WEBUI + "\n")
                if tika_id is None:
                    authorize.assert_called_once_with(WEBUI)
                else:
                    authorize.assert_called_once_with(
                        WEBUI, tika_id=TIKA
                    )

    def test_cli_rejects_bad_tika_before_docker(self):
        docker = mock.Mock(side_effect=AssertionError("Docker checked"))
        main = load("main", {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "sys": sys,
            "shutil": SimpleNamespace(which=docker),
            "validate_container_id": validate,
        })
        with mock.patch.object(
            sys, "argv",
            ["helper", "--authorize-start", WEBUI,
             "--tika-id", "invalid"],
        ):
            with self.assertRaises(ValueError):
                main()
        docker.assert_not_called()


if __name__ == "__main__":
    unittest.main()
