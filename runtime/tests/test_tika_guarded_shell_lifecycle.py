import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
MODULE = ROOT / "modules/tika.sh"
CID = "a" * 64


class TikaGuardedShellLifecycleTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(
            prefix="pragna-tika-shell-"
        )
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.events = self.directory / "events.txt"
        self.helper = self.directory / "tika-helper"

        self.helper.write_text(
            '#!/usr/bin/env python3\n'
            'import os, sys\n'
            'mode = sys.argv[1]\n'
            'with open(os.environ["EVENTS"], "a") as log:\n'
            '    log.write("helper:" + mode + "\\n")\n'
            'if mode == "--authorize-start" and '
            'os.environ.get("FAIL_AUTH") == "1":\n'
            '    sys.exit(1)\n'
            'if mode == "--verify-running" and '
            'os.environ.get("FAIL_RUNNING") == "1":\n'
            '    sys.exit(1)\n'
            'if mode == "--verify-identity" and '
            'os.environ.get("FAIL_IDENTITY") == "1":\n'
            '    sys.exit(1)\n'
            'if mode == "--verify-running" and '
            'os.environ.get("MISMATCH") == "1":\n'
            '    print("b" * 64)\n'
            'else:\n'
            '    print(os.environ["CID"])\n',
            encoding="utf-8",
        )
        self.helper.chmod(0o755)

        source = MODULE.read_text(encoding="utf-8")
        self.assertIn(
            "/usr/libexec/daia-tika-register-container", source
        )
        self.assertIn('[[ "$EUID" -ne 0 ]]', source)

        source = source.replace(
            "/usr/libexec/daia-tika-register-container",
            str(self.helper),
        ).replace(
            '[[ "$EUID" -ne 0 ]]',
            '[[ 0 -ne 0 ]]',
        )

        self.fixture = self.directory / "tika.sh"
        self.fixture.write_text(source, encoding="utf-8")

    def execute(self, command, **flags):
        script = r'''
set -euo pipefail
source "$1"

docker() {
    printf 'docker:%s\n' "$1" >> "$EVENTS"
    if [[ "$1" == "container" && "$2" == "inspect" ]]; then
        printf '%s\n' "$CID"
    fi
    return 0
}
''' + command + "\n"

        environment = {
            **os.environ,
            "CID": CID,
            "EVENTS": str(self.events),
            **flags,
        }

        result = subprocess.run(
            ["bash", "-c", script, "bash", str(self.fixture)],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
            timeout=20,
        )
        events = (
            self.events.read_text().splitlines()
            if self.events.exists() else []
        )
        return result, events

    def test_guarded_start_success(self):
        result, events = self.execute('tika_start_registered "$CID"')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertLess(
            events.index("helper:--authorize-start"),
            events.index("docker:start"),
        )
        self.assertLess(
            events.index("docker:start"),
            events.index("helper:--verify-running"),
        )
        self.assertNotIn("docker:stop", events)

    def test_authorization_failure_never_starts(self):
        result, events = self.execute(
            'tika_start_registered "$CID"', FAIL_AUTH="1"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("docker:start", events)

    def test_failed_poststart_verification_stops_trusted_id(self):
        result, events = self.execute(
            'tika_start_registered "$CID"', FAIL_RUNNING="1"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("helper:--verify-identity", events)
        self.assertIn("docker:stop", events)

    def test_untrusted_identity_prevents_cleanup_stop(self):
        result, events = self.execute(
            'tika_start_registered "$CID"',
            FAIL_RUNNING="1", FAIL_IDENTITY="1",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("docker:stop", events)

    def test_invalid_id_rejected_before_docker_start(self):
        result, events = self.execute(
            'tika_start_registered "invalid"'
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("docker:start", events)
        self.assertFalse(any(
            item.startswith("helper:") for item in events
        ))

    def test_running_verification_success(self):
        result, events = self.execute("tika_verify_registered")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("docker:container", events)
        self.assertIn("helper:--verify-running", events)
        self.assertNotIn("docker:start", events)

    def test_running_identity_mismatch_fails(self):
        result, events = self.execute(
            "tika_verify_registered", MISMATCH="1"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("docker:stop", events)

    def test_provisioning_uses_trusted_pg_authorizer(self):
        source = MODULE.read_text(encoding="utf-8")
        self.assertNotIn("sys.path.insert", source)
        self.assertIn(
            'postgresql_id="$(/usr/libexec/'
            'daia-pgvector-authorize-start runtime)"',
            source,
        )
        self.assertIn(
            'daia-rag-verify-network registration "$postgresql_id"',
            source,
        )

    def test_firstboot_uses_guarded_registered_lifecycle(self):
        source = MODULE.read_text(encoding="utf-8")
        install = source.split(
            "\ntika_install() {\n", 1
        )[1].split(
            "\ntika_verify() {\n", 1
        )[0]
        verify = source.split(
            "\ntika_verify() {\n", 1
        )[1]
        self.assertIn(
            "tika_install_registered || return 1", install
        )
        self.assertIn(
            "tika_verify_registered || return 1", verify
        )
        self.assertNotIn(
            "Tika container provisioning is not authorized yet",
            install,
        )

if __name__ == "__main__":
    unittest.main()
