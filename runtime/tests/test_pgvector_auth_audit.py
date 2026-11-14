"""Inert unit tests for pgvector authentication auditing."""

import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
AUDIT_FILE = ROOT / "runtime/daia-pgvector-audit-auth.py"

spec = importlib.util.spec_from_file_location(
    "pgvector_auth_audit", AUDIT_FILE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

CONTAINER_ID = "a" * 64


class Result:
    def __init__(self, returncode=0, stdout=""):
        self.returncode = returncode
        self.stdout = stdout


class PgvectorAuthAuditTests(unittest.TestCase):

    def test_authorizer_immutable_identity(self):
        self.assertEqual(
            module.authorize(
                runner=lambda *a, **k: Result(
                    stdout=CONTAINER_ID + chr(10)
                )
            ),
            CONTAINER_ID,
        )

    def test_authorizer_rejects_extra_output_and_failures(self):
        for result in (
            Result(returncode=1, stdout=CONTAINER_ID + chr(10)),
            Result(stdout=CONTAINER_ID),
            Result(stdout=CONTAINER_ID + chr(10) + "extra" + chr(10)),
            Result(stdout="bad" + chr(10)),
        ):
            with self.subTest(stdout=result.stdout):
                with self.assertRaises(module.AuditError):
                    module.authorize(runner=lambda *a, **k: result)

    def test_fixed_read_only_scram_audit(self):
        calls = []

        def runner(command, **options):
            calls.append((command, options))
            return Result(stdout="DAIA_SCRAM_OK" + chr(10))

        self.assertTrue(
            module.audit(CONTAINER_ID, runner=runner)
        )
        self.assertEqual(len(calls), 1)

        command, options = calls[0]

        self.assertEqual(
            command[:3], ["docker", "exec", CONTAINER_ID]
        )
        self.assertIn(module.CONNINFO, command)
        self.assertIn(
            "require_auth=scram-sha-256", module.CONNINFO
        )
        self.assertIn(
            "passfile=/run/secrets/daia-pgvector-pgpass",
            module.CONNINFO,
        )
        self.assertNotIn("password=", " ".join(command))
        self.assertIn("-w", command)
        self.assertIn("-c", command)
        self.assertEqual(command[-1], module.QUERY)
        self.assertEqual(options["timeout"], 45)

    def test_invalid_id_rejected_without_docker(self):
        def forbidden(*args, **kwargs):
            self.fail("Docker mock should not be reached")

        with self.assertRaises(module.AuditError):
            module.audit(
                "not-an-identity", runner=forbidden
            )

    def test_fail_closed_on_deny_or_sql_failure(self):
        for result in (
            Result(stdout="DAIA_SCRAM_DENY" + chr(10)),
            Result(stdout=""),
            Result(
                returncode=1,
                stdout="DAIA_SCRAM_OK" + chr(10),
            ),
            Result(stdout="DAIA_SCRAM_OK" + chr(10) + "extra"),
        ):
            with self.subTest(
                stdout=result.stdout,
                code=result.returncode,
            ):
                with self.assertRaises(module.AuditError):
                    module.audit(
                        CONTAINER_ID,
                        runner=lambda *a, **k: result,
                    )

    def test_query_rejects_unsafe_file_rules(self):
        for marker in (
            "error IS NOT NULL",
            "file_name IS DISTINCT FROM",
            "auth_method IS NULL",
            "auth_method NOT IN",
            "type = 'local'",
            "type = 'host'",
            "pg_conf_load_time()",
        ):
            self.assertIn(marker, module.QUERY)

        self.assertNotIn("CREATE ", module.QUERY)
        self.assertNotIn("ALTER ", module.QUERY)
        self.assertNotIn("UPDATE ", module.QUERY)

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
            patch.object(module, "audit") as operation,
        ):
            self.assertEqual(module.main(CONTAINER_ID), 1)
            operation.assert_not_called()

    def test_main_accepts_matching_container_identity(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "authorize", return_value=CONTAINER_ID
            ),
            patch.object(module, "audit") as operation,
        ):
            self.assertEqual(module.main(CONTAINER_ID), 0)
            operation.assert_called_once_with(CONTAINER_ID)

    def test_nonroot_blocks_authorization(self):
        with (
            patch.object(module.os, "geteuid", return_value=1000),
            patch.object(module, "authorize") as authorized,
        ):
            self.assertEqual(module.main(), 1)
            authorized.assert_not_called()

    def test_packaged_but_not_activated(self):
        payload = (
            ROOT / "build/build-payload.sh"
        ).read_text()

        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-pgvector-audit-auth.py"',
            payload,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-pgvector-audit-auth.py"',
            payload,
        )

        service = (
            ROOT / "runtime/services/daia-pgvector.service"
        ).read_text()

        self.assertNotIn(
            "daia-pgvector-audit-auth.py", service
        )


if __name__ == "__main__":
    unittest.main()
