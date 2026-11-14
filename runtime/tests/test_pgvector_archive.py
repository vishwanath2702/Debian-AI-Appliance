"""Tests for offline pgvector archive trust boundary (no Docker)."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

PATH = Path(__file__).resolve().parents[1] / 'pgvector_archive.py'
spec = importlib.util.spec_from_file_location('pgvector_archive', PATH)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

class ArchiveTests(unittest.TestCase):
    def test_requires_root(self):
        with patch.object(mod.os, 'geteuid', return_value=1000):
            with self.assertRaises(PermissionError):
                mod.verified_archive('/opt/daia/payload/images/pgvector.tar')

    def test_rejects_bad_paths(self):
        with patch.object(mod.os, 'geteuid', return_value=0):
            for path in ('relative', '/', '/tmp/../x', '/tmp//x', '/tmp/./x'):
                with self.subTest(path=path), self.assertRaises(ValueError):
                    mod.verified_archive(path)

    @unittest.skipUnless(os.geteuid() == 0, 'root ownership required')
    def test_accepts_private_root_owned_archive(self):
        import hashlib
        with tempfile.TemporaryDirectory(dir='/root') as directory:
            archive = Path(directory) / 'image.tar'
            archive.write_bytes(b'valid-test-archive')
            os.chmod(archive, 0o600)
            with mod.verified_archive(str(archive), hashlib.sha256(archive.read_bytes()).hexdigest()) as stream:
                self.assertEqual(stream.read(), b'valid-test-archive')

    @unittest.skipUnless(os.geteuid() == 0, 'root ownership required')
    def test_rejects_symlink_and_writable_parent(self):
        import hashlib
        with tempfile.TemporaryDirectory(dir='/root') as directory:
            root = Path(directory)
            archive = root / 'image.tar'
            archive.write_bytes(b'bytes')
            os.chmod(archive, 0o600)
            (root/'linked.tar').symlink_to(archive)
            digest = hashlib.sha256(b'bytes').hexdigest()
            with self.assertRaises(OSError):
                mod.verified_archive(str(root/'linked.tar'), digest)
            os.chmod(root, 0o777)
            with self.assertRaises(PermissionError):
                mod.verified_archive(str(archive), digest)

    @unittest.skipUnless(os.geteuid() == 0, 'root ownership required')
    def test_rejects_checksum_mismatch(self):
        with tempfile.TemporaryDirectory(dir='/root') as directory:
            archive = Path(directory) / 'image.tar'
            archive.write_bytes(b'wrong')
            os.chmod(archive, 0o600)
            with self.assertRaises(ValueError):
                mod.verified_archive(str(archive))

if __name__ == '__main__':
    unittest.main()
