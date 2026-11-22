"""Regression tests for pinned Open WebUI BM25 changes."""
import ast
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
PATCH = REPO / "payload/patches/open-webui/v0.13.0/retrieval-utils.patch"
SOURCE = Path("/tmp/daia-openwebui-rag-1011/app/backend/open_webui/retrieval/utils.py")

class BM25PatchTests(unittest.TestCase):
    def test_patch_structure(self):
        content = PATCH.read_text()
        self.assertIn("def daia_bm25_tokens(", content)
        self.assertIn("def daia_bm25_query_tokens(", content)
        self.assertIn("daia_bm25_query_tokens(query)", content)
        self.assertIn("daia_bm25_tokens(text)", content)

    def test_patch_no_docker(self):
        self.assertNotIn("docker ", PATCH.read_text())

    def test_patched_source_behavior(self):
        if not SOURCE.is_file():
            self.skipTest("Pinned extracted source unavailable")
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            target = root / "app/backend/open_webui/retrieval/utils.py"
            target.parent.mkdir(parents=True)
            target.write_bytes(SOURCE.read_bytes())
            subprocess.run(
                ["git", "apply", str(PATCH)],
                cwd=root, check=True, capture_output=True, text=True
            )
            code = target.read_text()
            tree = ast.parse(code)
            nodes = [
                n for n in tree.body
                if isinstance(n, ast.FunctionDef)
                and n.name in (
                    "daia_bm25_tokens", "daia_bm25_query_tokens"
                )
            ]
            self.assertEqual(len(nodes), 2)
            ns = {}
            exec(compile(
                ast.Module(body=nodes, type_ignores=[]),
                "<bm25-test>", "exec"
            ), ns)
            focus = ns["daia_bm25_query_tokens"]
            self.assertEqual(
                focus("Find the exact word lspci in Debian Reference"),
                ["lspci"]
            )
            self.assertEqual(
                focus("What does lspci do?"),
                ["what", "does", "lspci", "do"]
            )
            self.assertEqual(
                ns["daia_bm25_tokens"]("CUDA_VISIBLE_DEVICES"),
                ["cuda_visible_devices"]
            )
            self.assertIn("top_n=k_reranker", code)
            self.assertIn("r_score=r", code)

if __name__ == "__main__":
    unittest.main()
