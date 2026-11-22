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


RETRIEVAL = (
    "bind||/usr/libexec/daia-openwebui-retrieval-utils.py|"
    "/app/backend/open_webui/retrieval/utils.py|false"
)


MIDDLEWARE = (
    "bind||/usr/libexec/daia-openwebui-middleware.py|"
    "/app/backend/open_webui/utils/middleware.py|false"
)


class OpenWebUIMountInspectionTests(unittest.TestCase):

    def inspect(self, lines, *, fail=False, mountpoint=None,
                volume_fail=False, missing_retrieval=False,
                 missing_middleware=False):
        lines = list(lines)
        if not missing_middleware:
            lines.append(MIDDLEWARE)
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
            import unittest.mock as integrity_mock
            with integrity_mock.patch.object(
                registration,
                "verify_retrieval_module_integrity",
                return_value=True,
            ) as integrity_check:
                with integrity_mock.patch.object(
                    registration,
                    "verify_middleware_module_integrity",
                    return_value=True,
                ) as middleware_check:
                    registration.inspect_container_mounts(CONTAINER_ID)
                    middleware_check.assert_called_once_with()
                integrity_check.assert_called_once_with()
            self.assertEqual(run.call_count, 2)

    def test_accepts_exact_five_mounts(self):
        self.inspect([DATA, SECRET, ENTRYPOINT, RETRIEVAL])

    def test_rejects_missing_middleware(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                [DATA, SECRET, ENTRYPOINT, RETRIEVAL],
                missing_middleware=True,
            )

    def test_rejects_writable_middleware(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET, ENTRYPOINT, RETRIEVAL,
                MIDDLEWARE.replace("|false", "|true"),
            ], missing_middleware=True)

    def test_rejects_wrong_middleware_destination(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET, ENTRYPOINT, RETRIEVAL,
                MIDDLEWARE.replace(
                    "/open_webui/utils/middleware.py",
                    "/open_webui/utils/other.py",
                ),
            ], missing_middleware=True)

    def test_rejects_duplicate_middleware(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET, ENTRYPOINT, RETRIEVAL,
                MIDDLEWARE, MIDDLEWARE,
            ], missing_middleware=True)

    def test_accepts_different_mount_order(self):
        self.inspect([ENTRYPOINT, DATA, SECRET, RETRIEVAL])

    def test_rejects_missing_retrieval_module(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                [DATA, SECRET, ENTRYPOINT],
                missing_retrieval=True,
            )

    def test_rejects_writable_retrieval_module(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET, ENTRYPOINT,
                RETRIEVAL.replace("|false", "|true"),
            ])

    def test_rejects_wrong_retrieval_destination(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET, ENTRYPOINT,
                RETRIEVAL.replace(
                    "/open_webui/retrieval/utils.py",
                    "/open_webui/retrieval/other.py",
                ),
            ])

    def test_rejects_missing_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([DATA, SECRET, RETRIEVAL])

    def test_rejects_extra_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET, ENTRYPOINT,
                "bind||/tmp/extra|/tmp/extra|false",
             RETRIEVAL])

    def test_rejects_duplicate_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([DATA, SECRET, SECRET, RETRIEVAL])

    def test_rejects_writable_secret(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA,
                SECRET.replace("|false", "|true"),
                ENTRYPOINT,
             RETRIEVAL])

    def test_rejects_writable_entrypoint(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET,
                ENTRYPOINT.replace("|false", "|true"),
             RETRIEVAL])

    def test_rejects_readonly_data_volume(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA.replace("|true", "|false"),
                SECRET, ENTRYPOINT,
             RETRIEVAL])

    def test_rejects_wrong_volume_name(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA.replace("volume|open-webui|", "volume|other|"),
                SECRET, ENTRYPOINT,
             RETRIEVAL])

    def test_rejects_wrong_volume_source(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA.replace(
                    "/var/lib/docker/volumes/open-webui/_data",
                    "/tmp/untrusted",
                ),
                SECRET, ENTRYPOINT,
             RETRIEVAL])

    def test_rejects_wrong_secret_source(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA,
                SECRET.replace(
                    "/var/lib/daia/pgvector/openwebui-password",
                    "/tmp/password",
                ),
                ENTRYPOINT,
             RETRIEVAL])

    def test_rejects_wrong_entrypoint_source(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA, SECRET,
                ENTRYPOINT.replace(
                    "/usr/libexec/daia-openwebui-rag-entrypoint.py",
                    "/tmp/entrypoint.py",
                ),
             RETRIEVAL])

    def test_rejects_malformed_data_mount(self):
        with self.assertRaises(PermissionError):
            self.inspect([
                DATA + "|extra", SECRET, ENTRYPOINT,
             RETRIEVAL])

    def test_rejects_docker_inspection_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inspect([DATA, SECRET, ENTRYPOINT, RETRIEVAL], fail=True)

    def test_accepts_nondefault_docker_storage_root(self):
        alternate = "/srv/daia-docker/volumes/open-webui/_data"
        self.inspect(
            [DATA.replace(
                "/var/lib/docker/volumes/open-webui/_data",
                alternate,
            ), SECRET, ENTRYPOINT, RETRIEVAL],
            mountpoint=alternate,
        )

    def test_rejects_mountpoint_mismatch(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                [DATA, SECRET, ENTRYPOINT, RETRIEVAL],
                mountpoint="/srv/untrusted/open-webui",
            )

    def test_rejects_volume_inspection_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inspect(
                [DATA, SECRET, ENTRYPOINT, RETRIEVAL],
                volume_fail=True,
            )

    def test_rejects_root_volume_mountpoint(self):
        with self.assertRaises(PermissionError):
            self.inspect(
                [DATA.replace(
                    "/var/lib/docker/volumes/open-webui/_data",
                    "/",
                ), SECRET, ENTRYPOINT, RETRIEVAL],
                mountpoint="/",
            )

    def test_rejects_invalid_id_before_docker(self):
        with patch.object(registration.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                registration.inspect_container_mounts("invalid")
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
