"""Exercise production RAG recovery with isolated system mocks."""
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RECOVERY = ROOT / "daia-rag-recover"
IDS = {
    "daia-pgvector": "a" * 64,
    "daia-tika": "b" * 64,
    "open-webui": "c" * 64,
}

MOCK = r"""#!/usr/bin/env python3
import os
import sys
from pathlib import Path

args = sys.argv[1:]
name = Path(sys.argv[0]).name
log = Path(os.environ["MOCK_LOG"])
with log.open("a") as f:
    f.write(name + " " + " ".join(args) + "\n")

ids = {"daia-pgvector": "a"*64,
       "daia-tika": "b"*64,
       "open-webui": "c"*64}
failure = os.environ.get("MOCK_FAILURE", "")
if failure == "final-network" and name == "daia-rag-verify-network":
    if len(args) == 4:
        sys.exit(1)
if failure == "reject-staged-network" and name == "daia-rag-verify-network":
    if len(args) == 2:
        sys.exit(1)
if failure == "reject-staged-network" and name == "daia-pgvector-authorize-start":
    if args == ["runtime"]:
        sys.exit(1)
if failure == name or failure == name + ":" + ":".join(args[:2]):
    sys.exit(1)

if name == "docker":
    if args == ["info"]:
        sys.exit(0)
    if args[:3] == ["container", "inspect", "--format"]:
        fmt, key = args[3:5]
        if fmt == "{{.Id}}":
            print(ids.get(key, key))
        else:
            state = os.environ.get(
                "MOCK_" + key[:1].upper() + "_STATE", "stopped"
            )
            print("true|false|false|running" if state == "running"
                  else "false|false|false|exited")
        sys.exit(0)
    if args[:1] == ["exec"]:
        sys.exit(1 if failure == "pg_isready" else 0)
    sys.exit(91)

if name == "systemctl":
    if args[:2] == ["is-active", "--quiet"]:
        active = (
            os.environ.get("MOCK_PG_ACTIVE") == "1"
            or Path(os.environ["MOCK_LOG"] + ".started").exists()
        )
        sys.exit(0 if active else 3)
    if args[:1] == ["start"]:
        if failure == "systemctl-start":
            sys.exit(1)
        Path(os.environ["MOCK_LOG"] + ".started").touch()
        sys.exit(0)
    sys.exit(91)

if failure == name:
    sys.exit(1)
if name in ("daia-tika-register-container",
            "daia-openwebui-register-container"):
    print(args[1])
elif name == "daia-pgvector-authorize-start":
    print("a"*64)
sys.exit(0)
"""

class RecoveryBehaviorTests(unittest.TestCase):
    def execute(self, failure="", pg_active=False,
                pg_state="stopped", tika_state="stopped",
                web_state="stopped"):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            bin_dir = base / "bin"
            helpers = base / "helpers"
            modules = base / "modules"
            for directory in (bin_dir, helpers, modules):
                directory.mkdir()

            script = RECOVERY.read_text()
            self.assertIn(
                "readonly MODULE_ROOT=/opt/daia/modules", script
            )
            self.assertIn(
                "/var/lib/daia/firstboot-complete", script
            )
            self.assertIn("/usr/libexec/", script)
            script = script.replace(
                "readonly MODULE_ROOT=/opt/daia/modules",
                "readonly MODULE_ROOT=" + str(modules)
            ).replace(
                "/var/lib/daia/firstboot-complete",
                str(base / "firstboot-complete")
            ).replace("/usr/libexec/", str(helpers) + "/")
            script = script.replace(
                '"$EUID" -eq 0',
                '"${DAIA_TEST_EUID:-0}" -eq 0'
            )
            (base / "firstboot-complete").touch()
            (base / "recover").write_text(script)

            for name in ("docker", "systemctl"):
                path = bin_dir / name
                path.write_text(MOCK)
                path.chmod(0o755)

            for name in (
                "daia-tika-register-container",
                "daia-openwebui-register-container",
                "daia-pgvector-authorize-start",
                "daia-pgvector-verify-registration",
                "daia-rag-verify-network",
                "daia-pgvector-audit-auth.py",
                "daia-pgvector-init-extension.py",
            ):
                path = helpers / name
                path.write_text(MOCK)
                path.chmod(0o755)

            (modules / "pgvector.sh").write_text("# mock\n")
            (modules / "tika.sh").write_text(
                'tika_start_registered() { echo "tika-start $1" >> "$MOCK_LOG"; '
        '[[ "$MOCK_FAILURE" != "tika-start" ]]; }\n'
                'tika_verify_registered() { echo "tika-verify" >> "$MOCK_LOG"; }\n'
            )
            (modules / "openwebui.sh").write_text(
                'openwebui_start_registered() { '
                'echo "webui-start $1 $2" >> "$MOCK_LOG"; '
        '[[ "$MOCK_FAILURE" != "webui-start" ]]; }\n'
            )

            env = os.environ.copy()
            env.update({
                "PATH": str(bin_dir) + os.pathsep + env["PATH"],
                "MOCK_LOG": str(base / "events"),
                "MOCK_FAILURE": failure,
                "MOCK_PG_ACTIVE": "1" if pg_active else "0",
                "MOCK_A_STATE": pg_state,
                "MOCK_B_STATE": tika_state,
                "MOCK_C_STATE": web_state,
            })
            result = subprocess.run(
                ["bash", str(base / "recover")],
                env=env, text=True, capture_output=True, timeout=10
            )
            events = (base / "events").read_text().splitlines()
            return result, events

    def test_success_starts_registered_components(self):
        result, events = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("systemctl start daia-pgvector.service", events)
        self.assertTrue(any(x.startswith("tika-start ") for x in events))
        self.assertTrue(any(x.startswith("webui-start ") for x in events))

    def test_final_combined_network_failure_blocks_success(self):
        result, events = self.execute(failure="final-network")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(
            any(e.startswith("webui-start ") for e in events)
        )
        self.assertNotIn(
            "DAIA registered RAG containers recovered",
            result.stdout
        )

    def test_active_but_stopped_postgres_refused(self):
        result, events = self.execute(pg_active=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("tika-start ") for x in events))

    def test_failed_postgres_service_start_blocks_dependents(self):
        result, events = self.execute(failure="systemctl-start")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("tika-start ") for x in events))

    def test_failed_runtime_authorization_blocks_dependents(self):
        result, events = self.execute(
            failure="daia-pgvector-authorize-start"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("tika-start ") for x in events))

    def test_unhealthy_postgres_blocks_dependents(self):
        result, events = self.execute(failure="pg_isready")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("tika-start ") for x in events))

    def test_tika_start_failure_blocks_webui(self):
        result, events = self.execute(failure="tika-start")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(
            any(x.startswith("tika-start ") for x in events)
        )
        self.assertFalse(
            any(x.startswith("webui-start ") for x in events)
        )
        self.assertNotIn(
            "DAIA registered RAG containers recovered",
            result.stdout
        )

    def test_webui_start_failure_blocks_success(self):
        result, events = self.execute(failure="webui-start")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(
            any(x.startswith("tika-start ") for x in events)
        )
        self.assertTrue(
            any(x.startswith("webui-start ") for x in events)
        )
        self.assertNotIn(
            "DAIA registered RAG containers recovered",
            result.stdout
        )

    def test_warm_recovery_uses_combined_authorization(self):
        result, events = self.execute(
            failure="reject-staged-network",
            pg_active=True,
            pg_state="running",
            tika_state="running",
            web_state="running",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(
            "daia-pgvector-authorize-start runtime", events
        )
        self.assertIn(
            "daia-pgvector-authorize-start runtime "
            + "c"*64 + " --tika-id " + "b"*64,
            events,
        )
        self.assertNotIn(
            "daia-rag-verify-network runtime " + "a"*64,
            events,
        )
        self.assertIn(
            "daia-rag-verify-network runtime "
            + "a"*64 + " " + "c"*64 + " " + "b"*64,
            events,
        )
        self.assertFalse(any(
            event.startswith("tika-start ")
            or event.startswith("webui-start ")
            for event in events
        ))

    def test_mixed_state_pg_and_tika_running_fails_closed(self):
        result, events = self.execute(
            failure="reject-staged-network",
            pg_active=True,
            pg_state="running",
            tika_state="running",
            web_state="stopped",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(
            event.startswith("tika-start ")
            or event.startswith("webui-start ")
            for event in events
        ))

    def test_mixed_state_pg_and_webui_running_fails_closed(self):
        result, events = self.execute(
            failure="reject-staged-network",
            pg_active=True,
            pg_state="running",
            tika_state="stopped",
            web_state="running",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(
            event.startswith("tika-start ")
            or event.startswith("webui-start ")
            for event in events
        ))

    def test_warm_recovery_rejects_combined_network_failure(self):
        result, events = self.execute(
            failure="final-network",
            pg_active=True,
            pg_state="running",
            tika_state="running",
            web_state="running",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(
            event.startswith("tika-start ")
            or event.startswith("webui-start ")
            for event in events
        ))

    def test_running_postgres_not_restarted(self):
        result, events = self.execute(
            pg_active=True, pg_state="running"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn(
            "systemctl start daia-pgvector.service", events
        )

if __name__ == "__main__":
    unittest.main()
