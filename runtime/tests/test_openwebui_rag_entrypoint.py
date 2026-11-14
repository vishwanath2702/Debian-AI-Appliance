"""Open WebUI pgvector startup wrapper tests."""

import importlib.util
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WRAPPER = ROOT / "runtime/daia-openwebui-rag-entrypoint.py"

spec = importlib.util.spec_from_file_location(
    "daia_openwebui_rag_entrypoint", WRAPPER
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class OpenWebUIRAGEntrypointTests(unittest.TestCase):

    def test_password_url_encoding(self):
        url = module.build_pgvector_url("a:b@c/d")
        self.assertIn("a%3Ab%40c%2Fd", url)
        self.assertIn("@daia-pgvector:5432/daia_rag", url)

    def test_reads_secret_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "password"
            path.write_bytes(b"secret-value")
            self.assertEqual(
                module.read_password(path),
                "secret-value",
            )

    def test_rejects_empty_secret(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "password"
            path.write_bytes(b"")
            with self.assertRaises(ValueError):
                module.read_password(path)

    def test_rejects_newline(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "password"
            path.write_bytes(b"invalid\\nsecret")
            with self.assertRaises(ValueError):
                module.read_password(path)

    def test_original_startup_command_preserved(self):
        source = WRAPPER.read_text()
        self.assertIn(
            'os.execvp("bash", ["bash", "start.sh"])',
            source,
        )


if __name__ == "__main__":
    unittest.main()
