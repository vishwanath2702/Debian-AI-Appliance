import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng.sh"
CID = "a" * 64

class ReadinessTests(unittest.TestCase):
    def invoke(self, scenario, seconds="3"):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            bin_dir = base / "bin"
            bin_dir.mkdir()
            trace = base / "trace"

            docker = bin_dir / "docker"
            docker.write_text("""#!/bin/bash
echo "docker $*" >> "$TRACE"
[[ "$1" == "exec" ]] || exit 90
if [[ "$SCENARIO" == "slow-probe" ]]; then
    sleep 3
    exit 0
fi
[[ "$SCENARIO" == "healthy" ]]
""")
            docker.chmod(0o755)

            helper = bin_dir / "verify"
            helper.write_text("""#!/bin/bash
echo "verify $*" >> "$TRACE"
if [[ "$SCENARIO" == "slow-registration" ]]; then
    sleep 3
fi
if [[ "$SCENARIO" == "mismatch" ]]; then
    printf '%064d\\n' 0
else
    echo "$CID"
fi
""")
            helper.chmod(0o755)

            source = MODULE.read_text()
            prefix, body = source.split("searxng_wait_ready() {", 1)

            guard = '[[ "$EUID" -eq 0 ]] || return 1'
            self.assertEqual(body.split("\n}", 1)[0].count(guard), 1)

            body = body.replace(guard, ":", 1)
            body = body.replace(
                "/usr/libexec/daia-searxng-verify-registration",
                str(helper), 1
            )

            script = base / "module.sh"
            script.write_text(prefix + "searxng_wait_ready() {" + body)

            env = {
                **os.environ,
                "PATH": str(bin_dir) + ":" + os.environ["PATH"],
                "TRACE": str(trace),
                "CID": CID,
                "SCENARIO": scenario,
            }

            result = subprocess.run(
                ["bash", "-c",
                 f'source "{script}"; searxng_wait_ready "$CID" "{seconds}"'],
                env=env, capture_output=True, text=True, timeout=12
            )

            events = trace.read_text().splitlines() if trace.exists() else []
            return result, events

    def test_healthy(self):
        result, events = self.invoke("healthy")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any(x.startswith("docker exec ") for x in events))

    def test_mismatch(self):
        result, events = self.invoke("mismatch")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("docker exec ") for x in events))

    def test_deadline(self):
        result, events = self.invoke("unhealthy", "2")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("deadline exceeded", result.stderr)

    def test_invalid_timeout(self):
        result, events = self.invoke("healthy", "999")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("docker exec ") for x in events))

    def test_registration_helper_is_bounded(self):
        body = MODULE.read_text().split(
            "searxng_wait_ready() {", 1
        )[1].split("\n}", 1)[0]
        self.assertEqual(
            body.count('"${helper_seconds}s" "$helper" --resolve-registered'),
            2
        )

    def test_health_response_body_checked(self):
        body = MODULE.read_text().split(
            "searxng_wait_ready() {", 1
        )[1].split("\n}", 1)[0]
        self.assertIn('r.read(3) == b"OK"', body)

    def test_post_probe_deadline_rechecked(self):
        body = MODULE.read_text().split(
            "searxng_wait_ready() {", 1
        )[1].split("\n}", 1)[0]
        self.assertIn(
            '(( now - started_at < timeout_seconds )) || return 1',
            body
        )

    def test_monotonic_clock_source(self):
        body = MODULE.read_text().split(
            "searxng_wait_ready() {", 1
        )[1].split("\n}", 1)[0]

        self.assertEqual(body.count("date +%s"), 0)
        self.assertEqual(
            body.count("read -r uptime_seconds _ < /proc/uptime"),
            6
        )
        self.assertIn("[[ -r /proc/uptime ]]", body)

    def test_monotonic_clock_preserves_security_checks(self):
        body = MODULE.read_text().split(
            "searxng_wait_ready() {", 1
        )[1].split("\n}", 1)[0]

        self.assertEqual(
            body.count('"$helper" --resolve-registered'),
            2
        )
        self.assertIn('r.read(3) == b"OK"', body)
        self.assertIn(
            '(( now - started_at < timeout_seconds )) || return 1',
            body
        )

    def test_slow_registration_is_bounded(self):
        import time

        started = time.monotonic()
        result, events = self.invoke("slow-registration", "1")
        elapsed = time.monotonic() - started

        self.assertNotEqual(result.returncode, 0)
        self.assertGreater(elapsed, 0)
        self.assertLess(elapsed, 5)
        self.assertTrue(
            any(event.startswith("verify ") for event in events)
        )
        self.assertFalse(
            any(event.startswith("docker exec ") for event in events)
        )

    def test_slow_health_probe_is_bounded(self):
        import time

        started = time.monotonic()
        result, events = self.invoke("slow-probe", "1")
        elapsed = time.monotonic() - started

        self.assertNotEqual(result.returncode, 0)
        self.assertGreater(elapsed, 0)
        self.assertLess(elapsed, 5)
        self.assertTrue(
            any(event.startswith("verify ") for event in events)
        )
        self.assertTrue(
            any(event.startswith("docker exec ") for event in events)
        )

    def test_no_mutation(self):
        body = MODULE.read_text().split(
            "searxng_wait_ready() {", 1
        )[1].split("\n}", 1)[0]
        self.assertIn("timeout --signal=TERM --kill-after=1", body)
        self.assertNotIn("docker start ", body)
        self.assertNotIn("docker stop ", body)

if __name__ == "__main__":
    unittest.main()
