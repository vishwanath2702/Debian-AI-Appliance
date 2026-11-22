"""Regression tests for quotation passage-quality ranking."""
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


class PassageQualityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not SOURCE.is_file():
            raise AssertionError("Pinned source required")
        with tempfile.TemporaryDirectory() as td:
            target = Path(td) / REL
            target.parent.mkdir(parents=True)
            target.write_bytes(SOURCE.read_bytes())
            subprocess.run(
                ["git", "apply", str(PATCH)],
                cwd=td, check=True, capture_output=True
            )
            code = target.read_text()
        tree = ast.parse(code)
        nodes = [
            n for n in tree.body
            if isinstance(n, ast.FunctionDef)
            and n.name in (
                "daia_bm25_tokens",
                "daia_prioritize_exact_matches",
            )
        ]
        assert len(nodes) == 2
        cls.ns = {"CHUNK_HASH_KEY": "_chunk_hash"}
        exec(compile(ast.Module(body=nodes, type_ignores=[]),
                     "<passage-quality>", "exec"), cls.ns)

    def docs(self):
        return [
            (SimpleNamespace(metadata={"_chunk_hash": "link"}), .9),
            (SimpleNamespace(metadata={"_chunk_hash": "sentence"}), .5),
            (SimpleNamespace(metadata={"_chunk_hash": "other"}), .4),
        ]

    def originals(self):
        return {
            "link": "lspci(8) https://example.invalid/lspci",
            "sentence": (
                "For PCI-like devices such as PCI-Express and CardBus, "
                "lspci(8) is a good start for hardware identification."
            ),
            "other": "This section discusses hardware.",
        }

    def test_sentence_beats_reference(self):
        result = self.ns["daia_prioritize_exact_matches"](
            self.docs(),
            "Find the exact word lspci and quote the sentence",
            self.originals(),
        )
        self.assertEqual(
            result[0][0].metadata["_chunk_hash"], "sentence"
        )

    def test_non_quote_preserves_similarity_order(self):
        result = self.ns["daia_prioritize_exact_matches"](
            self.docs(), "Find the exact word lspci", self.originals()
        )
        self.assertEqual(
            result[0][0].metadata["_chunk_hash"], "link"
        )

    def test_ordinary_query_preserves_order(self):
        docs = self.docs()
        result = self.ns["daia_prioritize_exact_matches"](
            docs, "How can I identify PCI hardware?", self.originals()
        )
        self.assertEqual(result, docs)

    def test_result_count_and_scores_unchanged(self):
        docs = self.docs()
        result = self.ns["daia_prioritize_exact_matches"](
            docs, "Find the exact word lspci; quote the sentence",
            self.originals()
        )
        self.assertEqual(len(result), len(docs))
        self.assertEqual(
            sorted(score for _, score in result),
            sorted(score for _, score in docs),
        )


if __name__ == "__main__":
    unittest.main()
