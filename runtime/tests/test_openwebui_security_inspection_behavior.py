"""Isolated tests for Open WebUI Docker security inspection."""

import importlib.machinery
import importlib.util
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
sys.path.insert(0, str(RUNTIME))

HELPER = RUNTIME / "daia-openwebui-register-container"
CONTAINER_ID = "a" * 64

loader = importlib.machinery.SourceFileLoader(
    "daia_openwebui_security_inspection",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)
registration = importlib.util.module_from_spec(spec)
loader.exec_module(registration)


class OpenWebUISecurityInspectionTests(unittest.TestCase):

    def inspect(self, security, network, *, fail_at=None):
        calls = []

        def fake_run(args, **kwargs):
            calls.append(args)

            self.assertEqual(
                args[:3],
                ["docker", "container", "inspect"],
            )
            self.assertEqual(args[-1], CONTAINER_ID)
            self.assertTrue(kwargs["check"])
            self.assertEqual(kwargs["timeout"], 15)

            index = len(calls)

            if fail_at == index:
                raise subprocess.CalledProcessError(1, args)

            output = security if index == 1 else network

            return subprocess.CompletedProcess(
                args, 0, stdout=output, stderr=""
            )

        with patch.object(
            registration.subprocess,
            "run",
            side_effect=fake_run,
        ):
            registration.inspect_container_security(CONTAINER_ID)

        self.assertEqual(len(calls), 2)

    def test_accepts_exact_security_and_private_network(self):
        self.inspect(
            "no-new-privileges:true|false\n",
            "daia-rag-internal\n",
        )

    def test_rejects_missing_no_new_privileges(self):
        with self.assertRaises(PermissionError):
            self.inspect("|false\n", "daia-rag-internal\n")

    def test_rejects_privileged_container(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                "no-new-privileges:true|true\n",
                "daia-rag-internal\n",
            )

    def test_rejects_additional_security_options(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                "no-new-privileges:true,seccomp=unconfined|false\n",
                "daia-rag-internal\n",
            )

    def test_rejects_host_network(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                "no-new-privileges:true|false\n",
                "host\n",
            )

    def test_rejects_bridge_network(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                "no-new-privileges:true|false\n",
                "bridge\n",
            )

    def test_rejects_extra_network_output(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                "no-new-privileges:true|false\n",
                "daia-rag-internal\nhost\n",
            )

    def test_rejects_security_inspection_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inspect(
                "",
                "daia-rag-internal\n",
                fail_at=1,
            )

    def test_rejects_network_inspection_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inspect(
                "no-new-privileges:true|false\n",
                "",
                fail_at=2,
            )

    def test_rejects_invalid_id_before_docker(self):
        with patch.object(registration.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                registration.inspect_container_security("invalid")
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
