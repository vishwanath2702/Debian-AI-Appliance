"""Mock-only SearXNG provisioning orchestration tests."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng.sh"
PROVISION = ROOT / "runtime/modules/searxng_provision.sh"
RUNTIME = ROOT / "runtime/runtime.sh"

FUNCTION = "searxng_firstboot_provision_registered() {"
ROOT_GUARD = '    [[ "$EUID" -eq 0 ]] || return 1'

MOCKS = """
searxng_validate() {
    printf 'validate\\n'
    [[ "${FAIL_STAGE:-}" != "validate" ]]
}
searxng_load_verified_offline_image() {
    printf 'load\\n'
    [[ "${FAIL_STAGE:-}" != "load" ]]
}
searxng_provision_preflight() {
    printf 'preflight\\n'
    [[ "${FAIL_STAGE:-}" != "preflight" ]]
}
searxng_provision_egress_network() {
    printf 'network\\n'
    [[ "${FAIL_STAGE:-}" != "network" ]]
}
searxng_provision_container() {
    printf 'container_register\\n'
    [[ "${FAIL_STAGE:-}" != "container_register" ]]
}
"""

STAGES = (
    "validate", "load", "preflight",
    "network", "container_register"
)

class SearxngFirstbootOrchestrationTests(unittest.TestCase):

    def execute(self, enabled="true", fail_stage="",
                missing_function="", mock_root=False):
        source = MODULE.read_text()

        if mock_root:
            self.assertEqual(source.count(FUNCTION), 1)
            before, after = source.split(FUNCTION, 1)
            self.assertTrue(
                after.startswith("\\n" + ROOT_GUARD + "\\n")
                or after.startswith("\n" + ROOT_GUARD + "\n")
            )

            if after.startswith("\n" + ROOT_GUARD + "\n"):
                after = after.replace(
                    "\n" + ROOT_GUARD + "\n",
                    "\n    : # TEST-ONLY root guard substitution\n",
                    1,
                )
            else:
                raise AssertionError(
                    "Unexpected root-guard formatting"
                )

            source = before + FUNCTION + after

        with tempfile.TemporaryDirectory(
            prefix="daia-searxng-test-"
        ) as directory:
            temporary_module = Path(directory) / "searxng.sh"
            temporary_module.write_text(source)

            script = (
                'source "$SEARXNG_MODULE"\n'
                + MOCKS
                + "\n"
                + (
                    f"unset -f {missing_function}\n"
                    if missing_function else ""
                )
                + "searxng_firstboot_provision_registered\n"
            )

            env = os.environ.copy()
            env.update({
                "SEARXNG_MODULE": str(temporary_module),
                "DAIA_ENABLE_SEARXNG": enabled,
                "FAIL_STAGE": fail_stage,
            })

            return subprocess.run(
                ["bash", "-c", script],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
            )

    def test_disabled_feature(self):
        result = self.execute(
            enabled="false", mock_root=True
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_missing_function(self):
        result = self.execute(
            missing_function="searxng_provision_container",
            mock_root=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_runtime_not_wired(self):
        self.assertNotIn(
            'RUNTIME_MODULES+=("searxng")',
            RUNTIME.read_text(),
        )

    def test_registration_contract(self):
        self.assertIn(
            "/usr/libexec/daia-searxng-publish-registration",
            PROVISION.read_text(),
        )

    def test_no_startup_in_orchestrator(self):
        section = MODULE.read_text().split(
            FUNCTION, 1
        )[1]
        self.assertNotIn(
            "searxng_start_registered", section
        )
        self.assertNotIn(
            "searxng_wait_ready", section
        )
        self.assertNotIn("docker start", section)

    def test_fail_closed_stages(self):
        for index, stage in enumerate(STAGES):
            with self.subTest(stage=stage):
                result = self.execute(
                    fail_stage=stage, mock_root=True
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(
                    result.stdout.splitlines(),
                    list(STAGES[:index + 1]),
                )

    def test_successful_order(self):
        result = self.execute(mock_root=True)
        self.assertEqual(
            result.returncode, 0, result.stderr
        )
        self.assertEqual(
            result.stdout.splitlines(), list(STAGES)
        )

    def test_real_nonroot_guard(self):
        if os.geteuid() == 0:
            self.skipTest(
                "Real nonroot guard requires nonroot execution"
            )

        result = self.execute(mock_root=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

if __name__ == "__main__":
    unittest.main()
