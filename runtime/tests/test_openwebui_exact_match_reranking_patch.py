"""Regression tests for exact-identifier evidence preservation."""
import ast
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path("/tmp/daia-openwebui-rag-1011/app/backend/open_webui/retrieval/utils.py")
PATCH = ROOT / "payload/patches/open-webui/v0.13.0/retrieval-utils.patch"
REL = Path("app/backend/open_webui/retrieval/utils.py")


class ExactMatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not SOURCE.is_file():
            raise AssertionError("Pinned source unavailable")
        with tempfile.TemporaryDirectory() as td:
            target = Path(td) / REL
            target.parent.mkdir(parents=True)
            target.write_bytes(SOURCE.read_bytes())
            subprocess.run(["git", "apply", str(PATCH)], cwd=td,
                           check=True, capture_output=True)
            code = target.read_text()
        tree = ast.parse(code)
        functions = [
            n for n in tree.body
            if isinstance(n, ast.FunctionDef)
            and n.name in ("daia_bm25_tokens",
                           "daia_prioritize_exact_matches")
        ]
        assert len(functions) == 2
        cls.ns = {"CHUNK_HASH_KEY": "_chunk_hash"}
        exec(compile(ast.Module(body=functions, type_ignores=[]),
                     "<rerank>", "exec"), cls.ns)
        cls.code = code

    def test_exact_passage_survives(self):
        docs = [
            (SimpleNamespace(metadata={"_chunk_hash": str(i)}), 1-i*.05)
            for i in range(10)
        ]
        originals = {str(i): "unrelated text" for i in range(10)}
        originals["9"] = "The lspci(8) command identifies PCI hardware."
        result = self.ns["daia_prioritize_exact_matches"](
            docs, "Find the exact word lspci in Debian Reference", originals
        )
        self.assertEqual(result[0], docs[9])
        self.assertEqual(len(result[:5]), 5)

    def test_ordinary_query_unchanged(self):
        docs = [
            (SimpleNamespace(metadata={"_chunk_hash": str(i)}), 1-i*.1)
            for i in range(3)
        ]
        result = self.ns["daia_prioritize_exact_matches"](
            docs, "How can I inspect hardware?", {"2": "lspci"}
        )
        self.assertEqual(result, docs)

    def test_filename_only_not_evidence(self):
        docs = [
            (SimpleNamespace(metadata={"_chunk_hash": "a"}), .9),
            (SimpleNamespace(metadata={"_chunk_hash": "b"}), .5),
        ]
        result = self.ns["daia_prioritize_exact_matches"](
            docs, "Find the exact word lspci",
            {"a": "ordinary text", "b": "ordinary text"}
        )
        self.assertEqual(result, docs)

    def test_identifier_token_boundaries(self):
        tokens = self.ns["daia_bm25_tokens"]
        self.assertIn("lspci", tokens("lspci(8)"))
        self.assertNotIn("lspci", tokens("mylspciwrapper"))

    def test_threshold_and_top_k_preserved(self):
        self.assertIn("if self.r_score:", self.code)
        self.assertIn("if s >= self.r_score", self.code)
        self.assertIn("result[: self.top_n]", self.code)
        self.assertIn("top_n=k_reranker", self.code)
        self.assertIn("r_score=r", self.code)
        self.assertIn("id_key=CHUNK_HASH_KEY", self.code)


if __name__ == "__main__":
    unittest.main()
