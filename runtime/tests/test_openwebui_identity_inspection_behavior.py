"""Isolated behavior tests for Open WebUI Docker identity inspection."""

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
IMAGE_ID = (
    "sha256:e29d4d2457b54bb14434bee2666420e822cb0e9389466f02405d69fa7fd8d06c"
)

loader = importlib.machinery.SourceFileLoader(
    "daia_openwebui_identity_inspection",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)
registration = importlib.util.module_from_spec(spec)
loader.exec_module(registration)


class OpenWebUIIdentityInspectionTests(unittest.TestCase):

    def inspect(self, response, *, returncode=0):
        def fake_run(args, **kwargs):
            self.assertEqual(args[:3], [
                "docker", "container", "inspect"
            ])
            self.assertEqual(args[-1], CONTAINER_ID)
            self.assertEqual(kwargs["check"], True)
            self.assertEqual(kwargs["timeout"], 15)

            if returncode:
                raise subprocess.CalledProcessError(
                    returncode, args
                )

            return subprocess.CompletedProcess(
                args, 0, stdout=response, stderr=""
            )

        with patch.object(
            registration.subprocess, "run",
            side_effect=fake_run,
        ) as run:
            if returncode:
                with self.assertRaises(
                    subprocess.CalledProcessError
                ):
                    registration.inspect_container_identity(
                        CONTAINER_ID
                    )
                self.assertEqual(run.call_count, 1)
                return

            return registration.inspect_container_identity(
                CONTAINER_ID
            )

    def valid_response(self):
        return (
            CONTAINER_ID
            + "\topenwebui\ttrue\t"
            + IMAGE_ID
            + "\n"
        )

    def test_accepts_exact_verified_identity(self):
        self.assertEqual(
            self.inspect(self.valid_response()),
            CONTAINER_ID,
        )

    def test_rejects_changed_container_identity(self):
        response = self.valid_response().replace(
            CONTAINER_ID, "b" * 64
        )
        with self.assertRaises(PermissionError):
            self.inspect(response)

    def test_rejects_wrong_image(self):
        response = self.valid_response().replace(
            IMAGE_ID, "sha256:" + "b" * 64
        )
        with self.assertRaises(PermissionError):
            self.inspect(response)

    def test_rejects_wrong_component_label(self):
        response = self.valid_response().replace(
            "\topenwebui\t", "\tother\t"
        )
        with self.assertRaises(PermissionError):
            self.inspect(response)

    def test_rejects_unmanaged_container(self):
        response = self.valid_response().replace(
            "\ttrue\t", "\tfalse\t"
        )
        with self.assertRaises(PermissionError):
            self.inspect(response)

    def test_rejects_missing_ownership_label(self):
        response = self.valid_response().replace(
            "\topenwebui\t", "\t\t"
        )
        with self.assertRaises(PermissionError):
            self.inspect(response)

    def test_rejects_extra_metadata(self):
        response = self.valid_response().replace(
            IMAGE_ID, IMAGE_ID + "\textra"
        )
        with self.assertRaises(PermissionError):
            self.inspect(response)

    def test_rejects_multiple_inspection_lines(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                self.valid_response()
                + self.valid_response()
            )

    def test_rejects_docker_inspection_failure(self):
        self.inspect("", returncode=1)

    def test_rejects_invalid_id_before_docker(self):
        with patch.object(
            registration.subprocess, "run"
        ) as run:
            with self.assertRaises(ValueError):
                registration.inspect_container_identity(
                    "invalid"
                )
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
