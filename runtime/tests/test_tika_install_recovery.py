import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
MODULE = ROOT / "modules/tika.sh"
CID = "a" * 64


class TikaInstallRecoveryTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(
            prefix="pragna-tika-recovery-"
        )
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.events = self.directory / "events.txt"

        self.helper = self.directory / "tika-helper"
        self.helper.write_text(
            '#!/usr/bin/env python3\n'
            'import os, sys\n'
            'with open(os.environ["EVENTS"], "a") as log:\n'
            '    log.write("helper:" + sys.argv[1] + "\\n")\n'
            'if os.environ.get("UNTRUSTED") == "1":\n'
            '    sys.exit(1)\n'
            'if os.environ.get("MISMATCH") == "1":\n'
            '    print("b" * 64)\n'
            'else:\n'
            '    print(os.environ["CID"])\n',
            encoding="utf-8",
        )
        self.helper.chmod(0o755)

        original = MODULE.read_text(encoding="utf-8")
        self.assertIn(
            "/usr/libexec/daia-tika-register-container",
            original,
        )
        self.assertIn('[[ "$EUID" -ne 0 ]]', original)

        fixture = original.replace(
            "/usr/libexec/daia-tika-register-container",
            str(self.helper),
        ).replace(
            '[[ "$EUID" -ne 0 ]]',
            '[[ 0 -ne 0 ]]',
        )

        self.fixture = self.directory / "module.sh"
        self.fixture.write_text(fixture, encoding="utf-8")

    def execute(self, state="absent", **flags):
        script = r'''
set -euo pipefail
source "$1"

tika_validate() {
    return 0
}

docker() {
    if [[ "$1" == "info" ]]; then
        return 0
    fi

    if [[ "$1" == "container" && "$2" == "inspect" ]]; then
        if [[ "${3:-}" != "--format" ]]; then
            [[ "$STATE" != "absent" ]]
            return
        fi

        if [[ "$4" == "{{.Id}}" ]]; then
            printf '%s\n' "$CID"
        else
            printf '%s\n' "$STATE"
        fi
        return 0
    fi

    return 1
}

tika_provision_container() {
    printf 'provision\n' >> "$EVENTS"
    [[ "${FAIL_PROVISION:-0}" != "1" ]]
}

tika_start_registered() {
    printf 'start:%s\n' "$1" >> "$EVENTS"
    [[ "${FAIL_START:-0}" != "1" ]]
}

tika_verify_registered() {
    printf 'verify\n' >> "$EVENTS"
    [[ "${FAIL_VERIFY:-0}" != "1" ]]
}

tika_install_registered
'''

        environment = {
            **os.environ,
            "CID": CID,
            "STATE": state,
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

    def test_new_container_provisions_then_starts(self):
        result, events = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(events, [
            "provision",
            "helper:--verify-identity",
            "start:" + CID,
            "verify",
        ])

    def test_existing_running_container_only_verified(self):
        result, events = self.execute(
            "true|false|false|running"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(events, [
            "helper:--verify-identity",
            "verify",
        ])

    def test_existing_created_container_resumes(self):
        result, events = self.execute(
            "false|false|false|created"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(events, [
            "helper:--verify-identity",
            "start:" + CID,
            "verify",
        ])

    def test_existing_exited_container_resumes(self):
        result, events = self.execute(
            "false|false|false|exited"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("start:" + CID, events)
        self.assertNotIn("provision", events)

    def test_paused_container_is_rejected(self):
        result, events = self.execute(
            "true|true|false|paused"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("provision", events)
        self.assertFalse(any(
            item.startswith("start:") for item in events
        ))

    def test_untrusted_existing_container_is_rejected(self):
        result, events = self.execute(
            "false|false|false|created",
            UNTRUSTED="1",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("provision", events)
        self.assertFalse(any(
            item.startswith("start:") for item in events
        ))

    def test_identity_mismatch_rejected(self):
        result, events = self.execute(
            "false|false|false|exited",
            MISMATCH="1",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(
            item.startswith("start:") for item in events
        ))

    def test_failed_provision_never_starts(self):
        result, events = self.execute(
            FAIL_PROVISION="1"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(events, ["provision"])

    def test_failed_start_never_reports_success(self):
        result, events = self.execute(
            "false|false|false|created",
            FAIL_START="1",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("verify", events)

    def test_firstboot_delegates_to_registered_lifecycle(self):
        original = MODULE.read_text(encoding="utf-8")
        install = original.split(
            "\ntika_install() {\n", 1
        )[1].split(
            "\ntika_verify() {\n", 1
        )[0]
        verify = original.split(
            "\ntika_verify() {\n", 1
        )[1]
        self.assertIn("tika_install_registered || return 1", install)
        self.assertIn("tika_verify_registered || return 1", verify)
        self.assertNotIn(
            "Tika container provisioning is not authorized yet",
            install,
        )


if __name__ == "__main__":
    unittest.main()
