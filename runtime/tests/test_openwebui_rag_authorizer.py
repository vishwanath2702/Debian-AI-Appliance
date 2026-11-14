"""Fail-closed tests for the Open WebUI RAG preflight."""

import os
import runpy
import subprocess
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "runtime/daia-openwebui-rag-authorize"
BUILD = ROOT / "build/build-payload.sh"


def result(code=0, output=""):
    return SimpleNamespace(returncode=code, stdout=output)


class RAGAuthorizerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        namespace = runpy.run_path(str(HELPER))
        cls.preflight_function = staticmethod(namespace["preflight"])
        cls.error_type = namespace["AuthorizationError"]

    def test_nonroot_rejected_without_subprocess(self):
        with mock.patch.object(os, "geteuid", return_value=1000):
            with mock.patch.object(subprocess, "run") as runner:
                with self.assertRaises(self.error_type):
                    self.preflight_function()
                runner.assert_not_called()

    def test_pgvector_authorization_failure(self):
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run", return_value=result(1)
            ) as runner:
                with self.assertRaises(self.error_type):
                    self.preflight_function()
                self.assertEqual(runner.call_count, 1)

    def test_malformed_identity_rejected_before_docker(self):
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run",
                return_value=result(0, "not-an-id\n")
            ) as runner:
                with self.assertRaises(self.error_type):
                    self.preflight_function()
                self.assertEqual(runner.call_count, 1)

    def test_postgresql_readiness_failure(self):
        identity = "a" * 64
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run",
                side_effect=[
                    result(0, identity + "\n"),
                    result(1),
                ],
            ) as runner:
                with self.assertRaises(self.error_type):
                    self.preflight_function()
                self.assertEqual(runner.call_count, 2)

    def test_ready_database_requires_verified_role(self):
        identity = "b" * 64
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run",
                side_effect=[
                    result(0, identity + "\n"),
                    result(0),
                    result(0, ""),
                ],
            ) as runner:
                self.assertTrue(self.preflight_function())
                self.assertEqual(runner.call_count, 3)

                first = runner.call_args_list[0].args[0]
                second = runner.call_args_list[1].args[0]
                third = runner.call_args_list[2].args[0]

                self.assertEqual(
                    first,
                    ["/usr/libexec/daia-pgvector-authorize-start",
                     "runtime"],
                )
                self.assertEqual(
                    second[:3],
                    ["docker", "exec", identity],
                )
                self.assertEqual(
                    third,
                    [
                        "/usr/libexec/daia-openwebui-rag-provision.py",
                        "--verify-only",
                        identity,
                    ],
                )
                self.assertEqual(
                    runner.call_args_list[2].kwargs["timeout"], 90
                )

                for call in runner.call_args_list:
                    self.assertFalse(call.kwargs.get("shell", False))
                    self.assertNotIn("create", call.args[0])
                    self.assertNotIn("start", call.args[0])

    def test_role_verification_failure_blocks_authorization(self):
        identity = "c" * 64
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run",
                side_effect=[
                    result(0, identity + "\n"),
                    result(0),
                    result(1),
                ],
            ) as runner:
                with self.assertRaisesRegex(
                    self.error_type, "role authorization failed"
                ):
                    self.preflight_function()
                self.assertEqual(runner.call_count, 3)

    def test_unexpected_verifier_output_fails_closed(self):
        identity = "d" * 64
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run",
                side_effect=[
                    result(0, identity + "\n"),
                    result(0),
                    result(0, "ok\n"),
                ],
            ):
                with self.assertRaisesRegex(
                    self.error_type, "Unexpected"
                ):
                    self.preflight_function()

    def test_verifier_timeout_fails_closed(self):
        identity = "e" * 64
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run",
                side_effect=[
                    result(0, identity + "\n"),
                    result(0),
                    subprocess.TimeoutExpired("verify-only", 90),
                ],
            ):
                with self.assertRaises(self.error_type):
                    self.preflight_function()

    def test_cli_success_requires_all_checks(self):
        import sys
        namespace = runpy.run_path(str(HELPER))
        with (
            mock.patch.object(os, "geteuid", return_value=0),
            mock.patch.object(sys, "argv", [str(HELPER)]),
            mock.patch.object(
                subprocess, "run",
                side_effect=[
                    result(0, "f" * 64 + "\n"),
                    result(0),
                    result(0, ""),
                ],
            ),
        ):
            self.assertEqual(namespace["main"](), 0)

    def test_timeout_fails_closed(self):
        with mock.patch.object(os, "geteuid", return_value=0):
            with mock.patch.object(
                subprocess, "run",
                side_effect=subprocess.TimeoutExpired(
                    "authorization", 20
                ),
            ):
                with self.assertRaises(self.error_type):
                    self.preflight_function()

    def test_packaging_installs_required_executable(self):
        source = BUILD.read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-openwebui-rag-authorize"',
            source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/'
            'daia-openwebui-rag-authorize"',
            source,
        )
        self.assertIn(
            '0755 || return 1',
            source,
        )

    def test_installer_keeps_authorization_before_mutation(self):
        source = (
            ROOT / "runtime/modules/openwebui.sh"
        ).read_text()
        self.assertLess(
            source.index(
                "/usr/libexec/daia-openwebui-rag-authorize"
            ),
            source.index("docker load --input"),
        )
        self.assertIn('openwebui_prepare_rag "${tika_positional[@]}" || return 1', source)
        self.assertIn('if ! /usr/libexec/daia-openwebui-rag-authorize "${tika_flags[@]}"; then', source)
        self.assertLess(source.index('openwebui_prepare_rag "${tika_positional[@]}" || return 1'), source.index("docker load --input"))
        self.assertLess(source.index('if ! /usr/libexec/daia-openwebui-rag-authorize "${tika_flags[@]}"; then'), source.index("docker load --input"))
        self.assertLess(source.index("docker load --input"), source.index("docker container create"))
        self.assertLess(source.index("docker container create"), source.rindex('openwebui_start_registered "$container_id"'))


if __name__ == "__main__":
    unittest.main()
