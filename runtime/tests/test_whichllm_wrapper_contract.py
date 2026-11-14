import ast
import importlib.util
import unittest
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "daia-whichllm-offline.py"

class WrapperContractTests(unittest.TestCase):
    def test_upstream_cli_and_hardware_adaptation(self):
        spec = importlib.util.spec_from_file_location("daia_wrapper", SOURCE)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        command = module.recommendation_command("/usr/bin/bwrap", Path("/private"))
        self.assertIn("--unshare-net", command)
        self.assertIn("--context-length", command)
        self.assertIn("8192", command)
        self.assertIn("--fit", command)
        self.assertIn("any", command)
        self.assertIn("--json", command)
        self.assertNotIn("recommend", command)
        self.assertNotIn("--cpu-only", command)

    def test_snapshot_handoff_present(self):
        tree = ast.parse(SOURCE.read_text())
        self.assertTrue(any(
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr == "prepare_offline_cache"
            for node in ast.walk(tree)
        ))

    def test_no_index_url_dependency(self):
        source = SOURCE.read_text()
        self.assertNotIn("WHICHLLM_INDEX_URL", source)
        self.assertNotIn("index.json.gz", source)

if __name__ == "__main__":
    unittest.main()
