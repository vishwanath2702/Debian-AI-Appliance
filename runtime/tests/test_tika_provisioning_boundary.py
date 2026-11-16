import io
import subprocess
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

from test_tika_identity_verification import CID, load_helper

ROOT = Path(__file__).resolve().parents[1]
PG = "b" * 64
AUTH = "/usr/libexec/daia-pgvector-authorize-start"


def result(code=0, output=""):
    return subprocess.CompletedProcess([], code, output, "")


class ProvisioningBoundaryTests(unittest.TestCase):
    def test_prestart_pg_tika(self):
        module, _ = load_helper()
        with mock.patch.object(
            module.subprocess, "run",
            return_value=result(0, PG + "\n"),
        ) as runner:
            module.verify_provisioning_network(CID, prestart=True)
        self.assertEqual(
            runner.call_args.args[0],
            [AUTH, "runtime", "--tika-id", CID],
        )

    def test_prestart_pg_only(self):
        module, _ = load_helper()
        with mock.patch.object(
            module.subprocess, "run",
            side_effect=[
                result(1),
                result(0, PG + "\n"),
            ],
        ) as runner:
            module.verify_provisioning_network(CID, prestart=True)
        self.assertEqual(
            runner.call_args_list[-1].args[0],
            [AUTH, "runtime"],
        )

    def test_running_requires_pg_and_tika(self):
        module, _ = load_helper()
        with mock.patch.object(
            module.subprocess, "run",
            return_value=result(0, PG + "\n"),
        ) as runner:
            module.verify_provisioning_network(CID)
        self.assertEqual(runner.call_count, 1)
        self.assertEqual(
            runner.call_args.args[0],
            [AUTH, "runtime", "--tika-id", CID],
        )

    def test_running_rejects_unapproved_membership(self):
        module, _ = load_helper()
        with mock.patch.object(
            module.subprocess, "run",
            return_value=result(1),
        ):
            with self.assertRaises(PermissionError):
                module.verify_provisioning_network(CID)

    def test_wrong_pg_identity_rejected(self):
        module, _ = load_helper()
        with mock.patch.object(
            module.subprocess, "run",
            return_value=result(0, "c" * 64 + "\n"),
        ):
            with self.assertRaises(PermissionError):
                module.verify_provisioning_network(CID)

    def test_duplicate_identity_rejected(self):
        module, _ = load_helper()
        module.read_trusted_registration = lambda path: CID
        with mock.patch.object(module.subprocess, "run") as runner:
            with self.assertRaises(PermissionError):
                module.verify_provisioning_network(CID)
        runner.assert_not_called()

    def test_running_rechecks_state_and_network(self):
        module, tika = load_helper()
        with (
            mock.patch.object(module, "inspect_identity") as identity,
            mock.patch.object(module, "inspect_security") as security,
            mock.patch.object(module, "inspect_mounts") as mounts,
            mock.patch.object(module, "inspect_running") as running,
            mock.patch.object(module, "verify_provisioning_network") as net,
        ):
            self.assertEqual(module.verify_provisioning_running(CID), CID)
        self.assertEqual(identity.call_count, 2)
        security.assert_called_once_with(CID)
        mounts.assert_called_once_with(CID)
        self.assertEqual(running.call_count, 2)
        self.assertEqual(net.call_count, 2)
        tika.verify_registered_container.assert_called_once_with(CID)

    def test_cli_provisioning_modes_read_only(self):
        module, tika = load_helper()
        for mode, handler in (
            ("--authorize-provisioning-start",
             "authorize_provisioning_start"),
            ("--verify-provisioning-running",
             "verify_provisioning_running"),
        ):
            with self.subTest(mode=mode):
                output = io.StringIO()
                with (
                    mock.patch.object(module.os, "geteuid", return_value=0),
                    mock.patch.object(
                        module.shutil, "which",
                        return_value="/usr/bin/docker",
                    ),
                    mock.patch.object(
                        module.sys, "argv", ["helper", mode, CID]
                    ),
                    mock.patch.object(
                        module, handler, return_value=CID
                    ) as verifier,
                    mock.patch.object(
                        module, "verify_container"
                    ) as register,
                    redirect_stdout(output),
                ):
                    self.assertEqual(module.main(), 0)
                verifier.assert_called_once_with(CID)
                register.assert_not_called()
                self.assertEqual(output.getvalue(), CID + "\n")
        tika.register_authorized_container.assert_not_called()

    def test_new_container_only_uses_provisioning(self):
        source = (ROOT / "modules/tika.sh").read_text()
        install = source.split(
            "tika_install_registered() {", 1
        )[1].split("\ntika_install() {", 1)[0]
        self.assertIn(
            'tika_start_registered "$container_id" provisioning',
            install,
        )
        self.assertIn("tika_verify_registered provisioning", install)
        self.assertIn(
            'tika_start_registered "$container_id" || return 1',
            install,
        )
        self.assertIn("tika_verify_registered || return 1", install)

    def test_recovery_handlers_unchanged(self):
        module, _ = load_helper()
        self.assertIsNot(
            module.verify_registered_running,
            module.verify_provisioning_running,
        )
        self.assertIsNot(
            module.authorize_registered_start,
            module.authorize_provisioning_start,
        )


if __name__ == "__main__":
    unittest.main()
