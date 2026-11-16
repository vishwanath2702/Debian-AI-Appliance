import subprocess
import unittest
from unittest import mock

from test_tika_identity_verification import CID, load_helper


PG = "b" * 64
WEBUI = "d" * 64
AUTHORIZER = "/usr/libexec/daia-pgvector-authorize-start"


def result(code=0, output=""):
    return subprocess.CompletedProcess([], code, output, "")


class TikaPrestartMembershipTests(unittest.TestCase):
    def test_registered_tika_attached(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            return_value=result(0, PG + "\n"),
        ) as runner:
            module.verify_prestart_network(CID)

        self.assertEqual(runner.call_count, 1)
        self.assertEqual(
            runner.call_args.args[0],
            [AUTHORIZER, "runtime", WEBUI, "--tika-id", CID],
        )

    def test_registered_tika_not_yet_attached(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=[
                result(1),
                result(0, PG + "\n"),
            ],
        ) as runner:
            module.verify_prestart_network(CID)

        self.assertEqual(
            [call.args[0] for call in runner.call_args_list],
            [
                [AUTHORIZER, "runtime", WEBUI, "--tika-id", CID],
                [AUTHORIZER, "runtime", WEBUI],
            ],
        )

    def test_webui_and_tika_attached(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            return_value=result(0, PG + "\n"),
        ) as runner:
            module.verify_prestart_network(CID)
        self.assertEqual(
            runner.call_args.args[0],
            [AUTHORIZER, "runtime", WEBUI, "--tika-id", CID],
        )

    def test_webui_attached_tika_unattached(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=[result(1), result(0, PG + "\n")],
        ) as runner:
            module.verify_prestart_network(CID)
        self.assertEqual(
            [call.args[0] for call in runner.call_args_list],
            [
                [AUTHORIZER, "runtime", WEBUI, "--tika-id", CID],
                [AUTHORIZER, "runtime", WEBUI],
            ],
        )

    def test_postgresql_and_tika_without_webui(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=[
                result(1), result(1), result(0, PG + "\n"),
            ],
        ) as runner:
            module.verify_prestart_network(CID)
        self.assertEqual(
            runner.call_args.args[0],
            [AUTHORIZER, "runtime", "--tika-id", CID],
        )

    def test_postgresql_only_fallback(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=[
                result(1), result(1), result(1),
                result(0, PG + "\n"),
            ],
        ) as runner:
            module.verify_prestart_network(CID)
        self.assertEqual(
            runner.call_args.args[0],
            [AUTHORIZER, "runtime"],
        )

    def test_duplicate_registered_identity_rejected(self):
        module, _ = load_helper()
        module.read_trusted_registration = lambda path: PG
        with mock.patch.object(module.subprocess, "run") as runner:
            with self.assertRaises(PermissionError):
                module.verify_prestart_network(CID)
        runner.assert_not_called()

    def test_rejects_unapproved_membership(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=[result(1)] * 4,
        ):
            with self.assertRaises(PermissionError):
                module.verify_prestart_network(CID)

    def test_rejects_wrong_postgresql_identity(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            return_value=result(0, "c" * 64 + "\\n"),
        ) as runner:
            with self.assertRaises(PermissionError):
                module.verify_prestart_network(CID)
        self.assertEqual(runner.call_count, 1)

    def test_rejects_malformed_tika_identity(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(module.subprocess, "run") as runner:
            with self.assertRaises(ValueError):
                module.verify_prestart_network("invalid")
        runner.assert_not_called()

    def test_rejects_authorizer_timeout(self):
        module, _ = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=subprocess.TimeoutExpired("authorizer", 30),
        ):
            with self.assertRaises(PermissionError):
                module.verify_prestart_network(CID)

    def test_start_authorization_uses_prestart_only(self):
        module, tika = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with (
            mock.patch.object(module, "inspect_identity"),
            mock.patch.object(module, "inspect_security"),
            mock.patch.object(module, "inspect_mounts"),
            mock.patch.object(module, "inspect_stopped"),
            mock.patch.object(
                module, "verify_prestart_network"
            ) as prestart,
            mock.patch.object(
                module, "verify_runtime_network"
            ) as runtime,
        ):
            self.assertEqual(
                module.authorize_registered_start(CID), CID
            )

        self.assertEqual(prestart.call_count, 2)
        runtime.assert_not_called()
        tika.verify_registered_container.assert_called_once_with(CID)

    def test_running_verification_remains_strict(self):
        module, tika = load_helper()
        module.read_trusted_registration = (
            lambda path: WEBUI if path.endswith("/openwebui") else PG
        )
        with (
            mock.patch.object(module, "inspect_identity"),
            mock.patch.object(module, "inspect_security"),
            mock.patch.object(module, "inspect_mounts"),
            mock.patch.object(module, "inspect_running"),
            mock.patch.object(
                module, "verify_prestart_network"
            ) as prestart,
            mock.patch.object(
                module, "verify_runtime_network"
            ) as runtime,
        ):
            self.assertEqual(
                module.verify_registered_running(CID), CID
            )

        prestart.assert_not_called()
        self.assertEqual(runtime.call_count, 2)
        tika.verify_registered_container.assert_called_once_with(CID)


if __name__ == "__main__":
    unittest.main()
