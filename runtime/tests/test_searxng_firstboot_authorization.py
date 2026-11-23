"""SearXNG first-boot authorization policy tests."""

import ast
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "runtime/modules"
sys.path.insert(0, str(MODULES))

from searxng_firstboot_plan import Plan, Requirement
from searxng_firstboot_authorization import (
    Authorization,
    Grant,
    Verdict,
    authorize,
)


class AuthorizationTests(unittest.TestCase):

    def test_disabled_noop(self):
        result = authorize(
            Plan(Requirement.NONE, True),
            Grant.NONE,
        )
        self.assertEqual(result.verdict, Verdict.SKIP)
        self.assertTrue(result.may_mark_complete)

    def test_disabled_rejects_other_grants(self):
        for grant in (Grant.INITIALIZE, Grant.PROVISION, Grant.VERIFY):
            with self.subTest(grant=grant):
                self.assertEqual(
                    authorize(
                        Plan(Requirement.NONE, True), grant
                    ).verdict,
                    Verdict.REFUSE,
                )

    def test_initialization_requires_matching_grant(self):
        result = authorize(
            Plan(Requirement.AUTHORIZE_INITIALIZATION, False),
            Grant.INITIALIZE,
        )
        self.assertEqual(
            result.verdict,
            Verdict.AUTHORIZED_INITIALIZATION,
        )
        self.assertFalse(result.may_mark_complete)

    def test_provision_requires_matching_grant(self):
        result = authorize(
            Plan(Requirement.AUTHORIZE_PROVISIONING, False),
            Grant.PROVISION,
        )
        self.assertEqual(
            result.verdict,
            Verdict.AUTHORIZED_PROVISIONING,
        )
        self.assertFalse(result.may_mark_complete)

    def test_verification_requires_matching_grant(self):
        result = authorize(
            Plan(Requirement.VERIFY_EXISTING, False),
            Grant.VERIFY,
        )
        self.assertEqual(
            result.verdict,
            Verdict.AUTHORIZED_VERIFICATION,
        )
        self.assertFalse(result.may_mark_complete)

    def test_no_grant_never_authorizes_mutation(self):
        for requirement in (
            Requirement.AUTHORIZE_INITIALIZATION,
            Requirement.AUTHORIZE_PROVISIONING,
            Requirement.VERIFY_EXISTING,
        ):
            with self.subTest(requirement=requirement):
                result = authorize(
                    Plan(requirement, False),
                    Grant.NONE,
                )
                self.assertEqual(result.verdict, Verdict.REFUSE)

    def test_cross_grants_refused(self):
        pairs = (
            (Requirement.AUTHORIZE_INITIALIZATION, Grant.PROVISION),
            (Requirement.AUTHORIZE_INITIALIZATION, Grant.VERIFY),
            (Requirement.AUTHORIZE_PROVISIONING, Grant.INITIALIZE),
            (Requirement.AUTHORIZE_PROVISIONING, Grant.VERIFY),
            (Requirement.VERIFY_EXISTING, Grant.INITIALIZE),
            (Requirement.VERIFY_EXISTING, Grant.PROVISION),
        )
        for requirement, grant in pairs:
            with self.subTest(requirement=requirement, grant=grant):
                self.assertEqual(
                    authorize(
                        Plan(requirement, False), grant
                    ).verdict,
                    Verdict.REFUSE,
                )

    def test_refusal_cannot_be_overridden(self):
        for grant in Grant:
            with self.subTest(grant=grant):
                result = authorize(
                    Plan(Requirement.REFUSE, False),
                    grant,
                )
                self.assertEqual(result.verdict, Verdict.REFUSE)

    def test_inconsistent_completion_refused(self):
        for requirement in Requirement:
            if requirement is Requirement.NONE:
                continue
            for grant in Grant:
                with self.subTest(
                    requirement=requirement, grant=grant
                ):
                    result = authorize(
                        Plan(requirement, True), grant
                    )
                    self.assertEqual(result.verdict, Verdict.REFUSE)

    def test_none_without_completion_refused(self):
        result = authorize(
            Plan(Requirement.NONE, False),
            Grant.NONE,
        )
        self.assertEqual(result.verdict, Verdict.REFUSE)

    def test_invalid_plan_refused(self):
        for value in (None, True, "plan", 0, object()):
            with self.subTest(value=repr(value)):
                self.assertEqual(
                    authorize(value, Grant.INITIALIZE).verdict,
                    Verdict.REFUSE,
                )

    def test_invalid_grant_refused(self):
        plan = Plan(
            Requirement.AUTHORIZE_INITIALIZATION, False
        )
        for value in (None, True, 1, "INITIALIZE", object()):
            with self.subTest(value=repr(value)):
                self.assertEqual(
                    authorize(plan, value).verdict,
                    Verdict.REFUSE,
                )

    def test_invalid_requirement_refused(self):
        result = authorize(
            Plan("AUTHORIZE_INITIALIZATION", False),
            Grant.INITIALIZE,
        )
        self.assertEqual(result.verdict, Verdict.REFUSE)

    def test_invalid_completion_type_refused(self):
        result = authorize(
            Plan(Requirement.NONE, 1),
            Grant.NONE,
        )
        self.assertEqual(result.verdict, Verdict.REFUSE)

    def test_authorization_immutable(self):
        result = authorize(
            Plan(Requirement.NONE, True),
            Grant.NONE,
        )
        with self.assertRaises(Exception):
            result.may_mark_complete = False

    def test_every_plan_grant_combination(self):
        for requirement in Requirement:
            for completion in (False, True):
                for grant in Grant:
                    with self.subTest(
                        requirement=requirement,
                        completion=completion,
                        grant=grant,
                    ):
                        result = authorize(
                            Plan(requirement, completion),
                            grant,
                        )
                        self.assertIs(type(result), Authorization)
                        self.assertIs(type(result.verdict), Verdict)
                        self.assertIs(
                            type(result.may_mark_complete), bool
                        )

    def test_only_skip_may_complete(self):
        for requirement in Requirement:
            for completion in (False, True):
                for grant in Grant:
                    result = authorize(
                        Plan(requirement, completion), grant
                    )
                    if result.may_mark_complete:
                        self.assertEqual(
                            result.verdict, Verdict.SKIP
                        )

    def test_no_mutating_interfaces(self):
        tree = ast.parse(
            (MODULES / "searxng_firstboot_authorization.py").read_text()
        )
        forbidden = {
            "subprocess", "docker", "systemctl",
            "mkdir", "makedirs", "unlink",
            "remove", "rename", "replace",
            "chmod", "chown", "open",
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
            "searxng_firstboot_authorization",
            (ROOT / "runtime/firstboot.sh").read_text(),
        )

    def test_not_wired_into_runtime(self):
        self.assertNotIn(
            "searxng_firstboot_authorization",
            (ROOT / "runtime/runtime.sh").read_text(),
        )


if __name__ == "__main__":
    unittest.main()
