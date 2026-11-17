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
            module.authorize(
                runner=runner, webui_id="b" * 64, tika_id="c" * 64
            ),
            CONTAINER_ID,
        )
        self.assertEqual(
            calls,
            [[module.AUTHORIZER, "runtime", "b" * 64,
              "--tika-id", "c" * 64]],
        )

    def test_main_requires_pinned_identity(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "trusted_rag_identities", return_value=("", "")
            ),
            patch.object(module, "authorize") as authorized,
        ):
            self.assertEqual(module.main(), 1)
            authorized.assert_not_called()

    def test_main_rejects_changed_container_identity(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(
                module, "trusted_rag_identities", return_value=("", "")
            ),
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
                module, "trusted_rag_identities", return_value=("", "")
            ),
            patch.object(
                module, "authorize", return_value=CONTAINER_ID
            ),
            patch.object(module, "audit") as operation,
        ):
            self.assertEqual(module.main(CONTAINER_ID), 0)
            operation.assert_called_once_with(CONTAINER_ID)

    def test_postgres_only_authorizes_and_audits_without_peers(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "trusted_rag_identities") as peers,
            patch.object(module, "authorize",
                         return_value=CONTAINER_ID) as authorization,
            patch.object(module, "audit") as scram,
        ):
            self.assertEqual(
                module.main(CONTAINER_ID, postgres_only=True), 0
            )
            peers.assert_not_called()
            authorization.assert_called_once_with()
            scram.assert_called_once_with(CONTAINER_ID)

    def test_postgres_only_scram_failure_is_fatal(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "authorize",
                         return_value=CONTAINER_ID),
            patch.object(module, "audit",
                         side_effect=module.AuditError()),
        ):
            self.assertEqual(
                module.main(CONTAINER_ID, postgres_only=True), 1
            )

    def test_nonroot_blocks_authorization(self):
        with (
            patch.object(module.os, "geteuid", return_value=1000),
            patch.object(module, "authorize") as authorized,
        ):
            self.assertEqual(module.main(), 1)
            authorized.assert_not_called()


    def test_resolves_registered_peers_and_authorizes_combined(self):
        webui = "b" * 64
        tika = "c" * 64
        calls = []

        def runner(command, **options):
            calls.append(command)
            if command[:3] == ["docker", "container", "ls"]:
                return Result(stdout=chr(10).join(
                    ["daia-pgvector", "daia-tika", "open-webui", ""]
                ))
            if command[:3] == ["docker", "container", "inspect"]:
                return Result(stdout=webui + chr(10))
            if command[0].endswith("daia-openwebui-register-container"):
                return Result(stdout=webui + chr(10))
            if command[0].endswith("daia-tika-register-container"):
                return Result(stdout=tika + chr(10))
            return Result(stdout=CONTAINER_ID + chr(10))

        peers = module.trusted_rag_identities(runner=runner)
        self.assertEqual(peers, (webui, tika))
        self.assertEqual(
            module.authorize(
                runner=runner, webui_id=peers[0], tika_id=peers[1]
            ),
            CONTAINER_ID,
        )
        self.assertEqual(
            calls[-1],
            [module.AUTHORIZER, "runtime", webui, "--tika-id", tika],
        )

    def test_inventory_failure_blocks_authorization(self):
        with self.assertRaises(module.AuditError):
            module.trusted_rag_identities(
                runner=lambda *a, **k: Result(returncode=1)
            )

    def test_untrusted_webui_blocks_authorization(self):
        webui = "b" * 64

        def runner(command, **options):
            if command[:3] == ["docker", "container", "ls"]:
                return Result(stdout="open-webui" + chr(10))
            if command[:3] == ["docker", "container", "inspect"]:
                return Result(stdout=webui + chr(10))
            return Result(returncode=1)

        with self.assertRaises(module.AuditError):
            module.trusted_rag_identities(runner=runner)

    def test_untrusted_tika_blocks_authorization(self):
        def runner(command, **options):
            if command[:3] == ["docker", "container", "ls"]:
                return Result(stdout="daia-tika" + chr(10))
            return Result(returncode=1)

        with self.assertRaises(module.AuditError):
            module.trusted_rag_identities(runner=runner)

    def test_absent_peers_use_pgvector_only(self):
        def runner(command, **options):
            self.assertEqual(
                command[:3], ["docker", "container", "ls"]
            )
            return Result(stdout="daia-pgvector" + chr(10))

        self.assertEqual(
            module.trusted_rag_identities(runner=runner), ("", "")
        )

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
