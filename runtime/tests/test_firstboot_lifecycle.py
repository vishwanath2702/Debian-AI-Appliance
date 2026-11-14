"""Execute production first-boot main with inert lifecycle functions."""

from pathlib import Path
import os
import subprocess
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "firstboot.sh"


class FirstbootLifecycleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        script = SOURCE.read_text()
        start = script.index("\nmain()\n")
        end = script.index('\nmain "$@"', start)
        cls.production_main = script[start:end]

    def execute(self, failure="none"):
        script = r"""
set -euo pipefail
DAIA_STATE_DIRECTORY=/var/lib/daia
events=()

record() {
    events+=("$1")
}

require_root() {
    record root
    [[ "$FAILURE" != root ]]
}

prepare_model_directory() {
    [[ "$1" == "/var/lib/daia" ]] || return 98
    record prepare
    [[ "$FAILURE" != prepare ]]
}

run_runtime() {
    record runtime
    [[ "$FAILURE" != runtime ]]
}

realize_models() {
    record models
    [[ "$FAILURE" != models ]]
}

mark_firstboot_complete() {
    record marker
    [[ "$FAILURE" != marker ]]
}

disable_firstboot_service() {
    record disable
    [[ "$FAILURE" != disable ]]
}
"""
        script += self.production_main
        script += r"""
trap 'status=$?; printf "STATUS:%s\n" "$status"; printf "EVENTS:%s\n" "${events[*]}"' EXIT
main
"""
        env = os.environ.copy()
        env["FAILURE"] = failure
        return subprocess.run(
            ["bash", "-c", script],
            env=env, capture_output=True, text=True,
            timeout=10, check=False,
        )

    def assert_case(self, failure, events):
        result = self.execute(failure)
        expected = 0 if failure == "none" else 1
        self.assertEqual(result.returncode, expected, result.stderr)
        self.assertIn("STATUS:" + str(expected), result.stdout)
        self.assertIn("EVENTS:" + " ".join(events), result.stdout)

    def test_success(self):
        self.assert_case(
            "none",
            ["root", "prepare", "runtime", "models", "marker", "disable"],
        )

    def test_root_failure(self):
        self.assert_case("root", ["root"])

    def test_provisioning_failure(self):
        self.assert_case("prepare", ["root", "prepare"])

    def test_runtime_failure(self):
        self.assert_case("runtime", ["root", "prepare", "runtime"])

    def test_model_realization_failure(self):
        self.assert_case(
            "models", ["root", "prepare", "runtime", "models"],
        )

    def test_completion_marker_failure(self):
        self.assert_case(
            "marker", ["root", "prepare", "runtime", "models", "marker"],
        )

    def test_disable_failure(self):
        self.assert_case(
            "disable",
            ["root", "prepare", "runtime", "models", "marker", "disable"],
        )


if __name__ == "__main__":
    unittest.main()
