import ast
import pathlib
import unittest
from unittest import mock

SOURCE = pathlib.Path(__file__).resolve().parents[1] / (
    "daia-tika-register-container"
)

class TikaRuntimePeerIdentityTests(unittest.TestCase):
    def test_runtime_passes_registered_webui_and_tika_ids(self):
        tree = ast.parse(SOURCE.read_text())
        function = next(
            node for node in tree.body
            if isinstance(node, ast.FunctionDef)
            and node.name == "verify_runtime_network"
        )
        module = ast.Module(body=[function], type_ignores=[])
        namespace = {
            "validate_container_id": lambda value: value,
            "read_trusted_registration": lambda path: {
                "/var/lib/daia/pgvector": "a" * 64,
                "/var/lib/daia/openwebui": "b" * 64,
            }[path],
            "subprocess": mock.Mock(),
            "PermissionError": PermissionError,
        }
        namespace["subprocess"].run.return_value.stdout = "a" * 64 + "\n"
        exec(compile(module, str(SOURCE), "exec"), namespace)

        namespace["verify_runtime_network"]("c" * 64)

        args = namespace["subprocess"].run.call_args.args[0]
        self.assertEqual(
            args,
            [
                "/usr/libexec/daia-pgvector-authorize-start",
                "runtime", "b" * 64, "--tika-id", "c" * 64,
            ],
        )
        self.assertTrue(
            namespace["subprocess"].run.call_args.kwargs["check"]
        )

if __name__ == "__main__":
    unittest.main()
