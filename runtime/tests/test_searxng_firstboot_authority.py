"""Regression tests for read-only SearXNG authority assessment."""
import ast
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "runtime/modules"
sys.path.insert(0, str(MODULES))

import searxng_firstboot_authority as authority
from searxng_firstboot_authority import Status
from searxng_firstboot_authorization import Grant
from searxng_firstboot_plan import Plan, Requirement


class AuthorityTests(unittest.TestCase):

    def evaluate_plan(self, plan, grant=Grant.NONE):
        with patch.object(authority.os, "geteuid", return_value=0):
            with patch.object(
                authority, "assess_plan", return_value=plan
            ) as observer:
                result = authority.evaluate(object(), object(), grant)
                if type(grant) is Grant:
                    observer.assert_called_once()
                else:
                    observer.assert_not_called()
                return result

    def test_nonroot_refused_without_observation(self):
        with patch.object(authority.os, "geteuid", return_value=1000):
            with patch.object(authority, "assess_plan") as observer:
                result = authority.evaluate(None, None)
                self.assertEqual(result.status, Status.REFUSED)
                observer.assert_not_called()

    def test_disabled_noop(self):
        result = self.evaluate_plan(Plan(Requirement.NONE, True))
        self.assertEqual(result.status, Status.SKIPPED)
        self.assertTrue(result.may_mark_complete)

    def test_disabled_rejects_mutation_grant(self):
        for grant in (Grant.INITIALIZE, Grant.PROVISION, Grant.VERIFY):
            with self.subTest(grant=grant):
                result = self.evaluate_plan(
                    Plan(Requirement.NONE, True), grant
                )
                self.assertEqual(result.status, Status.REFUSED)

    def test_initialization_candidate(self):
        result = self.evaluate_plan(
            Plan(Requirement.AUTHORIZE_INITIALIZATION, False),
            Grant.INITIALIZE
        )
        self.assertEqual(result.status, Status.INITIALIZATION_CANDIDATE)

    def test_provisioning_candidate(self):
        result = self.evaluate_plan(
            Plan(Requirement.AUTHORIZE_PROVISIONING, False),
            Grant.PROVISION
        )
        self.assertEqual(result.status, Status.PROVISIONING_CANDIDATE)

    def test_verification_candidate(self):
        result = self.evaluate_plan(
            Plan(Requirement.VERIFY_EXISTING, False),
            Grant.VERIFY
        )
        self.assertEqual(result.status, Status.VERIFIED_CANDIDATE)

    def test_missing_grant_refused(self):
        for requirement in (
            Requirement.AUTHORIZE_INITIALIZATION,
            Requirement.AUTHORIZE_PROVISIONING,
            Requirement.VERIFY_EXISTING,
        ):
            with self.subTest(requirement=requirement):
                result = self.evaluate_plan(Plan(requirement, False))
                self.assertEqual(result.status, Status.REFUSED)

    def test_cross_grants_refused(self):
        pairs = (
            (Requirement.AUTHORIZE_INITIALIZATION, Grant.INITIALIZE),
            (Requirement.AUTHORIZE_PROVISIONING, Grant.PROVISION),
            (Requirement.VERIFY_EXISTING, Grant.VERIFY),
        )
        for requirement, expected in pairs:
            for grant in Grant:
                if grant is expected:
                    continue
                with self.subTest(requirement=requirement, grant=grant):
                    result = self.evaluate_plan(
                        Plan(requirement, False), grant
                    )
                    self.assertEqual(result.status, Status.REFUSED)

    def test_untrusted_state_refused(self):
        for grant in Grant:
            result = self.evaluate_plan(
                Plan(Requirement.REFUSE, False), grant
            )
            self.assertEqual(result.status, Status.REFUSED)

    def test_inconsistent_completion_refused(self):
        for requirement in Requirement:
            if requirement is Requirement.NONE:
                continue
            result = self.evaluate_plan(
                Plan(requirement, True), Grant.PROVISION
            )
            self.assertEqual(result.status, Status.REFUSED)

    def test_invalid_grants_refused_without_observation(self):
        for grant in (None, True, 1, "PROVISION", object()):
            with self.subTest(grant=repr(grant)):
                result = self.evaluate_plan(
                    Plan(Requirement.AUTHORIZE_PROVISIONING, False),
                    grant
                )
                self.assertEqual(result.status, Status.REFUSED)

    def test_invalid_plans_refused(self):
        for plan in (
            None, "plan",
            Plan("PROVISION", False),
            Plan(Requirement.NONE, 1),
        ):
            result = self.evaluate_plan(plan)
            self.assertEqual(result.status, Status.REFUSED)

    def test_observation_exception_refused(self):
        with patch.object(authority.os, "geteuid", return_value=0):
            with patch.object(
                authority, "assess_plan",
                side_effect=RuntimeError("inspection failed")
            ):
                result = authority.evaluate(None, None)
        self.assertEqual(result.status, Status.REFUSED)

    def test_no_execution_authorization(self):
        for requirement in Requirement:
            for complete in (False, True):
                for grant in Grant:
                    result = self.evaluate_plan(
                        Plan(requirement, complete), grant
                    )
                    self.assertFalse(result.execution_authorized)

    def test_only_disabled_may_complete(self):
        for requirement in Requirement:
            for complete in (False, True):
                for grant in Grant:
                    result = self.evaluate_plan(
                        Plan(requirement, complete), grant
                    )
                    if result.may_mark_complete:
                        self.assertEqual(result.status, Status.SKIPPED)

    def test_assessment_immutable(self):
        result = self.evaluate_plan(Plan(Requirement.NONE, True))
        with self.assertRaises(Exception):
            result.execution_authorized = True

    def test_no_mutation_calls(self):
        tree = ast.parse(
            (MODULES / "searxng_firstboot_authority.py").read_text()
        )
        forbidden = {
            "subprocess", "docker", "systemctl", "mkdir",
            "makedirs", "unlink", "remove", "rename",
            "replace", "chmod", "chown", "open",
            "Popen", "run", "system",
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
            "searxng_firstboot_authority",
            (ROOT / "runtime/firstboot.sh").read_text()
        )

    def test_not_wired_into_runtime(self):
        self.assertNotIn(
            "searxng_firstboot_authority",
            (ROOT / "runtime/runtime.sh").read_text()
        )


if __name__ == "__main__":
    unittest.main()
