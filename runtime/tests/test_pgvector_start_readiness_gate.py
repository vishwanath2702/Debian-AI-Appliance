import subprocess
import unittest
from pathlib import Path

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-pgvector-start"
)

class PgvectorStartupReadinessTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.script = SOURCE.read_text(encoding="utf-8")
        start = cls.script.index(
            "wait_for_pgvector_ready() {"
        )
        end = cls.script.index("\n}\n", start) + 2
        cls.function = cls.script[start:end]

    def exercise(self, success_after):
        shell = (
            self.function
            + "\n"
            + 'container_id="' + ("a" * 64) + '"\n'
            + "count=0\n"
            + f"success_after={success_after}\n"
            + """
docker() {
    count=$((count + 1))
    [[ "$*" == "exec $container_id pg_isready -h 127.0.0.1 -p 5432 -U daia_rag -d daia_rag" ]] || return 97
    [[ "$count" -ge "$success_after" ]]
}
sleep() { :; }
if wait_for_pgvector_ready "$container_id"; then
    outcome=success
else
    outcome=failure
fi
printf '%s:%s' "$outcome" "$count"
"""
        )
        result = subprocess.run(
            ["bash", "-c", shell],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        self.assertEqual(
            result.returncode, 0, result.stderr
        )
        return result.stdout

    def test_immediate_readiness(self):
        self.assertEqual(
            self.exercise(1), "success:1"
        )

    def test_delayed_readiness(self):
        self.assertEqual(
            self.exercise(3), "success:3"
        )

    def test_readiness_timeout_is_bounded(self):
        self.assertEqual(
            self.exercise(100), "failure:20"
        )

    def test_security_gate_order(self):
        script = self.script
        network = script.index(
            "daia-rag-verify-network runtime"
        )
        readiness = script.index(
            'elif ! wait_for_pgvector_ready "$container_id"'
        )
        audit = script.index(
            'elif ! /usr/libexec/daia-pgvector-audit-auth.py'
        )
        extension = script.index(
            'elif ! /usr/libexec/daia-pgvector-init-extension.py'
        )
        cleanup = script.index(
            'daia-pgvector-verify-registration "$container_id"',
            extension,
        )
        self.assertLess(
            network, readiness
        )
        self.assertLess(
            readiness, audit
        )
        self.assertLess(
            audit, extension
        )
        self.assertLess(
            extension, cleanup
        )

if __name__ == "__main__":
    unittest.main()
