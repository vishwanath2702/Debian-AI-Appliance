"""SearXNG initializer security and filesystem tests."""

import ast
import importlib.util
import os
from pathlib import Path
import stat
import tempfile
import types
import unittest
from unittest.mock import patch

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "modules/searxng_initialize.py"
)

spec = importlib.util.spec_from_file_location(
    "searxng_initialize_production", SOURCE
)
production = importlib.util.module_from_spec(spec)
spec.loader.exec_module(production)


class TestOwnershipAdapter(ast.NodeTransformer):
    """Adapt ownership comparisons only in a test-local AST."""

    def __init__(self):
        self.replacements = {}

    def visit_Compare(self, node):
        node = self.generic_visit(node)

        if (
            len(node.ops) == 1
            and isinstance(node.ops[0], ast.Eq)
            and len(node.comparators) == 1
            and isinstance(node.comparators[0], ast.Constant)
            and node.comparators[0].value == 0
            and isinstance(node.left, ast.Attribute)
            and node.left.attr == "st_uid"
            and isinstance(node.left.value, ast.Name)
            and node.left.value.id in ("root_info", "info")
        ):
            name = node.left.value.id
            self.replacements[name] = (
                self.replacements.get(name, 0) + 1
            )

            replacement = ast.parse(
                f"{name}.st_uid in (0, os.geteuid())",
                mode="eval",
            ).body

            return ast.copy_location(replacement, node)

        return node


def load_isolated():
    tree = ast.parse(SOURCE.read_text(), str(SOURCE))
    adapter = TestOwnershipAdapter()
    tree = adapter.visit(tree)
    ast.fix_missing_locations(tree)

    if adapter.replacements != {
        "root_info": 1,
        "info": 2,
    }:
        raise AssertionError(
            f"Unexpected ownership comparisons: "
            f"{adapter.replacements}"
        )

    isolated = types.ModuleType(
        "searxng_initializer_isolated_test"
    )

    exec(
        compile(tree, "<isolated-test-initializer>", "exec"),
        isolated.__dict__,
    )
    return isolated


class InitializationTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.isolated = load_isolated()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(
            prefix="searxng-init-fixture-",
            dir=Path.home(),
        )
        self.addCleanup(self.temp.cleanup)

        self.base = Path(self.temp.name)
        self.parent = self.base / "state"
        self.parent.mkdir(mode=0o700)
        self.target = self.parent / "searxng"

    def initialize(self, path=None):
        self.isolated._initialize(
            str(path if path is not None else self.target)
        )

    def test_private_directory_created(self):
        self.initialize()
        self.assertEqual(
            stat.S_IMODE(self.target.stat().st_mode),
            0o700,
        )

    def test_repeat_rejected(self):
        self.initialize()
        with self.assertRaises(FileExistsError):
            self.initialize()

    def test_existing_directory_rejected(self):
        self.target.mkdir(mode=0o700)
        with self.assertRaises(FileExistsError):
            self.initialize()

    def test_existing_file_preserved(self):
        self.target.write_text("preserve")
        with self.assertRaises(FileExistsError):
            self.initialize()
        self.assertEqual(
            self.target.read_text(), "preserve"
        )

    def test_symlink_parent_rejected(self):
        actual = self.base / "actual"
        actual.mkdir(mode=0o700)
        link = self.base / "linked"
        link.symlink_to(
            actual, target_is_directory=True
        )

        with self.assertRaises(OSError):
            self.initialize(link / "searxng")

        self.assertFalse(
            (actual / "searxng").exists()
        )

    def test_writable_parent_rejected(self):
        self.parent.chmod(0o777)
        with self.assertRaises(
            self.isolated.InitializationError
        ):
            self.initialize()
        self.assertFalse(self.target.exists())

    def test_relative_path_rejected(self):
        with self.assertRaises(
            self.isolated.InitializationError
        ):
            self.initialize(Path("relative/searxng"))

    def test_parent_traversal_rejected(self):
        with self.assertRaises(
            self.isolated.InitializationError
        ):
            self.initialize(
                self.parent / ".." / "state" / "searxng"
            )

    def test_nonroot_public_entry_rejected(self):
        with patch.object(
            production.os, "geteuid", return_value=1000
        ):
            with self.assertRaises(
                production.InitializationError
            ):
                production.initialize()

    def test_public_entry_rejects_custom_path(self):
        with patch.object(
            production.os, "geteuid", return_value=0
        ):
            with self.assertRaises(
                production.InitializationError
            ):
                production.initialize(str(self.target))

    def test_no_transition_interface(self):
        tree = ast.parse(SOURCE.read_text())
        functions = {
            node.name: node
            for node in tree.body
            if isinstance(node, ast.FunctionDef)
        }

        for name in (
            "_open_parent",
            "_initialize",
            "initialize",
        ):
            parameters = [
                arg.arg
                for arg in functions[name].args.args
            ]
            self.assertNotIn("transition", parameters)
            self.assertNotIn("owner_uid", parameters)

    def test_production_root_checks_unchanged(self):
        source = SOURCE.read_text()
        self.assertIn("root_info.st_uid == 0", source)
        self.assertIn("info.st_uid == 0", source)
        self.assertIn("os.geteuid() == 0", source)
        self.assertNotIn("transition=", source)


if __name__ == "__main__":
    unittest.main()
