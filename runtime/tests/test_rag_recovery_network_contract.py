import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RECOVERY = ROOT / "daia-rag-recover"


class RecoveryNetworkContractTests(unittest.TestCase):
    def test_runtime_verifier_receives_all_registered_identities(self):
        source = RECOVERY.read_text()

        # Initial verification must use exactly the peers that
        # were running before recovery.
        self.assertIn('auth_args=(runtime)', source)
        self.assertIn('network_args=(runtime "$pg_id")', source)
        self.assertIn('auth_args+=("$existing_webui")', source)
        self.assertIn('auth_args+=(--tika-id "$existing_tika")', source)
        self.assertIn('network_args+=("$existing_webui")', source)
        self.assertIn('network_args+=("$existing_tika")', source)
        self.assertIn(
            '/usr/libexec/daia-rag-verify-network "${network_args[@]}"',
            source,
        )

        # Complete three-peer verification remains mandatory.
        expected = (
            '/usr/libexec/daia-rag-verify-network runtime '
            '"$pg_id" "$webui_id" "$tika_id"'
        )
        normalized = source.replace("\\\n", " ")
        invocations = [
            " ".join(line.split())
            for line in normalized.splitlines()
            if line.strip().startswith(
                "/usr/libexec/daia-rag-verify-network runtime"
            )
        ]
        self.assertEqual(invocations, [expected])

        # All three runtime states are still inspected.
        for state in ("pg_state", "tika_state", "webui_state"):
            self.assertIn(f'"${state}" == "running"', source)

        # No final success before complete-network verification.
        self.assertLess(
            source.index(expected),
            source.index(
                'echo "DAIA registered RAG containers recovered"'
            ),
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
