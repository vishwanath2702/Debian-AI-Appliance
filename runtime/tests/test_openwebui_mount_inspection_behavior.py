"""Mock-Docker tests for exact Open WebUI mount verification."""

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
    "daia_openwebui_mount_inspection",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)
registration = importlib.util.module_from_spec(spec)
loader.exec_module(registration)

DATA = (
    "volume|open-webui|"
    "/var/lib/docker/volumes/open-webui/_data|"
    "/app/backend/data|true"
)
SECRET = (
    "bind||/var/lib/daia/pgvector/openwebui-password|"
    "/run/secrets/daia-openwebui-pgvector-password|false"
)
ENTRYPOINT = (
    "bind||/usr/libexec/daia-openwebui-rag-entrypoint.py|"
    "/usr/local/bin/daia-openwebui-rag-entrypoint.py|false"
)


class OpenWebUIMountInspectionTests(unittest.TestCase):

    def inspect(self, lines, *, fail=False, mountpoint=None,
                volume_fail=False):
        output = "\n".join(lines) + "\n"
        if mountpoint is None:
            mountpoint = "/var/lib/docker/volumes/open-webui/_data"

        def fake_run(args, **kwargs):
            self.assertTrue(kwargs["check"])
            self.assertEqual(kwargs["timeout"], 15)

            if args[:3] == ["docker", "container", "inspect"]:
                self.assertEqual(args[-1], CONTAINER_ID)
                if fail:
                    raise subprocess.CalledProcessError(1, args)
                return subprocess.CompletedProcess(
                    args, 0, stdout=output, stderr=""
                )

            if args[:3] == ["docker", "volume", "inspect"]:
                self.assertEqual(args[-1], "open-webui")
                if volume_fail:
                    raise subprocess.CalledProcessError(1, args)
                return subprocess.CompletedProcess(
                    args, 0,
                    stdout=mountpoint + "\n",
                    stderr="",
                )

            self.fail(f"Unexpected Docker command: {args!r}")

        with patch.object(
            registration.subprocess,
            "run",
            side_effect=fake_run,
        ) as run:
            registration.inspect_container_mounts(CONTAINER_ID)
            self.assertEqual(run.call_count, 2)

    def test_accepts_exact_three_mounts(self):
        self.inspect([DATA, SECRET, ENTRYPOINT])

    def test_accepts_different_mount_order(self):
        self.inspect([ENTRYPOINT, DATA, SECRET])

    def test_rejects_missing_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([DATA, SECRET])

    def test_rejects_extra_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET, ENTRYPOINT,
                "bind||/tmp/extra|/tmp/extra|false",
            ])

    def test_rejects_duplicate_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([DATA, SECRET, SECRET])

    def test_rejects_writable_secret(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA,
                SECRET.replace("|false", "|true"),
                ENTRYPOINT,
            ])

    def test_rejects_writable_entrypoint(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET,
                ENTRYPOINT.replace("|false", "|true"),
            ])

    def test_rejects_readonly_data_volume(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA.replace("|true", "|false"),
                SECRET, ENTRYPOINT,
            ])

    def test_rejects_wrong_volume_name(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA.replace("volume|open-webui|", "volume|other|"),
                SECRET, ENTRYPOINT,
            ])

    def test_rejects_wrong_volume_source(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA.replace(
                    "/var/lib/docker/volumes/open-webui/_data",
                    "/tmp/untrusted",
                ),
                SECRET, ENTRYPOINT,
            ])

    def test_rejects_wrong_secret_source(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA,
                SECRET.replace(
                    "/var/lib/daia/pgvector/openwebui-password",
                    "/tmp/password",
                ),
                ENTRYPOINT,
            ])

    def test_rejects_wrong_entrypoint_source(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET,
                ENTRYPOINT.replace(
                    "/usr/libexec/daia-openwebui-rag-entrypoint.py",
                    "/tmp/entrypoint.py",
                ),
            ])

    def test_rejects_malformed_data_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA + "|extra", SECRET, ENTRYPOINT,
            ])

    def test_rejects_docker_inspection_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inspect([DATA, SECRET, ENTRYPOINT], fail=True)

    def test_accepts_nondefault_docker_storage_root(self):
        alternate = "/srv/daia-docker/volumes/open-webui/_data"
        self.inspect(
            [DATA.replace(
                "/var/lib/docker/volumes/open-webui/_data",
                alternate,
            ), SECRET, ENTRYPOINT],
            mountpoint=alternate,
        )

    def test_rejects_mountpoint_mismatch(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                [DATA, SECRET, ENTRYPOINT],
                mountpoint="/srv/untrusted/open-webui",
            )

    def test_rejects_volume_inspection_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inspect(
                [DATA, SECRET, ENTRYPOINT],
                volume_fail=True,
            )

    def test_rejects_root_volume_mountpoint(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                [DATA.replace(
                    "/var/lib/docker/volumes/open-webui/_data",
                    "/",
                ), SECRET, ENTRYPOINT],
                mountpoint="/",
            )

    def test_rejects_invalid_id_before_docker(self):
        with patch.object(registration.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                registration.inspect_container_mounts("invalid")
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
