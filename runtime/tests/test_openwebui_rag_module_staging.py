"""Pinned Open WebUI RAG payload verification tests."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "build/verify-openwebui-rag-module.py"
MODULE = ROOT / "payload/patches/open-webui/v0.13.0/patched-utils.py"
MANIFEST = ROOT / "payload/patches/open-webui/v0.13.0/patched-utils.json"
BUILD = ROOT / "build/build-payload.sh"


class RAGModuleStagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("rag_module_verifier", HELPER)
        cls.verifier = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.verifier)

    def test_pinned_module(self):
        self.assertTrue(self.verifier.verify(MODULE, MANIFEST))

    def test_tampered_module_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "utils.py"
            target.write_bytes(MODULE.read_bytes() + b"\\n# tampered\\n")
            with self.assertRaises(ValueError):
                self.verifier.verify(target, MANIFEST)

    def test_tampered_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "manifest.json"
            metadata = json.loads(MANIFEST.read_text())
            metadata["patch_sha256"] = "0" * 64
            target.write_text(json.dumps(metadata))
            with self.assertRaises(ValueError):
                self.verifier.verify(MODULE, target)

    def test_symlink_module_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "utils.py"
            target.symlink_to(MODULE)
            with self.assertRaises(OSError):
                self.verifier.verify(target, MANIFEST)

    def test_build_stages_verified_module(self):
        source = BUILD.read_text()
        self.assertIn("verify-openwebui-rag-module.py", source)
        self.assertIn("daia-openwebui-retrieval-utils.py", source)
        self.assertIn("0644 || return 1", source)


if __name__ == "__main__":
    unittest.main()
