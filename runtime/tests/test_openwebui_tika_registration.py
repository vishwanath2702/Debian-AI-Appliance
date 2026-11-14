import ast
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
TIKA = "b" * 64
WEBUI = "c" * 64


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
        raise AssertionError("Function not found")
    namespace = dict(bindings)
    exec(
        compile(
            ast.Module(body=nodes, type_ignores=[]),
            str(SOURCE), "exec",
        ),
        namespace,
    )
    return namespace[name]


class TikaRegistrationTests(unittest.TestCase):
    def setUp(self):
        self.network = mock.Mock()
        self.legacy = mock.Mock()
        self.calls = []
        self.run = mock.Mock(
            return_value=SimpleNamespace(returncode=0, stdout="")
        )
        self.bindings = {
            "validate_container_id": validate,
            "inspect_container_identity": lambda x: x,
            "inspect_container_stopped": lambda x: None,
            "inspect_container_security": lambda x: None,
            "inspect_container_mounts": lambda x: None,
            "inspect_volume_ownership": lambda: None,
            "read_registered_postgresql_id": lambda: PG,
            "verify_registration_network": self.legacy,
            "revalidate_container_before_registration": lambda x: x,
            "subprocess": SimpleNamespace(run=self.run),
        }
        self.verify = load(
            "verify_container_for_registration", self.bindings
        )

    def test_postgresql_only_keeps_original_verifier(self):
        self.assertEqual(self.verify(WEBUI), WEBUI)
        self.legacy.assert_called_once_with(PG)
        self.run.assert_not_called()

    def test_combined_registration_checks_trusted_network(self):
        self.assertEqual(self.verify(WEBUI, tika_id=TIKA), WEBUI)
        self.legacy.assert_not_called()
        self.assertEqual(
            self.run.call_args.args[0],
            [
                "/usr/libexec/daia-rag-verify-network",
                "registration", PG, TIKA,
            ],
        )
        self.assertTrue(self.run.call_args.kwargs["check"])

    def test_invalid_and_duplicate_tika_rejected(self):
        for value in ("invalid", PG, WEBUI):
            with self.subTest(value=value):
                with self.assertRaises((ValueError, PermissionError)):
                    self.verify(WEBUI, tika_id=value)
        self.run.assert_not_called()

    def test_combined_network_failure_blocks_registration(self):
        self.run.side_effect = subprocess.CalledProcessError(
            1, "verify-network"
        )
        with self.assertRaises(subprocess.CalledProcessError):
            self.verify(WEBUI, tika_id=TIKA)

    def test_main_legacy_and_combined_dispatch(self):
        verify = mock.Mock(return_value=WEBUI)
        register = mock.Mock()
        namespace = {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "sys": sys,
            "shutil": SimpleNamespace(which=lambda _: "/usr/bin/docker"),
            "validate_container_id": validate,
            "verify_container_for_registration": verify,
            "provision_registration_directory": mock.Mock(),
            "register_authorized_container": register,
            "EXPECTED_IMAGE": "sha256:" + "d" * 64,
        }
        main = load("main", namespace)

        for argv, expected in (
            (["helper", WEBUI], None),
            (["helper", "--tika-id", TIKA, WEBUI], TIKA),
        ):
            with self.subTest(argv=argv):
                verify.reset_mock()
                with mock.patch.object(sys, "argv", argv):
                    self.assertEqual(main(), 0)
                if expected is None:
                    verify.assert_called_once_with(WEBUI)
                else:
                    verify.assert_called_once_with(
                        WEBUI, tika_id=expected
                    )

    def test_main_rejects_invalid_tika_before_docker(self):
        verify = mock.Mock()
        main = load("main", {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "sys": sys,
            "validate_container_id": validate,
            "shutil": SimpleNamespace(
                which=mock.Mock(side_effect=AssertionError(
                    "Docker must not be checked"
                ))
            ),
            "verify_container_for_registration": verify,
        })
        with mock.patch.object(
            sys, "argv", ["helper", "--tika-id", "bad", WEBUI]
        ):
            with self.assertRaises(ValueError):
                main()
        verify.assert_not_called()


if __name__ == "__main__":
    unittest.main()
