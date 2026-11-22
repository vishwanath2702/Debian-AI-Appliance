import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASE = ROOT / "payload/patches/open-webui/v0.13.0"
MODULE = BASE / "patched-middleware.py"
MANIFEST = BASE / "builtin-tools-opt-in.json"
VERIFIER = ROOT / "build/verify-openwebui-middleware.py"

spec = importlib.util.spec_from_file_location(
    "openwebui_middleware_verifier", VERIFIER
)
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)

class MiddlewareStagingTests(unittest.TestCase):
    def test_verified_module(self):
        self.assertTrue(verifier.verify(MODULE, MANIFEST))

    def test_tampered_module_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            altered = Path(tmp) / "middleware.py"
            altered.write_bytes(MODULE.read_bytes() + b"\\n# changed\\n")
            with self.assertRaises(ValueError):
                verifier.verify(altered, MANIFEST)

    def test_tampered_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            altered = Path(tmp) / "manifest.json"
            data = json.loads(MANIFEST.read_text())
            data["patch_sha256"] = "0" * 64
            altered.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                verifier.verify(MODULE, altered)

    def test_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            link = Path(tmp) / "middleware.py"
            link.symlink_to(MODULE)
            with self.assertRaises(OSError):
                verifier.verify(link, MANIFEST)

    def test_opt_in_default(self):
        source = MODULE.read_text()
        self.assertIn(".get('builtin_tools', False)", source)
        self.assertIn(
            "use_builtin_tools = is_note_chat or (", source
        )
        self.assertIn("if payload_tools is None:", source)

    def test_staging_preserves_retrieval(self):
        builder = (ROOT / "build/build-payload.sh").read_text()
        self.assertIn("verify-openwebui-middleware.py", builder)
        self.assertIn("daia-openwebui-middleware.py", builder)
        self.assertIn("daia-openwebui-retrieval-utils.py", builder)

if __name__ == "__main__":
    unittest.main()
