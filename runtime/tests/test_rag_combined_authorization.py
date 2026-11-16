import importlib.util
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[1]
RECOVERY = ROOT / "daia-rag-recover"
INITIALIZER = ROOT / "daia-pgvector-init-extension.py"


def load_initializer():
    spec = importlib.util.spec_from_file_location(
        "rag_combined_initializer", INITIALIZER
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CombinedAuthorizationTests(unittest.TestCase):
    def test_recovery_passes_both_registered_identities(self):
        source = RECOVERY.read_text()
        self.assertIn(
            '/usr/libexec/daia-pgvector-authorize-start runtime '
            '"$webui_id" --tika-id "$tika_id"',
            source,
        )
        self.assertIn(
            '/usr/libexec/daia-pgvector-init-extension.py '
            '"$pg_id" "$webui_id" "$tika_id"',
            source,
        )

    def test_initializer_passes_combined_identities(self):
        module = load_initializer()
        pg_id, webui_id, tika_id = ("a" * 64, "b" * 64, "c" * 64)
        calls = []

        def runner(command, **kwargs):
            calls.append(command)
            return SimpleNamespace(returncode=0, stdout=pg_id + "\n")

        result = module.authorize(
            runner, webui_id=webui_id, tika_id=tika_id
        )
        self.assertEqual(result, pg_id)
        self.assertEqual(calls, [[
            module.AUTHORIZER, "runtime", webui_id,
            "--tika-id", tika_id
        ]])

    def test_initializer_rejects_incomplete_identities(self):
        module = load_initializer()
        with self.assertRaises(module.InitializationError):
            module.authorize(webui_id="b" * 64)

    def test_initializer_rejects_invalid_identity_before_runner(self):
        module = load_initializer()
        calls = []

        def runner(*args, **kwargs):
            calls.append(args)
            raise AssertionError("runner must not be called")

        with self.assertRaises(module.InitializationError):
            module.authorize(
                runner, webui_id="invalid", tika_id="c" * 64
            )
        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
