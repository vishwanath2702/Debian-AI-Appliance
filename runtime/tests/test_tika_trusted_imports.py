import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
LOADER = ROOT / "openwebui_trusted_imports.py"
TIKA = ROOT / "daia-tika-register-container"

def load_loader():
    spec = importlib.util.spec_from_file_location(
        "tika_trust_fixture", LOADER
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

class TikaTrustedImportsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(
            prefix="pragna-tika-trust-"
        )
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.uid = os.getuid()

        sources = {
            "pgvector_credentials.py": "VALUE = 'trusted'\n",
            "pgvector_registration.py":
                "from pgvector_credentials import VALUE\n",
            "openwebui_registration.py":
                "from pgvector_registration import VALUE\n",
            "tika_registration.py":
                "from pgvector_registration import VALUE\n",
        }
        for name, source in sources.items():
            path = self.directory / name
            path.write_text(source, encoding="utf-8")
            path.chmod(0o644)

    def test_existing_default_remains_three_modules(self):
        with mock.patch.dict(sys.modules, {}):
            result = load_loader().load_trusted_runtime_modules(
                self.directory, expected_uid=self.uid
            )
        self.assertEqual(set(result), {
            "pgvector_credentials",
            "pgvector_registration",
            "openwebui_registration",
        })

    def test_explicit_tika_module_is_trusted(self):
        with mock.patch.dict(sys.modules, {}):
            result = load_loader().load_trusted_runtime_modules(
                self.directory,
                expected_uid=self.uid,
                extra_modules=("tika_registration",),
            )
        self.assertEqual(
            result["tika_registration"].VALUE, "trusted"
        )

    def test_unapproved_module_rejected(self):
        with self.assertRaises(ValueError):
            load_loader().load_trusted_runtime_modules(
                self.directory,
                expected_uid=self.uid,
                extra_modules=("unapproved",),
            )

    def test_symlinked_tika_module_rejected(self):
        path = self.directory / "tika_registration.py"
        path.unlink()
        path.symlink_to(
            self.directory / "pgvector_registration.py"
        )
        with self.assertRaises(OSError):
            load_loader().load_trusted_runtime_modules(
                self.directory,
                expected_uid=self.uid,
                extra_modules=("tika_registration",),
            )

    def test_privileged_entrypoint_has_no_path_insertion(self):
        source = TIKA.read_text(encoding="utf-8")
        self.assertNotIn("sys.path.insert", source)
        self.assertIn(
            'extra_modules=("tika_registration",)', source
        )
        self.assertIn("_bind_trusted_dependencies()", source)

if __name__ == "__main__":
    unittest.main()
