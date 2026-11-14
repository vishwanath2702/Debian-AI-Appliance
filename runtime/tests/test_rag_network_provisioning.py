"""Behavioral tests for DAIA RAG network provisioning."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/pgvector.sh"


class RAGNetworkProvisioningTests(unittest.TestCase):
    def run_case(self, inventory, verification_success=True):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log = root / "docker.log"
            mock = root / "docker"

            mock.write_text("""#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >> "$DAIA_TEST_LOG"
case "$1" in
  info) exit 0 ;;
  network) case "${2:-}" in
  ls)
    printf '%s\\n' "$DAIA_TEST_NETWORKS"
    ;;
  create)
    exit 0
    ;;
  *)
    exit 99
    ;;
  esac
  ;;
  *)
    exit 99
    ;;
esac
""")
            mock.chmod(0o755)

            script = r"""
source "$DAIA_MODULE"
# Test-only override: production root enforcement remains unchanged.
# Bash EUID is readonly, so replace only the function's UID guard
# in the sourced function definition for this isolated test.
eval "$(declare -f pgvector_ensure_rag_network |
    sed 's/\[\[ "$EUID" -ne 0 \]\]/[[ 0 -ne 0 ]]/')"
function /usr/libexec/daia-rag-verify-network {
    printf 'VERIFY\n' >> "$DAIA_TEST_LOG"
    return "$DAIA_VERIFY_EXIT"
}
pgvector_ensure_rag_network
"""

            environment = os.environ.copy()
            environment.update({
                "PATH": str(root) + ":" + environment["PATH"],
                "DAIA_MODULE": str(MODULE),
                "DAIA_TEST_LOG": str(log),
                "DAIA_TEST_NETWORKS": inventory,
                "DAIA_VERIFY_EXIT": (
                    "0" if verification_success else "1"
                ),
            })

            result = subprocess.run(
                ["bash", "-c", script],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )

            events = log.read_text() if log.exists() else ""
            return result, events

    def test_creates_absent_network_and_verifies(self):
        result, events = self.run_case("bridge\nhost\nnone")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("network create", events)
        self.assertIn("--internal", events)
        self.assertIn("VERIFY", events)

    def test_accepts_existing_verified_network(self):
        result, events = self.run_case(
            "bridge\ndaia-rag-internal\nhost"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("network create", events)
        self.assertIn("VERIFY", events)

    def test_rejects_existing_untrusted_network(self):
        result, events = self.run_case(
            "daia-rag-internal", verification_success=False
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("network create", events)

    def test_rejects_failed_post_creation_verification(self):
        result, events = self.run_case(
            "bridge", verification_success=False
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("network create", events)

    def test_never_deletes_or_connects_networks(self):
        for inventory in ("bridge", "daia-rag-internal"):
            _, events = self.run_case(inventory)
            self.assertNotIn("network rm", events)
            self.assertNotIn("network connect", events)


if __name__ == "__main__":
    unittest.main()
