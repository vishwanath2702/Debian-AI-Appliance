import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RECOVERY = ROOT / "daia-rag-recover"


class RecoveryNetworkContractTests(unittest.TestCase):
    def test_runtime_verifier_receives_all_registered_identities(self):
        source = RECOVERY.read_text()
        expected = (
            '/usr/libexec/daia-rag-verify-network runtime '
            '"$pg_id" "$webui_id" "$tika_id"'
        )
        # Normalize shell line continuations before comparing commands.
        normalized = source.replace("\\\n", " ")
        invocations = [
            " ".join(line.split())
            for line in normalized.splitlines()
            if line.strip().startswith(
                "/usr/libexec/daia-rag-verify-network runtime"
            )
        ]
        staged = (
            '/usr/libexec/daia-rag-verify-network runtime "$pg_id"'
        )
        self.assertEqual(invocations, [expected, staged, expected])
        self.assertIn(
            '"$pg_state" == "running"', source
        )
        self.assertIn(
            '"$tika_state" == "running"', source
        )
        self.assertIn(
            '"$webui_state" == "running"', source
        )

    def test_identities_resolved_before_network_verification(self):
        source = RECOVERY.read_text()
        verifier = source.index(
            '/usr/libexec/daia-rag-verify-network runtime '
            '"$pg_id" "$webui_id" "$tika_id"'
        )
        for resolver in (
            'pg_id="$(resolve_id daia-pgvector)"',
            'webui_id="$(resolve_id open-webui)"',
            'tika_id="$(resolve_id daia-tika)"',
        ):
            self.assertIn(resolver, source)
            self.assertLess(source.index(resolver), verifier)


if __name__ == "__main__":
    unittest.main()
