"""SearXNG pure first-boot planning tests."""

import ast
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "runtime/modules"
sys.path.insert(0, str(MODULES))

import searxng_firstboot_plan as subject
from searxng_firstboot_adapter import Disposition


class PlanningTests(unittest.TestCase):

    def test_disabled_is_noop(self):
        result = subject.plan(Disposition.SKIPPED)
        self.assertEqual(result.requirement, subject.Requirement.NONE)
        self.assertTrue(result.may_mark_complete)

    def test_verified_requires_reverification(self):
        result = subject.plan(Disposition.VERIFIED)
        self.assertEqual(
            result.requirement,
            subject.Requirement.VERIFY_EXISTING,
        )
        self.assertFalse(result.may_mark_complete)

    def test_initialization_requires_authorization(self):
        result = subject.plan(
            Disposition.NEEDS_INITIALIZATION_AUTHORIZATION
        )
        self.assertEqual(
            result.requirement,
            subject.Requirement.AUTHORIZE_INITIALIZATION,
        )
        self.assertFalse(result.may_mark_complete)

    def test_provisioning_requires_authorization(self):
        result = subject.plan(
            Disposition.NEEDS_PROVISION_AUTHORIZATION
        )
        self.assertEqual(
            result.requirement,
            subject.Requirement.AUTHORIZE_PROVISIONING,
        )
        self.assertFalse(result.may_mark_complete)

    def test_refusal(self):
        result = subject.plan(Disposition.REFUSED)
        self.assertEqual(result.requirement, subject.Requirement.REFUSE)
        self.assertFalse(result.may_mark_complete)

    def test_invalid_values_refuse(self):
        for value in (None, True, False, 0, 1, "VERIFIED", object()):
            with self.subTest(value=repr(value)):
                result = subject.plan(value)
                self.assertEqual(
                    result.requirement,
                    subject.Requirement.REFUSE,
                )
                self.assertFalse(result.may_mark_complete)

    def test_all_dispositions_handled(self):
        for value in Disposition:
            with self.subTest(value=value):
                result = subject.plan(value)
                self.assertIs(type(result), subject.Plan)
                self.assertIs(type(result.requirement), subject.Requirement)
                self.assertIs(type(result.may_mark_complete), bool)

    def test_plan_is_immutable(self):
        result = subject.plan(Disposition.SKIPPED)
        with self.assertRaises(Exception):
            result.may_mark_complete = False

    def test_assess_plan_disabled(self):
        with patch(
            "searxng_firstboot_adapter.assess",
            return_value=Disposition.SKIPPED,
        ):
            result = subject.assess_plan(None, None)
        self.assertEqual(result.requirement, subject.Requirement.NONE)

    def test_assess_plan_verified(self):
        with patch(
            "searxng_firstboot_adapter.assess",
            return_value=Disposition.VERIFIED,
        ):
            result = subject.assess_plan(None, None)
        self.assertEqual(
            result.requirement,
            subject.Requirement.VERIFY_EXISTING,
        )

    def test_assess_plan_failure_refuses(self):
        with patch(
            "searxng_firstboot_adapter.assess",
            side_effect=RuntimeError("inspection failed"),
        ):
            result = subject.assess_plan(None, None)
        self.assertEqual(result.requirement, subject.Requirement.REFUSE)

    def test_no_mutating_calls(self):
        tree = ast.parse(
            (MODULES / "searxng_firstboot_plan.py").read_text()
        )
        forbidden = {
            "subprocess", "docker", "systemctl",
            "mkdir", "makedirs", "unlink",
            "remove", "rename", "replace",
            "chmod", "chown", "open",
        }
        for node in ast.walk(tree):
            if isinstance(node, ast.Call):
                fn = node.func
                if isinstance(fn, ast.Name):
                    self.assertNotIn(fn.id, forbidden)
                elif isinstance(fn, ast.Attribute):
                    self.assertNotIn(fn.attr, forbidden)

    def test_not_wired_into_firstboot(self):
        self.assertNotIn(
            "searxng_firstboot_plan",
            (ROOT / "runtime/firstboot.sh").read_text(),
        )

    def test_not_wired_into_runtime(self):
        self.assertNotIn(
            "searxng_firstboot_plan",
            (ROOT / "runtime/runtime.sh").read_text(),
        )

    def test_no_auto_provisioning(self):
        source = (
            MODULES / "searxng_firstboot_plan.py"
        ).read_text()
        self.assertNotIn("searxng_provision_container(", source)
        self.assertNotIn("searxng_start_registered(", source)


if __name__ == "__main__":
    unittest.main()
