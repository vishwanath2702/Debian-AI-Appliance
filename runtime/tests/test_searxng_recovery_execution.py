"""Execute the production SearXNG recovery section with inert mocks."""
import os
from pathlib import Path
import subprocess
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "daia-rag-recover"
MARKER = "# SearXNG recovery is optional"
CID = "a" * 64


class RecoveryExecutionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        source = SOURCE.read_text()
        assert source.count(MARKER) == 1
        section = MARKER + source.split(MARKER, 1)[1]

        # Test-only substitution of filesystem-dependent operations.
        # No production file is modified by these substitutions.
        expected = (
            '[[ -f "$searxng_module" && ! -L "$searxng_module" ]] ||\n'
            '            exit 1'
        )
        assert section.count(expected) == 1
        section = section.replace(expected, ":", 1)

        source_command = 'source "$searxng_module"'
        assert section.count(source_command) == 1
        section = section.replace(source_command, ":", 1)

        helper = "/usr/libexec/daia-searxng-verify-registration"
        assert section.count(helper) == 1
        section = section.replace(helper, "verify_registration", 1)

        cls.section = section

    def execute(self, feature="enabled", state="stopped",
                failure="none", identity=None):
        identity = CID if identity is None else identity

        harness = r"""
set -euo pipefail
MODULE_ROOT=/mock/modules

python3() {
    # Consume the actual production heredoc without running Python.
    cat >/dev/null
    case "$FEATURE" in
        enabled) return 0 ;;
        disabled) return 2 ;;
        *) return 1 ;;
    esac
}

verify_registration() {
    echo "$IDENTITY"
    [[ "$FAILURE" != registration ]]
}

docker() {
    [[ "$1" == container && "$2" == inspect ]] || return 90
    [[ "$FAILURE" != inspect ]] || return 1
    case "$STATE" in
        stopped) echo 'false|false|false|exited' ;;
        created) echo 'false|false|false|created' ;;
        running) echo 'true|false|false|running' ;;
        paused) echo 'true|true|false|paused' ;;
        *) return 91 ;;
    esac
}

searxng_start_registered() {
    echo "EVENT:start:$1"
    [[ "$FAILURE" != start ]]
}

searxng_verify_network_contract() {
    echo "EVENT:network"
    [[ "$FAILURE" != network ]]
}

searxng_wait_ready() {
    echo "EVENT:ready:$1:$2"
    [[ "$FAILURE" != ready ]]
}
"""
        env = os.environ.copy()
        env.update({
            "FEATURE": feature,
            "STATE": state,
            "FAILURE": failure,
            "IDENTITY": identity,
        })
        return subprocess.run(
            ["bash", "-c", harness + "\n" + self.section],
            env=env, text=True, capture_output=True,
            timeout=10, check=False,
        )

    def test_disabled_skips_recovery(self):
        result = self.execute(feature="disabled")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("EVENT:", result.stdout)

    def test_stopped_starts_and_waits(self):
        result = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("EVENT:start:" + CID, result.stdout)
        self.assertIn("EVENT:ready:" + CID + ":60", result.stdout)

    def test_created_starts_and_waits(self):
        result = self.execute(state="created")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("EVENT:start:" + CID, result.stdout)

    def test_running_verifies_without_restart(self):
        result = self.execute(state="running")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("EVENT:network", result.stdout)
        self.assertIn("EVENT:ready:", result.stdout)
        self.assertNotIn("EVENT:start:", result.stdout)

    def test_invalid_state_fails(self):
        result = self.execute(state="paused")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("EVENT:ready:", result.stdout)

    def test_invalid_configuration_fails(self):
        result = self.execute(feature="invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("EVENT:", result.stdout)

    def test_registration_failure_fails(self):
        result = self.execute(failure="registration")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("EVENT:start:", result.stdout)

    def test_identity_mismatch_fails(self):
        result = self.execute(identity="invalid")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("EVENT:start:", result.stdout)

    def test_inspection_failure_fails(self):
        result = self.execute(failure="inspect")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("EVENT:start:", result.stdout)

    def test_start_failure_fails(self):
        result = self.execute(failure="start")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("EVENT:ready:", result.stdout)

    def test_network_failure_fails(self):
        result = self.execute(state="running", failure="network")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("EVENT:ready:", result.stdout)

    def test_readiness_failure_fails(self):
        result = self.execute(failure="ready")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
