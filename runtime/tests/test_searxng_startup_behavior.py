"""Behavioral tests for SearXNG startup; no real Docker access."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng.sh"
CID = "a" * 64


class SearxngStartupBehaviorTests(unittest.TestCase):
    def invoke(self, scenario):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            bin_dir = directory / "bin"
            bin_dir.mkdir()
            trace = directory / "trace"

            docker = bin_dir / "docker"
            docker.write_text(r"""#!/usr/bin/env bash
set -euo pipefail
printf 'docker %s\n' "$*" >> "$TRACE"

case "${1:-}" in
    info)
        exit 0
        ;;
    network)
        if [[ "${2:-}" == "inspect" ]]; then
            if [[ "$*" == *"daia-rag-internal"* ]]; then
                echo 'daia-rag-internal|bridge|true|rag|true'
            else
                echo 'daia-searxng-egress|bridge|false|searxng|true'
            fi
            exit 0
        fi
        ;;
    container)
        if [[ "${2:-}" == "inspect" ]]; then
            case "$SCENARIO" in
                running)
                    echo 'true|false|false|running'
                    ;;
                paused)
                    echo 'false|true|false|exited'
                    ;;
                *)
                    echo 'false|false|false|exited'
                    ;;
            esac
            exit 0
        fi
        ;;
    start)
        [[ "$SCENARIO" != "start-failure" ]]
        ;;
    stop)
        case "$SCENARIO" in
            post-failure-stop-failure|post-mismatch-stop-failure)
                echo "simulated Docker stop failure" >&2
                exit 91
                ;;
        esac
        exit 0
        ;;
esac
""")
            docker.chmod(0o755)

            helper = bin_dir / "verify-registration"
            helper.write_text(r"""#!/usr/bin/env bash
set -euo pipefail
printf 'verify %s\n' "$*" >> "$TRACE"
[[ "${1:-}" == "--resolve-registered" ]] || exit 1

case "$SCENARIO" in
    mismatch)
        printf '%064d\n' 0
        ;;
    post-failure|post-failure-stop-failure)
        count="$(grep -c '^verify ' "$TRACE" || true)"
        if [[ "$count" -ge 3 ]]; then
            exit 1
        fi
        printf '%s\n' "$CID"
        ;;
    post-mismatch|post-mismatch-stop-failure)
        count="$(grep -c '^verify ' "$TRACE" || true)"
        if [[ "$count" -ge 3 ]]; then
            printf '%064d\n' 0
        else
            printf '%s\n' "$CID"
        fi
        ;;
    *)
        printf '%s\n' "$CID"
        ;;
esac
""")
            helper.chmod(0o755)

            source = MODULE.read_text()

            # These substitutions affect only the temporary test copy.
            # The production module retains its root requirement.
            root_guard = '[[ "$EUID" -eq 0 ]] || return 1'
            startup_body = source.split(
                "searxng_start_registered() {", 1
            )[1].split("\n}", 1)[0]
            self.assertEqual(startup_body.count(root_guard), 1)
            source = source.replace(root_guard, ': # mocked root boundary', 1)

            trusted_helper = (
                "/usr/libexec/daia-searxng-verify-registration"
            )
            self.assertEqual(startup_body.count(trusted_helper), 1)
            source = source.replace(trusted_helper, str(helper), 1)

            script = directory / "module.sh"
            script.write_text(source)

            env = {
                **os.environ,
                "PATH": str(bin_dir) + os.pathsep + os.environ["PATH"],
                "TRACE": str(trace),
                "SCENARIO": scenario,
                "CID": CID,
            }

            result = subprocess.run(
                [
                    "bash", "-c",
                    f'source "{script}"; searxng_start_registered "$CID"',
                ],
                env=env,
                capture_output=True,
                text=True,
                timeout=15,
            )

            events = (
                trace.read_text().splitlines()
                if trace.exists() else []
            )
            return result, events

    def test_success_starts_once(self):
        result, events = self.invoke("success")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            events.count("docker start " + CID), 1
        )
        self.assertFalse(
            any(e.startswith("docker stop ") for e in events)
        )
        self.assertEqual(
            sum(e.startswith("verify ") for e in events), 3
        )

    def test_registration_mismatch_never_starts(self):
        result, events = self.invoke("mismatch")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any(e.startswith("docker start ") for e in events)
        )

    def test_already_running_refused(self):
        result, events = self.invoke("running")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any(e.startswith("docker start ") for e in events)
        )

    def test_paused_refused(self):
        result, events = self.invoke("paused")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any(e.startswith("docker start ") for e in events)
        )

    def test_start_failure_does_not_stop(self):
        result, events = self.invoke("start-failure")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("docker start " + CID, events)
        self.assertFalse(
            any(e.startswith("docker stop ") for e in events)
        )

    def test_cleanup_failure_warnings_are_not_suppressed(self):
        source = MODULE.read_text()
        body = source.split(
            "searxng_start_registered() {", 1
        )[1].split("\n}", 1)[0]

        warning = (
            "SearXNG SECURITY WARNING: failed to stop container "
            "after identity verification failure:"
        )

        self.assertEqual(body.count(warning), 2)
        self.assertEqual(body.count("docker stop --time 10"), 2)
        self.assertNotIn(
            'docker stop --time 10 "$container_id" >/dev/null 2>&1 || true',
            body
        )

    def test_post_verification_stop_failure_warns(self):
        result, events = self.invoke("post-failure-stop-failure")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("docker start " + CID, events)
        self.assertEqual(
            [e for e in events if e.startswith("docker stop ")],
            ["docker stop --time 10 " + CID]
        )
        self.assertIn(
            "SearXNG SECURITY WARNING: failed to stop container",
            result.stderr
        )
        self.assertIn(CID, result.stderr)

    def test_post_identity_mismatch_stop_failure_warns(self):
        result, events = self.invoke("post-mismatch-stop-failure")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("docker start " + CID, events)
        self.assertEqual(
            [e for e in events if e.startswith("docker stop ")],
            ["docker stop --time 10 " + CID]
        )
        self.assertIn(
            "SearXNG SECURITY WARNING: failed to stop container",
            result.stderr
        )
        self.assertIn(CID, result.stderr)

    def test_post_verification_failure_stops_same_id(self):
        result, events = self.invoke("post-failure")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("docker start " + CID, events)
        self.assertIn("docker stop --time 10 " + CID, events)

    def test_post_identity_mismatch_stops_same_id(self):
        result, events = self.invoke("post-mismatch")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("docker start " + CID, events)
        self.assertIn("docker stop --time 10 " + CID, events)


if __name__ == "__main__":
    unittest.main()
