import io
import subprocess
import unittest
from contextlib import redirect_stdout
from unittest import mock

from test_tika_identity_verification import load_helper

CID = "a" * 64
PGID = "b" * 64


class TikaGuardedAuthorizationTests(unittest.TestCase):
    def test_start_authorization_requires_trust_and_stopped_state(self):
        module, tika = load_helper()
        events = []

        names = (
            "inspect_identity",
            "inspect_security",
            "inspect_mounts",
            "inspect_stopped",
            "verify_prestart_network",
        )
        patches = [
            mock.patch.object(
                module, name,
                side_effect=lambda value, name=name:
                    events.append(name),
            )
            for name in names
        ]

        with patches[0], patches[1], patches[2], patches[3], patches[4]:
            self.assertEqual(
                module.authorize_registered_start(CID), CID
            )

        self.assertEqual(events, [
            "inspect_identity",
            "inspect_security",
            "inspect_mounts",
            "inspect_stopped",
            "verify_prestart_network",
            "inspect_identity",
            "inspect_stopped",
            "verify_prestart_network",
        ])
        tika.verify_registered_container.assert_called_once_with(CID)
        tika.register_authorized_container.assert_not_called()

    def test_running_verification_rechecks_state_and_network(self):
        module, tika = load_helper()
        with (
            mock.patch.object(module, "inspect_identity") as identity,
            mock.patch.object(module, "inspect_security") as security,
            mock.patch.object(module, "inspect_mounts") as mounts,
            mock.patch.object(module, "inspect_running") as running,
            mock.patch.object(module, "verify_runtime_network") as network,
        ):
            self.assertEqual(
                module.verify_registered_running(CID), CID
            )

        self.assertEqual(identity.call_count, 2)
        security.assert_called_once_with(CID)
        mounts.assert_called_once_with(CID)
        self.assertEqual(running.call_count, 2)
        self.assertEqual(network.call_count, 2)
        tika.verify_registered_container.assert_called_once_with(CID)

    def test_unregistered_container_cannot_be_authorized(self):
        module, tika = load_helper()
        tika.verify_registered_container.side_effect = PermissionError(
            "unregistered"
        )
        with (
            mock.patch.object(module, "inspect_identity"),
            mock.patch.object(module, "inspect_security"),
            mock.patch.object(module, "inspect_mounts"),
            mock.patch.object(module, "inspect_stopped"),
            mock.patch.object(module, "verify_runtime_network") as network,
        ):
            with self.assertRaises(PermissionError):
                module.authorize_registered_start(CID)
        network.assert_not_called()

    def test_network_authorization_requires_exact_pg_identity(self):
        module, _ = load_helper()
        result = subprocess.CompletedProcess(
            [], 0, stdout=PGID + "\n", stderr=""
        )
        with mock.patch.object(
            module.subprocess, "run", return_value=result
        ) as runner:
            module.verify_runtime_network(CID)

        self.assertEqual(runner.call_args.args[0], [
            "/usr/libexec/daia-pgvector-authorize-start",
            "runtime", "--tika-id", CID,
        ])
        self.assertTrue(runner.call_args.kwargs["check"])

    def test_network_identity_mismatch_fails_closed(self):
        module, _ = load_helper()
        result = subprocess.CompletedProcess(
            [], 0, stdout="d" * 64 + "\n", stderr=""
        )
        with mock.patch.object(
            module.subprocess, "run", return_value=result
        ):
            with self.assertRaises(PermissionError):
                module.verify_runtime_network(CID)

    def test_stopped_state_rejects_running_container(self):
        module, _ = load_helper()
        with mock.patch.object(
            module, "inspect",
            return_value="true|false|false|running",
        ):
            with self.assertRaises(PermissionError):
                module.inspect_stopped(CID)

    def test_running_state_rejects_paused_container(self):
        module, _ = load_helper()
        with mock.patch.object(
            module, "inspect",
            return_value="true|true|false|paused",
        ):
            with self.assertRaises(PermissionError):
                module.inspect_running(CID)

    def test_cli_modes_do_not_register_or_mutate(self):
        for mode, function in (
            ("--authorize-start", "authorize_registered_start"),
            ("--verify-running", "verify_registered_running"),
        ):
            with self.subTest(mode=mode):
                module, tika = load_helper()
                output = io.StringIO()
                with (
                    mock.patch.object(
                        module.os, "geteuid", return_value=0
                    ),
                    mock.patch.object(
                        module.shutil, "which",
                        return_value="/usr/bin/docker",
                    ),
                    mock.patch.object(
                        module.sys, "argv", ["helper", mode, CID]
                    ),
                    mock.patch.object(
                        module, function, return_value=CID
                    ) as verifier,
                    mock.patch.object(
                        module, "verify_container"
                    ) as register,
                    redirect_stdout(output),
                ):
                    self.assertEqual(module.main(), 0)

                verifier.assert_called_once_with(CID)
                register.assert_not_called()
                tika.register_authorized_container.assert_not_called()
                self.assertEqual(output.getvalue(), CID + "\n")

    def test_cli_rejects_unrecognized_mode(self):
        module, _ = load_helper()
        with (
            mock.patch.object(
                module.os, "geteuid", return_value=0
            ),
            mock.patch.object(
                module.sys, "argv",
                ["helper", "--unsafe-mode", CID],
            ),
        ):
            with self.assertRaises(ValueError):
                module.main()


if __name__ == "__main__":
    unittest.main()
