"""Isolated behavioral tests for DAIA RAG network authorization."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "runtime/daia-rag-verify-network"
PG_ID = "a" * 64
NETWORK_ID = "b" * 64


class RAGNetworkRuntimeTests(unittest.TestCase):
    def verify(self, members, attachment=NETWORK_ID,
               network_id=NETWORK_ID, mode="runtime", webui_id=None,
               webui_attachment=NETWORK_ID):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            docker = root / "docker"
            log = root / "docker.log"

            docker.write_text(r"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$DAIA_TEST_LOG"

case "${1:-}" in
  info) exit 0 ;;
  network)
    [[ "${2:-}" == inspect ]] || exit 99
    case "$*" in
      *".Containers"*)
        printf '%s\n' "$DAIA_TEST_MEMBERS"
        ;;
      *".Id"*)
        printf '%s\n' "$DAIA_TEST_NETWORK_ID"
        ;;
      *)
        printf 'daia-rag-internal\tbridge\ttrue\trag\ttrue\n'
        ;;
    esac
    ;;
  container)
    [[ "${2:-}" == inspect ]] || exit 99
    if [[ "${*: -1}" == "$DAIA_TEST_WEBUI_ID" ]]; then
      printf '%s\n' "$DAIA_TEST_WEBUI_ATTACHMENT"
    else
      printf '%s\n' "$DAIA_TEST_ATTACHMENT"
    fi
    ;;
  *) exit 99 ;;
esac
""")
            docker.chmod(0o755)

            env = os.environ.copy()
            env.update({
                "PATH": str(root) + ":" + env["PATH"],
                "DAIA_TEST_LOG": str(log),
                "DAIA_TEST_MEMBERS": members,
                "DAIA_TEST_ATTACHMENT": attachment,
                "DAIA_TEST_NETWORK_ID": network_id,
                "DAIA_TEST_WEBUI_ID": webui_id or "",
                "DAIA_TEST_WEBUI_ATTACHMENT": webui_attachment,
            })

            # Create a test-only copy with the root check bypassed.
            # Never modify the production verifier.
            verifier_copy = root / "verifier-under-test"
            verifier_source = VERIFIER.read_text()
            guard = '[[ "$EUID" -ne 0 ]]'
            if verifier_source.count(guard) != 1:
                raise AssertionError("Production root guard changed")
            verifier_copy.write_text(
                verifier_source.replace(guard, '[[ 0 -ne 0 ]]', 1)
            )

            command = [
                "bash", str(verifier_copy), mode, PG_ID,
            ]
            if webui_id is not None:
                command.append(webui_id)

            result = subprocess.run(
                command,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )

            events = log.read_text() if log.exists() else ""
            self.assertIn(
                "network inspect",
                events,
                "Verifier did not reach mocked network inspection",
            )
            return result, events

    def test_registration_accepts_empty_network(self):
        result, _ = self.verify("", mode="registration")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_registration_accepts_trusted_postgresql_member(self):
        result, _ = self.verify(PG_ID, mode="registration")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_registration_rejects_unknown_member(self):
        result, _ = self.verify("c" * 64, mode="registration")
        self.assertNotEqual(result.returncode, 0)

    def test_registration_rejects_extra_member(self):
        result, _ = self.verify(
            PG_ID + "\\n" + "c" * 64,
            mode="registration",
        )
        self.assertNotEqual(result.returncode, 0)

    def test_registration_rejects_postgresql_attachment_mismatch(self):
        result, _ = self.verify(
            PG_ID,
            attachment="c" * 64,
            mode="registration",
        )
        self.assertNotEqual(result.returncode, 0)

    def test_registration_rejects_invalid_network_identity(self):
        result, _ = self.verify(
            PG_ID,
            network_id="invalid",
            mode="registration",
        )
        self.assertNotEqual(result.returncode, 0)

    def test_prestart_accepts_empty_network(self):
        result, events = self.verify("", mode="prestart")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("network inspect", events)

    def test_prestart_rejects_existing_member(self):
        result, _ = self.verify(PG_ID, mode="prestart")
        self.assertNotEqual(result.returncode, 0)

    def test_prestart_rejects_unregistered_member(self):
        result, _ = self.verify("c" * 64, mode="prestart")
        self.assertNotEqual(result.returncode, 0)

    def test_accepts_registered_postgresql(self):
        result, _ = self.verify(PG_ID)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_unregistered_member(self):
        result, _ = self.verify("c" * 64)
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_extra_member(self):
        result, _ = self.verify(PG_ID + "\n" + "c" * 64)
        self.assertNotEqual(result.returncode, 0)

    def test_accepts_registered_postgresql_and_webui(self):
        webui_id = "d" * 64
        result, _ = self.verify(
            PG_ID + "\n" + webui_id,
            webui_id=webui_id,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_webui_network_identity_mismatch(self):
        webui_id = "d" * 64
        result, _ = self.verify(
            PG_ID + "\n" + webui_id,
            webui_id=webui_id,
            webui_attachment="c" * 64,
        )
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_unregistered_webui_member(self):
        result, _ = self.verify(
            PG_ID + "\n" + "e" * 64,
            webui_id="d" * 64,
        )
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_empty_membership(self):
        result, _ = self.verify("")
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_network_identity_mismatch(self):
        result, _ = self.verify(PG_ID, attachment="c" * 64)
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_invalid_network_identity(self):
        result, _ = self.verify(PG_ID, network_id="invalid")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
