"""Mocked tests for pgvector extension initialization."""

import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

HELPER = (
    Path(__file__).resolve().parents[1]
    / "daia-pgvector-init-extension.py"
)
spec = importlib.util.spec_from_file_location("pgvector_init_extension", HELPER)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

CONTAINER_ID = "a" * 64


class Result:
    def __init__(self, returncode=0, stdout=""):
        self.returncode = returncode
        self.stdout = stdout


class InitializationTests(unittest.TestCase):
    def test_fixed_extension_sql(self):
        self.assertEqual(
            module.SQL,
            "CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;\n",
        )

    def test_authorization_returns_immutable_id(self):
        self.assertEqual(
            module.authorize(
                runner=lambda *a, **k: Result(stdout=CONTAINER_ID + "\n")
            ),
            CONTAINER_ID,
        )

    def test_authorization_failure_blocks_execution(self):
        with self.assertRaises(module.InitializationError):
            module.authorize(
                runner=lambda *a, **k: Result(returncode=1)
            )

    def test_invalid_identity_rejected_before_docker(self):
        with patch.object(module.subprocess, "run") as runner:
            with self.assertRaises(module.InitializationError):
                module.initialize("daia-pgvector")
            runner.assert_not_called()

    def test_executes_fixed_sql_on_immutable_container(self):
        calls = []

        def runner(command, **kwargs):
            calls.append((command, kwargs))
            if "-c" in command:
                return Result(stdout="vector\n")
            return Result()

        self.assertTrue(module.initialize(CONTAINER_ID, runner=runner))
        command, options = calls[0]
        self.assertEqual(
            command[:4], ["docker", "exec", "-i", CONTAINER_ID]
        )
        self.assertEqual(options["input"], module.SQL)
        self.assertIn("ON_ERROR_STOP=1", command)

    def test_sql_failure_propagates(self):
        with self.assertRaises(module.InitializationError):
            module.initialize(
                CONTAINER_ID,
                runner=lambda *a, **k: Result(returncode=1),
            )

    def test_rejects_authorization_with_extra_whitespace(self):
        for output in (
            CONTAINER_ID,
            " " + CONTAINER_ID + "\n",
            CONTAINER_ID + "\nextra\n",
        ):
            with self.subTest(output=output):
                with self.assertRaises(module.InitializationError):
                    module.authorize(
                        runner=lambda *a, **k: Result(stdout=output)
                    )

    def test_verifies_extension_after_initialization(self):
        calls = []

        def runner(command, **kwargs):
            calls.append(command)
            if "-c" in command:
                return Result(stdout="vector\n")
            return Result()

        self.assertTrue(module.initialize(CONTAINER_ID, runner=runner))
        self.assertEqual(len(calls), 2)
        self.assertIn("FROM pg_extension", calls[1][-1])

    def test_rejects_missing_extension_after_successful_sql(self):
        with self.assertRaises(module.InitializationError):
            module.initialize(
                CONTAINER_ID,
                runner=lambda *a, **k: Result(stdout=""),
            )

    def test_scram_passfile_connection_contract(self):
        calls = []

        def runner(command, **kwargs):
            calls.append(command)
            if "-c" in command:
                return Result(stdout="vector" + chr(10))
            return Result()

        self.assertTrue(module.initialize(CONTAINER_ID, runner=runner))
        self.assertEqual(len(calls), 2)

        for command in calls:
            self.assertIn(module.CONNINFO, command)
            self.assertIn(
                "require_auth=scram-sha-256", module.CONNINFO
            )
            self.assertIn(
                "passfile=/run/secrets/daia-pgvector-pgpass",
                module.CONNINFO,
            )
            self.assertIn("-w", command)
            self.assertNotIn("password=", " ".join(command))

    def test_packaging_stages_initializer(self):
        root = Path(__file__).resolve().parents[2]
        payload = (root / "build/build-payload.sh").read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-pgvector-init-extension.py"',
            payload,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-pgvector-init-extension.py"',
            payload,
        )
        service = (
            root / "runtime/services/daia-pgvector.service"
        ).read_text()
        self.assertNotIn("daia-pgvector-init-extension.py", service)

    def test_authorizer_requests_runtime_mode(self):
        calls = []

        def runner(command, **options):
            calls.append(command)
            return Result(stdout=CONTAINER_ID + chr(10))

        self.assertEqual(
            module.authorize(runner=runner),
            CONTAINER_ID,
        )
        self.assertEqual(
            calls, [[module.AUTHORIZER, "runtime"]]
        )

    def test_main_requires_pinned_identity(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "authorize") as authorized,
        ):
            self.assertEqual(module.main(), 1)
            authorized.assert_not_called()

    def test_main_rejects_changed_container_identity(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize", return_value="b" * 64
            ),
            patch.object(module, "initialize") as operation,
        ):
            self.assertEqual(module.main(CONTAINER_ID), 1)
            operation.assert_not_called()

    def test_main_accepts_matching_container_identity(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize", return_value=CONTAINER_ID
            ),
            patch.object(module, "initialize") as operation,
        ):
            self.assertEqual(module.main(CONTAINER_ID), 0)
            operation.assert_called_once_with(CONTAINER_ID)

    def test_nonroot_rejected_before_authorization(self):
        with (
            patch.object(module.os, "geteuid", return_value=1000),
            patch.object(module, "authorize") as authorize,
        ):
            self.assertEqual(module.main(), 1)
            authorize.assert_not_called()


if __name__ == "__main__":
    unittest.main()
