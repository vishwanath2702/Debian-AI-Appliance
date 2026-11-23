"""Fail-closed SearXNG execution-gate regression tests."""

import ast
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
MODULES = ROOT / "modules"
sys.path.insert(0, str(MODULES))

from searxng_firstboot_authority import Assessment, Status
import searxng_firstboot_execution as execution


class ExecutionGateTests(unittest.TestCase):

    def test_disabled_skip(self):
        with patch.object(execution.os, "geteuid", return_value=0):
            result = execution.evaluate(
                Assessment(Status.SKIPPED, False, True)
            )
        self.assertIs(result.decision, execution.Decision.SKIP)
        self.assertFalse(result.execution_authorized)
        self.assertFalse(result.may_mark_searxng_complete)

    def test_candidates_never_authorized(self):
        for status in (
            Status.VERIFIED_CANDIDATE,
            Status.INITIALIZATION_CANDIDATE,
            Status.PROVISIONING_CANDIDATE,
            Status.REFUSED,
        ):
            with self.subTest(status=status):
                with patch.object(execution.os, "geteuid", return_value=0):
                    result = execution.evaluate(Assessment(status))
                self.assertEqual(result, execution.REFUSAL)

    def test_forged_execution_authorization_refused(self):
        with patch.object(execution.os, "geteuid", return_value=0):
            result = execution.evaluate(
                Assessment(Status.SKIPPED, True, True)
            )
        self.assertEqual(result, execution.REFUSAL)

    def test_nonroot_refused(self):
        with patch.object(execution.os, "geteuid", return_value=1000):
            result = execution.evaluate(
                Assessment(Status.SKIPPED, False, True)
            )
        self.assertEqual(result, execution.REFUSAL)

    def test_untyped_input_refused(self):
        with patch.object(execution.os, "geteuid", return_value=0):
            self.assertEqual(
                execution.evaluate({"status": "SKIPPED"}),
                execution.REFUSAL,
            )

    def test_malformed_assessment_refused(self):
        cases = (
            Assessment(Status.SKIPPED, False, False),
            Assessment(Status.SKIPPED, False, 1),
            Assessment(Status.SKIPPED, 1, True),
            Assessment("SKIPPED", False, True),
        )
        with patch.object(execution.os, "geteuid", return_value=0):
            for assessment in cases:
                with self.subTest(assessment=assessment):
                    self.assertEqual(
                        execution.evaluate(assessment),
                        execution.REFUSAL,
                    )

    def test_no_mutation_primitives(self):
        tree = ast.parse(
            (MODULES / "searxng_firstboot_execution.py").read_text()
        )
        imports = [
            node for node in ast.walk(tree)
            if isinstance(node, (ast.Import, ast.ImportFrom))
        ]
        names = []
        for node in imports:
            if isinstance(node, ast.Import):
                names.extend(alias.name for alias in node.names)
            else:
                names.append(node.module or "")
        self.assertFalse(
            set(names) & {"subprocess", "shutil", "socket"}
        )

    def test_not_wired_into_firstboot(self):
        source = (ROOT / "firstboot.sh").read_text()
        self.assertNotIn(
            "searxng_firstboot_execution", source
        )

    def test_not_wired_into_runtime(self):
        source = (ROOT / "runtime.sh").read_text()
        self.assertNotIn(
            "searxng_firstboot_execution", source
        )


if __name__ == "__main__":
    unittest.main()
