"""Isolated tests for trusted Open WebUI registration network checks."""

import importlib.machinery
import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
HELPER = RUNTIME / "daia-openwebui-register-container"

sys.path.insert(0, str(RUNTIME))

loader = importlib.machinery.SourceFileLoader(
    "daia_openwebui_registration_network_authorization",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)

PG_ID = "a" * 64


class OpenWebUIRegistrationNetworkAuthorizationTests(unittest.TestCase):

    def test_reads_postgresql_id_from_trusted_registration(self):
        with patch.object(
            helper,
            "read_trusted_registration",
            return_value=PG_ID,
            create=True,
        ) as reader:
            result = helper.read_registered_postgresql_id()

        self.assertEqual(result, PG_ID)
        reader.assert_called_once_with("/var/lib/daia/pgvector")

    def test_rejects_invalid_trusted_postgresql_id(self):
        with patch.object(
            helper,
            "read_trusted_registration",
            return_value="invalid",
            create=True,
        ):
            with self.assertRaises((ValueError, PermissionError)):
                helper.read_registered_postgresql_id()

    def test_rejects_missing_postgresql_registration(self):
        with patch.object(
            helper,
            "read_trusted_registration",
            side_effect=FileNotFoundError,
            create=True,
        ):
            with self.assertRaises(FileNotFoundError):
                helper.read_registered_postgresql_id()

    def test_network_verifier_uses_trusted_postgresql_id(self):
        with patch.object(helper.subprocess, "run") as runner:
            helper.verify_registration_network(PG_ID)

        runner.assert_called_once()
        args = runner.call_args.args[0]

        self.assertEqual(
            args,
            [
                "/usr/libexec/daia-rag-verify-network",
                "registration",
                PG_ID,
            ],
        )

        self.assertTrue(runner.call_args.kwargs["check"])
        self.assertIn("timeout", runner.call_args.kwargs)

    def test_network_verification_failure_is_fatal(self):
        import subprocess

        with patch.object(
            helper.subprocess,
            "run",
            side_effect=subprocess.CalledProcessError(
                1,
                ["/usr/libexec/daia-rag-verify-network"],
            ),
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                helper.verify_registration_network(PG_ID)


if __name__ == "__main__":
    unittest.main()
