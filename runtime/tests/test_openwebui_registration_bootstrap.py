import ast
import os
import stat
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-openwebui-register-container"

LOADER_SOURCE = """
from types import SimpleNamespace

def load_trusted_runtime_modules(directory, *, expected_uid):
    if directory != "/opt/daia" or expected_uid != 0:
        raise PermissionError("Incorrect trusted loader contract")

    return {
        "pgvector_registration": SimpleNamespace(
            read_trusted_registration=lambda path: "a" * 64,
            validate_container_id=lambda value: value,
        ),
        "openwebui_registration": SimpleNamespace(
            provision_registration_directory=lambda: None,
            register_authorized_container=lambda *args: None,
        ),
    }
"""


class BootstrapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        tree = ast.parse(HELPER.read_text(encoding="utf-8"))
        functions = [
            node for node in tree.body
            if isinstance(node, ast.FunctionDef)
            and node.name == "_bind_trusted_dependencies"
        ]
        if len(functions) != 1:
            raise AssertionError("Expected one bootstrap function")

        isolated = ast.Module(body=functions, type_ignores=[])
        cls.bootstrap_code = compile(isolated, str(HELPER), "exec")

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(
            prefix="pragna-bootstrap-"
        )
        self.addCleanup(self.temporary.cleanup)

        self.fake_root = Path(self.temporary.name) / "root"
        self.libexec = self.fake_root / "usr/libexec"
        self.libexec.mkdir(parents=True)

        # The simulated trusted installation must not depend on umask.
        for directory in (
            self.fake_root,
            self.fake_root / "usr",
            self.libexec,
        ):
            directory.chmod(0o755)

        self.loader = self.libexec / "openwebui_trusted_imports.py"
        self.loader.write_text(LOADER_SOURCE, encoding="utf-8")
        self.loader.chmod(0o644)

    def invoke(self):
        namespace = {"os": os, "stat": stat}
        exec(self.bootstrap_code, namespace)

        original_open = os.open
        original_fstat = os.fstat

        def redirected_open(path, flags, mode=0o777, *, dir_fd=None):
            if path == "/" and dir_fd is None:
                path = str(self.fake_root)
            return original_open(path, flags, mode, dir_fd=dir_fd)

        def simulated_root_metadata(fd):
            metadata = original_fstat(fd)
            return types.SimpleNamespace(
                st_mode=metadata.st_mode,
                st_uid=0,
                st_nlink=metadata.st_nlink,
                st_size=metadata.st_size,
                st_dev=metadata.st_dev,
                st_ino=metadata.st_ino,
                st_mtime_ns=metadata.st_mtime_ns,
                st_ctime_ns=metadata.st_ctime_ns,
            )

        with (
            patch.object(os, "open", side_effect=redirected_open),
            patch.object(os, "fstat", side_effect=simulated_root_metadata),
        ):
            namespace["_bind_trusted_dependencies"]()

        return namespace

    def test_accepts_verified_loader_and_binds_dependencies(self):
        namespace = self.invoke()
        self.assertTrue(callable(namespace["validate_container_id"]))
        self.assertTrue(callable(namespace["read_trusted_registration"]))
        self.assertTrue(callable(namespace["register_authorized_container"]))

    def test_rejects_symlinked_loader(self):
        self.loader.unlink()
        target = self.libexec / "target.py"
        target.write_text(LOADER_SOURCE, encoding="utf-8")
        self.loader.symlink_to(target.name)

        with self.assertRaises(OSError):
            self.invoke()

    def test_rejects_group_writable_loader(self):
        self.loader.chmod(0o664)
        with self.assertRaises(PermissionError):
            self.invoke()

    def test_rejects_group_writable_loader_directory(self):
        self.libexec.chmod(0o775)
        with self.assertRaises(PermissionError):
            self.invoke()

    def test_rejects_hardlinked_loader(self):
        os.link(self.loader, self.libexec / "duplicate.py")
        with self.assertRaises(PermissionError):
            self.invoke()

    def test_rejects_missing_loader(self):
        self.loader.unlink()
        with self.assertRaises(OSError):
            self.invoke()


if __name__ == "__main__":
    unittest.main()
