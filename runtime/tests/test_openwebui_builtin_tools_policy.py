import ast
import hashlib
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / (
    "payload/patches/open-webui/v0.13.0/"
    "patched-middleware.py"
)
EXPECTED_SHA = (
    "a8b1bb595fdbb0bc3cfeb8600e16b59"
    "c694f88b2163244fed0342cee46ad93b8"
)

class BuiltinToolsPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        data = SOURCE.read_bytes()
        assert hashlib.sha256(data).hexdigest() == EXPECTED_SHA
        tree = ast.parse(data)
        matches = [
            node for node in ast.walk(tree)
            if isinstance(node, ast.Assign)
            and any(
                isinstance(target, ast.Name)
                and target.id == "use_builtin_tools"
                for target in node.targets
            )
        ]
        assert len(matches) == 1
        expression = ast.Expression(matches[0].value)
        cls.policy = compile(expression, str(SOURCE), "eval")

    def evaluate(self, *, note=False, session=True,
                 legacy=False, capability=None):
        metadata = {
            "session_id": "session" if session else None,
            "params": {
                "function_calling": "legacy" if legacy else "native"
            },
        }
        capabilities = {}
        if capability is not None:
            capabilities["builtin_tools"] = capability
        model = {
            "info": {
                "meta": {
                    "capabilities": capabilities
                }
            }
        }
        return bool(eval(
            self.policy,
            {"__builtins__": {"bool": bool}},
            {
                "is_note_chat": note,
                "metadata": metadata,
                "model": model,
            },
        ))

    def test_ordinary_chat_defaults_off(self):
        self.assertFalse(self.evaluate())

    def test_explicit_model_opt_in(self):
        self.assertTrue(self.evaluate(capability=True))

    def test_explicit_model_opt_out(self):
        self.assertFalse(self.evaluate(capability=False))

    def test_internal_note_exception(self):
        self.assertTrue(self.evaluate(note=True))

    def test_internal_note_without_session(self):
        self.assertTrue(self.evaluate(note=True, session=False))

    def test_api_request_without_session(self):
        self.assertFalse(
            self.evaluate(session=False, capability=True)
        )

    def test_legacy_function_calling(self):
        self.assertFalse(
            self.evaluate(legacy=True, capability=True)
        )

    def test_legacy_internal_note_exception(self):
        self.assertTrue(
            self.evaluate(note=True, legacy=True)
        )

if __name__ == "__main__":
    unittest.main()
