import ast
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
PROVISIONER = ROOT / "daia-openwebui-rag-provision.py"
SHELL = ROOT / "modules/openwebui.sh"
PG = "a" * 64
TIKA = "b" * 64
OTHER = "c" * 64


def validate(value):
    if not isinstance(value, str) or not re.fullmatch(
        r"[0-9a-f]{64}", value
    ):
        raise ValueError("Invalid immutable ID")
    return value


def load_function(name, bindings):
    tree = ast.parse(PROVISIONER.read_text(encoding="utf-8"))
    nodes = [
        node for node in tree.body
        if isinstance(node, ast.FunctionDef) and node.name == name
    ]
    if len(nodes) != 1:
        raise AssertionError("Function not unique: " + name)
    namespace = {"__name__": "isolated_rag_test", **bindings}
    exec(compile(
        ast.Module(body=nodes, type_ignores=[]),
        str(PROVISIONER), "exec"
    ), namespace)
    return namespace[name]


def shell_function(name, source):
    pattern = re.compile(
        rf"(?ms)^{re.escape(name)}\(\) \{{\n.*?^\}}\n"
    )
    matches = pattern.findall(source)
    if len(matches) != 1:
        raise AssertionError("Function not unique: " + name)
    return matches[0]


class ProvisionerTests(unittest.TestCase):
    def make_provisioner(self):
        auth = mock.Mock(return_value=PG)
        credential = mock.Mock(return_value="private-secret")
        ready = mock.Mock()
        verifier = mock.Mock(return_value="existing-scram-verifier")
        verify_password = mock.Mock()
        verify_role = mock.Mock()

        function = load_function("provision", {
            "os": SimpleNamespace(geteuid=lambda: 0),
            "ProvisioningError": RuntimeError,
            "authorize_container": auth,
            "validate_container_id": validate,
            "validate_application_credential": credential,
            "verify_postgres_ready": ready,
            "read_role_verifier": verifier,
            "verify_role_password": verify_password,
            "verify_role_state": verify_role,
        })
        return function, auth, credential, ready, verify_role

    def test_tika_authorized_before_credential_access(self):
        function, auth, credential, ready, verify_role = (
            self.make_provisioner()
        )
        self.assertTrue(function(tika_id=TIKA))
        auth.assert_called_once_with(tika_id=TIKA)
        credential.assert_called_once_with()
        ready.assert_called_once_with(PG)
        verify_role.assert_called_once_with(PG)

    def test_legacy_postgresql_only_path_preserved(self):
        function, auth, _, _, _ = self.make_provisioner()
        self.assertTrue(function())
        auth.assert_called_once_with()

    def test_invalid_tika_rejected_before_authorization(self):
        function, auth, credential, _, _ = self.make_provisioner()
        with self.assertRaises(ValueError):
            function(tika_id="invalid")
        auth.assert_not_called()
        credential.assert_not_called()

    def test_cli_routes_tika_to_provisioning(self):
        provision = mock.Mock()
        main = load_function("main", {
            "provision": provision,
            "validate_container_id": validate,
            "ProvisioningError": RuntimeError,
        })
        self.assertEqual(main(["--tika-id", TIKA]), 0)
        provision.assert_called_once_with(tika_id=TIKA)

    def test_cli_legacy_path_unchanged(self):
        provision = mock.Mock()
        main = load_function("main", {
            "provision": provision,
            "validate_container_id": validate,
            "ProvisioningError": RuntimeError,
        })
        self.assertEqual(main([]), 0)
        provision.assert_called_once_with()


class ShellPreparationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        folder = Path(temporary.name)
        self.log = folder / "calls.log"
        self.log.write_text("", encoding="utf-8")

        credentials = folder / "credentials"
        provisioner = folder / "provisioner"
        tika = folder / "tika-resolver"

        credentials.write_text(
            '#!/usr/bin/env bash\n'
            'printf "credentials\\n" >> "$MOCK_LOG"\n',
            encoding="utf-8",
        )
        provisioner.write_text(
            '#!/usr/bin/env bash\n'
            'printf "provision %s\\n" "$*" >> "$MOCK_LOG"\n',
            encoding="utf-8",
        )
        tika.write_text(
            '#!/usr/bin/env bash\n'
            'printf "tika %s\\n" "$*" >> "$MOCK_LOG"\n'
            '[[ "$1" == "--resolve-registered" ]] || exit 1\n'
            'printf "%s\\n" "${MOCK_TIKA_RESULT:-$TIKA_ID}"\n',
            encoding="utf-8",
        )
        for path in (credentials, provisioner, tika):
            path.chmod(0o700)

        source = SHELL.read_text(encoding="utf-8")
        functions = "\n".join(
            shell_function(name, source)
            for name in (
                "openwebui_require_tika_id",
                "openwebui_prepare_rag",
            )
        )
        functions = functions.replace(
            "/usr/libexec/daia-openwebui-pgvector-credentials.py",
            str(credentials),
        ).replace(
            "/usr/libexec/daia-openwebui-rag-provision.py",
            str(provisioner),
        ).replace(
            "/usr/libexec/daia-tika-register-container",
            str(tika),
        ).replace('"$EUID"', '"0"')

        self.script = (
            functions + '\nopenwebui_prepare_rag "$@"\n'
        )

    def execute(self, *args, **extra):
        result = subprocess.run(
            ["bash", "-c", self.script, "mock", *args],
            capture_output=True, text=True, check=False,
            timeout=15,
            env={
                **os.environ,
                "MOCK_LOG": str(self.log),
                "TIKA_ID": TIKA,
                **extra,
            },
        )
        return result, self.log.read_text(
            encoding="utf-8"
        ).splitlines()

    def test_combined_provisioning(self):
        result, calls = self.execute(TIKA)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, [
            "tika --resolve-registered",
            "credentials",
            "provision --tika-id " + TIKA,
        ])

    def test_legacy_provisioning(self):
        result, calls = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls, ["credentials", "provision "])

    def test_changed_tika_blocks_credential_access(self):
        result, calls = self.execute(
            TIKA, MOCK_TIKA_RESULT=OTHER
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, ["tika --resolve-registered"])

    def test_invalid_tika_blocks_provisioning(self):
        result, calls = self.execute("invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, [])

    def test_extra_arguments_rejected(self):
        result, calls = self.execute(TIKA, OTHER)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
