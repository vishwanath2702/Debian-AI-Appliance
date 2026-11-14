import importlib.util
import os
from pathlib import Path
import unittest


SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-pgvector-credentials.py"
)
spec = importlib.util.spec_from_file_location(
    "pragna_openwebui_credentials_boundary", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CredentialLoaderBoundaryTests(unittest.TestCase):
    def test_no_runtime_path_import(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertNotIn("sys.path.insert", source)
        self.assertNotIn("from pgvector_credentials import", source)

    def test_rejects_relative_directory(self):
        with self.assertRaises(ValueError):
            module.open_existing_state_directory("var/lib/daia")

    def test_rejects_dot_and_parent_components(self):
        for path in (
            "/var/./lib/daia",
            "/var/../lib/daia",
        ):
            with self.subTest(path=path):
                with self.assertRaises(ValueError):
                    module.open_existing_state_directory(path)

    def test_rejects_ambiguous_separators(self):
        for path in (
            "/var//lib/daia",
            "/var/lib/daia/",
            "/",
        ):
            with self.subTest(path=path):
                with self.assertRaises(ValueError):
                    module.open_existing_state_directory(path)

    def test_rejects_writable_parent(self):
        with self.assertRaises(PermissionError):
            module.open_existing_state_directory("/tmp/daia-test")

    def test_rejects_symlink_component(self):
        with self.assertRaises(OSError):
            module.open_existing_state_directory("/proc/self")


if __name__ == "__main__":
    unittest.main()
