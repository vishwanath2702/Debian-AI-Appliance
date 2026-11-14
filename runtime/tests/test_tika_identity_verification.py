import importlib.machinery
import importlib.util
import io
from contextlib import redirect_stdout
from pathlib import Path
import re
import sys
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "daia-tika-register-container"
CID = "a" * 64


def load_helper():
    pg = types.ModuleType("pgvector_registration")
    pg.read_trusted_registration = mock.Mock(return_value="b" * 64)

    def validate(value):
        if not isinstance(value, str) or not re.fullmatch(
            r"[0-9a-f]{64}", value
        ):
            raise ValueError("invalid container ID")
        return value

    pg.validate_container_id = validate

    tika = types.ModuleType("tika_registration")
    tika.EXPECTED_IMAGE = "sha256:" + "c" * 64
    tika.provision_registration_directory = mock.Mock()
    tika.register_authorized_container = mock.Mock()
    tika.verify_registered_container = mock.Mock()

    loader = importlib.machinery.SourceFileLoader(
        "tika_guard_test_module", str(HELPER)
    )
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)

    with mock.patch.dict(
        sys.modules,
        {
            "pgvector_registration": pg,
            "tika_registration": tika,
        },
    ):
        loader.exec_module(module)

    return module, tika


class TikaIdentityVerificationTests(unittest.TestCase):
    def test_read_only_verification_sequence(self):
        module, tika = load_helper()
        calls = []

        with (
            mock.patch.object(
                module, "inspect_identity",
                side_effect=lambda value: calls.append(("identity", value)),
            ),
            mock.patch.object(
                module, "inspect_security",
                side_effect=lambda value: calls.append(("security", value)),
            ),
            mock.patch.object(
                module, "inspect_mounts",
                side_effect=lambda value: calls.append(("mounts", value)),
            ),
        ):
            self.assertEqual(module.verify_registered_identity(CID), CID)

        self.assertEqual(
            calls,
            [
                ("identity", CID),
                ("security", CID),
                ("mounts", CID),
                ("identity", CID),
            ],
        )
        tika.verify_registered_container.assert_called_once_with(CID)
        tika.register_authorized_container.assert_not_called()

    def test_rejects_unregistered_container(self):
        module, tika = load_helper()
        tika.verify_registered_container.side_effect = PermissionError(
            "not registered"
        )
        with (
            mock.patch.object(module, "inspect_identity"),
            mock.patch.object(module, "inspect_security"),
            mock.patch.object(module, "inspect_mounts"),
        ):
            with self.assertRaises(PermissionError):
                module.verify_registered_identity(CID)
        tika.register_authorized_container.assert_not_called()

    def test_rejects_malformed_identity_before_docker(self):
        module, tika = load_helper()
        with mock.patch.object(module, "inspect_identity") as inspect:
            with self.assertRaises(ValueError):
                module.verify_registered_identity("not-an-id")
        inspect.assert_not_called()
        tika.verify_registered_container.assert_not_called()

    def test_cli_verify_mode_is_read_only(self):
        module, tika = load_helper()
        output = io.StringIO()
        with (
            mock.patch.object(module.os, "geteuid", return_value=0),
            mock.patch.object(module.shutil, "which", return_value="/usr/bin/docker"),
            mock.patch.object(module.sys, "argv", ["helper", "--verify-identity", CID]),
            mock.patch.object(
                module, "verify_registered_identity", return_value=CID
            ) as verify,
            mock.patch.object(module, "verify_container") as register_verify,
            redirect_stdout(output),
        ):
            self.assertEqual(module.main(), 0)

        verify.assert_called_once_with(CID)
        register_verify.assert_not_called()
        tika.provision_registration_directory.assert_not_called()
        tika.register_authorized_container.assert_not_called()
        self.assertEqual(output.getvalue(), CID + "\n")

    def test_cli_requires_root(self):
        module, tika = load_helper()
        with (
            mock.patch.object(module.os, "geteuid", return_value=1000),
            mock.patch.object(module.sys, "argv", ["helper", "--verify-identity", CID]),
        ):
            with self.assertRaises(PermissionError):
                module.main()
        tika.verify_registered_container.assert_not_called()

    def test_cli_rejects_extra_arguments(self):
        module, _ = load_helper()
        with (
            mock.patch.object(module.os, "geteuid", return_value=0),
            mock.patch.object(module.sys, "argv", [
                "helper", "--verify-identity", CID, "extra"
            ]),
        ):
            with self.assertRaises(ValueError):
                module.main()


if __name__ == "__main__":
    unittest.main()
