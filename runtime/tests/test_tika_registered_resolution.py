import ast
import contextlib
import io
import re
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "daia-tika-register-container"
TIKA = "a" * 64
OTHER = "b" * 64


def validate(value):
    if not isinstance(value, str) or not re.fullmatch(
        r"[0-9a-f]{64}", value
    ):
        raise ValueError("Invalid container ID")
    return value


def load(name, bindings):
    tree = ast.parse(SOURCE.read_text(encoding="utf-8"))
    matches = [
        node for node in tree.body
        if isinstance(node, ast.FunctionDef) and node.name == name
    ]
    if len(matches) != 1:
        raise AssertionError("Function not unique")
    namespace = {"__name__": "tika_resolution_tests", **bindings}
    exec(compile(
        ast.Module(body=matches, type_ignores=[]),
        str(SOURCE), "exec",
    ), namespace)
    return namespace[name]


class RegisteredResolutionTests(unittest.TestCase):
    def make_resolver(self, registration=None, verify=None, uid=0):
        if registration is None:
            registration = mock.Mock(return_value=TIKA)
        if verify is None:
            verify = mock.Mock(return_value=TIKA)
        resolver = load("resolve_registered_tika_id", {
            "os": SimpleNamespace(geteuid=lambda: uid),
            "validate_container_id": validate,
            "read_trusted_registration": registration,
            "verify_registered_identity": verify,
        })
        return resolver, registration, verify

    def test_resolves_only_from_trusted_registration(self):
        resolver, registration, verify = self.make_resolver()
        self.assertEqual(resolver(), TIKA)
        self.assertEqual(registration.call_count, 2)
        registration.assert_has_calls([
            mock.call("/var/lib/daia/tika"),
            mock.call("/var/lib/daia/tika"),
        ])
        verify.assert_called_once_with(TIKA)

    def test_nonroot_rejected_before_registration(self):
        resolver, registration, verify = self.make_resolver(uid=1000)
        with self.assertRaises(PermissionError):
            resolver()
        registration.assert_not_called()
        verify.assert_not_called()

    def test_invalid_registration_rejected_before_docker(self):
        resolver, _, verify = self.make_resolver(
            registration=mock.Mock(return_value="bad")
        )
        with self.assertRaises(ValueError):
            resolver()
        verify.assert_not_called()

    def test_missing_registration_fails_closed(self):
        resolver, _, verify = self.make_resolver(
            registration=mock.Mock(
                side_effect=FileNotFoundError("missing registration")
            )
        )
        with self.assertRaises(FileNotFoundError):
            resolver()
        verify.assert_not_called()

    def test_untrusted_identity_rejected(self):
        resolver, registration, verify = self.make_resolver(
            verify=mock.Mock(side_effect=PermissionError("untrusted"))
        )
        with self.assertRaises(PermissionError):
            resolver()
        self.assertEqual(registration.call_count, 1)
        verify.assert_called_once_with(TIKA)

    def test_mismatched_verified_identity_rejected(self):
        resolver, registration, _ = self.make_resolver(
            verify=mock.Mock(return_value=OTHER)
        )
        with self.assertRaises(PermissionError):
            resolver()
        self.assertEqual(registration.call_count, 1)

    def test_changed_registration_rejected(self):
        registration = mock.Mock(side_effect=[TIKA, OTHER])
        resolver, _, verify = self.make_resolver(
            registration=registration
        )
        with self.assertRaises(PermissionError):
            resolver()
        verify.assert_called_once_with(TIKA)

    def test_cli_prints_only_verified_immutable_id(self):
        resolver = mock.Mock(return_value=TIKA)
        docker = mock.Mock(return_value="/usr/bin/docker")
        main = load("main", {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "sys": sys,
            "shutil": SimpleNamespace(which=docker),
            "resolve_registered_tika_id": resolver,
        })
        with mock.patch.object(
            sys, "argv", ["helper", "--resolve-registered"]
        ):
            with contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(main(), 0)
        self.assertEqual(output.getvalue(), TIKA + "\n")
        resolver.assert_called_once_with()
        docker.assert_called_once_with("docker")

    def test_cli_rejects_missing_docker(self):
        resolver = mock.Mock()
        main = load("main", {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "sys": sys,
            "shutil": SimpleNamespace(which=lambda _: None),
            "resolve_registered_tika_id": resolver,
        })
        with mock.patch.object(
            sys, "argv", ["helper", "--resolve-registered"]
        ):
            with self.assertRaises(RuntimeError):
                main()
        resolver.assert_not_called()

    def test_cli_rejects_nonroot_before_resolution(self):
        resolver = mock.Mock()
        main = load("main", {
            "os": SimpleNamespace(geteuid=lambda: 1000),
            "sys": sys,
            "resolve_registered_tika_id": resolver,
        })
        with mock.patch.object(
            sys, "argv", ["helper", "--resolve-registered"]
        ):
            with self.assertRaises(PermissionError):
                main()
        resolver.assert_not_called()


if __name__ == "__main__":
    unittest.main()
