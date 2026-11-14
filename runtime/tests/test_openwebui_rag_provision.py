"""Fail-closed DAIA RAG provisioning boundary tests."""

import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

HELPER = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-rag-provision.py"
)
spec = importlib.util.spec_from_file_location("rag_provision", HELPER)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ProvisioningTests(unittest.TestCase):
    def test_rejects_invalid_container_identity(self):
        for value in ("", "daia-pgvector", "a" * 63, "A" * 64):
            with self.assertRaises(module.ProvisioningError):
                module.validate_container_id(value)

    def test_accepts_immutable_container_identity(self):
        self.assertEqual(
            module.validate_container_id("a" * 64),
            "a" * 64,
        )

    def test_rejects_authorization_failure(self):
        class Result:
            returncode = 1
            stdout = ""
        with self.assertRaises(module.ProvisioningError):
            module.authorize_container(runner=lambda *a, **k: Result())

    def test_rejects_malformed_authorizer_output(self):
        class Result:
            returncode = 0
            stdout = "container-name\\n"
        with self.assertRaises(module.ProvisioningError):
            module.authorize_container(runner=lambda *a, **k: Result())

    def test_executes_only_after_authorization_and_readiness(self):
        from unittest.mock import Mock

        calls = Mock()
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "authorize_container",
                         side_effect=lambda: (
                             calls.authorize(), "a" * 64
                         )[1]),
            patch.object(module, "validate_application_credential",
                         side_effect=lambda: (
                             calls.credential(), b"A" * 48
                         )[1]),
            patch.object(module, "verify_postgres_ready",
                         side_effect=lambda identity: calls.ready(identity)),
            patch.object(
                module, "read_role_verifier",
                side_effect=[None, "verified-scram"],
            ),
            patch.object(
                module, "verify_role_password",
                side_effect=lambda *args: calls.password(*args),
            ),
            patch.object(module, "execute_role_sql",
                         side_effect=lambda *args: calls.execute(*args)),
            patch.object(module, "verify_role_state",
                         side_effect=lambda identity: calls.verify(identity)),
        ):
            self.assertTrue(module.provision())

        self.assertEqual(
            [item[0] for item in calls.mock_calls],
            ["authorize", "credential", "ready",
             "execute", "verify", "password"],
        )

    def test_credential_failure_blocks_readiness(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "authorize_container", return_value="a" * 64),
            patch.object(
                module, "validate_application_credential",
                side_effect=module.ProvisioningError("Unsafe credential")
            ),
            patch.object(module, "verify_postgres_ready") as readiness,
        ):
            with self.assertRaises(module.ProvisioningError):
                module.provision()
            readiness.assert_not_called()

    def test_readiness_failure_blocks_provisioning(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "authorize_container", return_value="a" * 64),
            patch.object(module, "validate_application_credential",
                         return_value=True),
            patch.object(
                module, "verify_postgres_ready",
                side_effect=module.ProvisioningError("Not ready")
            ),
        ):
            with self.assertRaises(module.ProvisioningError):
                module.provision()

    def test_readiness_uses_immutable_id(self):
        class Result:
            returncode = 0

        calls = []

        def runner(args, **kwargs):
            calls.append(args)
            return Result()

        self.assertTrue(
            module.verify_postgres_ready("a" * 64, runner=runner)
        )
        self.assertEqual(calls[0][0:3], [
            "docker", "exec", "a" * 64
        ])

    def test_readiness_rejects_invalid_id_without_docker(self):
        with patch.object(module.subprocess, "run") as runner:
            with self.assertRaises(module.ProvisioningError):
                module.verify_postgres_ready("daia-pgvector")
            runner.assert_not_called()

    def test_readiness_rejects_failure(self):
        class Result:
            returncode = 1

        with self.assertRaises(module.ProvisioningError):
            module.verify_postgres_ready(
                "a" * 64,
                runner=lambda *args, **kwargs: Result(),
            )

    def test_nonroot_rejected_before_authorization(self):
        with (
            patch.object(module.os, "geteuid", return_value=1000),
            patch.object(module, "authorize_container") as authorize,
        ):
            with self.assertRaises(module.ProvisioningError):
                module.provision()
            authorize.assert_not_called()


class CredentialSecurityTests(unittest.TestCase):
    """Exercise the real file-descriptor credential validator."""

    def setUp(self):
        import tempfile
        import os

        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.directory_fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY,
        )
        self.addCleanup(os.close, self.directory_fd)

    def write_secret(self, data=b"valid_password_123", mode=0o600):
        import os

        path = self.directory / module.SECRET_NAME
        path.write_bytes(data)
        os.chmod(path, mode)
        return path

    def test_accepts_valid_root_owned_secret(self):
        import os

        if os.geteuid() != 0:
            self.skipTest("Requires root-owned test fixture")
        self.write_secret()
        self.assertTrue(module.validate_secret(self.directory_fd))

    def test_rejects_world_readable_secret(self):
        self.write_secret(mode=0o644)
        with self.assertRaises(module.ProvisioningError):
            module.validate_secret(self.directory_fd)

    def test_rejects_malformed_secret(self):
        self.write_secret(b"invalid password with spaces")
        with self.assertRaises(module.ProvisioningError):
            module.validate_secret(self.directory_fd)

    def test_rejects_empty_secret(self):
        self.write_secret(b"")
        with self.assertRaises(module.ProvisioningError):
            module.validate_secret(self.directory_fd)

    def test_rejects_symlink(self):
        import os

        target = self.directory / "target"
        target.write_bytes(b"valid_password_123")
        os.symlink(target, self.directory / module.SECRET_NAME)

        with self.assertRaises(OSError):
            module.validate_secret(self.directory_fd)

    def test_rejects_hardlinked_secret(self):
        import os

        path = self.write_secret()
        os.link(path, self.directory / "second-link")

        with self.assertRaises(module.ProvisioningError):
            module.validate_secret(self.directory_fd)

    def test_rejects_nonregular_secret(self):
        (self.directory / module.SECRET_NAME).mkdir()
        with self.assertRaises(module.ProvisioningError):
            module.validate_secret(self.directory_fd)

    def test_rejects_oversized_secret(self):
        self.write_secret(b"a" * 257)
        with self.assertRaises(module.ProvisioningError):
            module.validate_secret(self.directory_fd)

    def test_rejects_wrong_owner(self):
        import os

        if os.geteuid() != 0:
            self.skipTest("Requires root to change file ownership")

        path = self.write_secret()
        os.chown(path, 65534, 65534)

        with self.assertRaises(module.ProvisioningError):
            module.validate_secret(self.directory_fd)


if __name__ == "__main__":
    unittest.main()
