"""Mocked behavioral checks for guarded Open WebUI startup."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"
IDENTITY = "a" * 64


class GuardedStartTests(unittest.TestCase):
    def run_case(self, *, auth="ok", verify="ok",
                 identity="ok", start="ok", supplied=IDENTITY):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            actions = folder / "actions.log"
            helper = folder / "trusted-helper"
            isolated = folder / "openwebui.sh"

            helper.write_text(
                '#!/bin/sh\n'
                'printf "helper:%s\\n" "$1" >> "$ACTION_LOG"\n'
                'case "$1" in\n'
                '  --authorize-start) [ "$MOCK_AUTH" = ok ] || exit 1 ;;\n'
                '  --verify-running) [ "$MOCK_VERIFY" = ok ] || exit 1 ;;\n'
                '  --verify-identity) [ "$MOCK_IDENTITY" = ok ] || exit 1 ;;\n'
                '  *) exit 1 ;;\n'
                'esac\n'
                'printf "%s\\n" "$2"\n',
                encoding="utf-8",
            )
            helper.chmod(0o700)

            source = MODULE.read_text(encoding="utf-8")
            start_at = source.index("openwebui_start_registered() {")
            end_at = source.index("\nopenwebui_install() {", start_at)
            function = source[start_at:end_at]

            self.assertIn('if [[ "$EUID" -ne 0 ]]; then', function)
            function = function.replace(
                'if [[ "$EUID" -ne 0 ]]; then',
                'if false; then',
                1,
            )
            function = function.replace(
                "/usr/libexec/daia-openwebui-register-container",
                str(helper),
                1,
            )
            isolated.write_text(function, encoding="utf-8")

            script = (
                'docker() {\n'
                '  printf "docker:%s\\n" "$1" >> "$ACTION_LOG"\n'
                '  if [[ "$1" == start && "$MOCK_START" != ok ]]; then\n'
                '    return 1\n'
                '  fi\n'
                '  return 0\n'
                '}\n'
                'source "$1"\n'
                'openwebui_start_registered "$2"\n'
            )
            env = {
                **os.environ,
                "ACTION_LOG": str(actions),
                "MOCK_AUTH": auth,
                "MOCK_VERIFY": verify,
                "MOCK_IDENTITY": identity,
                "MOCK_START": start,
            }
            result = subprocess.run(
                ["bash", "-c", script, "bash", str(isolated), supplied],
                capture_output=True,
                text=True,
                check=False,
                timeout=15,
                env=env,
            )
            observed = (
                actions.read_text(encoding="utf-8").splitlines()
                if actions.exists() else []
            )
            return result, observed

    def test_authorized_start_and_running_proof(self):
        result, actions = self.run_case()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            actions,
            [
                "helper:--authorize-start",
                "docker:start",
                "helper:--verify-running",
            ],
        )

    def test_denied_authorization_never_starts(self):
        result, actions = self.run_case(auth="fail")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(actions, ["helper:--authorize-start"])

    def test_invalid_identity_never_calls_helper(self):
        result, actions = self.run_case(supplied="invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(actions, [])

    def test_failed_docker_start_never_claims_running(self):
        result, actions = self.run_case(start="fail")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            actions, ["helper:--authorize-start", "docker:start"]
        )

    def test_failed_verification_stops_verified_identity(self):
        result, actions = self.run_case(verify="fail")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            actions,
            [
                "helper:--authorize-start",
                "docker:start",
                "helper:--verify-running",
                "helper:--verify-identity",
                "docker:stop",
            ],
        )

    def test_failed_identity_proof_never_stops_container(self):
        result, actions = self.run_case(
            verify="fail", identity="fail"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            actions,
            [
                "helper:--authorize-start",
                "docker:start",
                "helper:--verify-running",
                "helper:--verify-identity",
            ],
        )

    def test_production_retains_root_and_immutable_id_checks(self):
        source = MODULE.read_text(encoding="utf-8")
        function = source.split(
            "openwebui_start_registered() {", 1
        )[1].split("\nopenwebui_install() {", 1)[0]
        self.assertIn('"$EUID" -ne 0', function)
        self.assertIn('^[0-9a-f]{64}$', function)
        self.assertIn('docker start "$container_id"', function)
        self.assertNotIn("docker rm", function)
        self.assertNotIn("docker volume rm", function)


if __name__ == "__main__":
    unittest.main()
