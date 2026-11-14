"""Isolated tests for Open WebUI credential file publication."""

import importlib.util
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
sys.path.insert(0, str(RUNTIME))

import pgvector_credentials

HELPER = RUNTIME / "daia-openwebui-pgvector-credentials.py"
spec = importlib.util.spec_from_file_location("openwebui_credentials", HELPER)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CredentialWriteTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.directory.chmod(0o700)

        real_fstat = os.fstat

        def trusted_directory(_path):
            return os.open(
                self.directory,
                os.O_RDONLY | os.O_DIRECTORY |
                os.O_NOFOLLOW | os.O_CLOEXEC,
            )

        def trusted_metadata(fd):
            metadata = real_fstat(fd)
            # Simulate only the ownership reported for our trusted
            # directory descriptor; preserve real file operations.
            if stat.S_ISDIR(metadata.st_mode):
                fields = list(metadata)
                fields[4] = 0
                fields[5] = 0
                return os.stat_result(fields)
            return metadata

        validator = patch.object(
            module,
            "open_existing_state_directory",
            side_effect=trusted_directory,
        )
        metadata = patch.object(
            module.os,
            "fstat",
            side_effect=trusted_metadata,
        )
        validator.start()
        metadata.start()
        self.addCleanup(metadata.stop)
        self.addCleanup(validator.stop)

    def test_creates_private_secret(self):
        destination = module.provision(self.directory)
        self.assertEqual(destination.name, "openwebui-password")
        self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)
        self.assertGreater(len(destination.read_bytes()), 40)

    def test_refuses_overwrite(self):
        destination = module.provision(self.directory)
        original = destination.read_bytes()
        with self.assertRaises(FileExistsError):
            module.provision(self.directory)
        self.assertEqual(destination.read_bytes(), original)

    def test_removes_partial_secret_on_write_failure(self):
        real_fdopen = os.fdopen

        class FailingWriter:
            def __init__(self, stream):
                self.stream = stream

            def __enter__(self):
                self.stream.__enter__()
                return self

            def __exit__(self, *args):
                return self.stream.__exit__(*args)

            def write(self, data):
                self.stream.write(data[:4])
                raise OSError("simulated partial write")

            def flush(self):
                return self.stream.flush()

        def failing_fdopen(fd, *args, **kwargs):
            return FailingWriter(real_fdopen(fd, *args, **kwargs))

        with patch.object(module.os, "fdopen", side_effect=failing_fdopen):
            with self.assertRaisesRegex(OSError, "simulated partial write"):
                module.provision(self.directory)

        self.assertFalse(
            (self.directory / "openwebui-password").exists()
        )


if __name__ == "__main__":
    unittest.main()
