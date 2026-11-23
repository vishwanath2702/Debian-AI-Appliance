"""SearXNG payload packaging and installed import tests."""

import ast
import importlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
MODULES = ROOT / "runtime/modules"


class PayloadModuleTests(unittest.TestCase):

    def test_feature_gated_staging(self):
        source = BUILDER.read_text()

        gate = (
            'if [[ "${DAIA_ENABLE_SEARXNG:-false}" '
            '== "true" ]]; then'
        )

        start = source.index(gate)

        observation = source.index(
            '"$DAIA_ROOT_TARGET/searxng_observation.py"'
        )

        policy = source.index(
            '"$DAIA_ROOT_TARGET/searxng_retry_policy.py"'
        )

        self.assertLess(start, observation)
        self.assertLess(start, policy)

        # Both staging operations precede the next recorded
        # SearXNG component boundary.
        boundary = source.index(
            "record_staged_component",
            observation,
        )

        self.assertLess(observation, boundary)
        self.assertLess(policy, boundary)

    def test_source_guards(self):
        source = BUILDER.read_text()

        for name in (
            "searxng_observation.py",
            "searxng_retry_policy.py",
        ):
            self.assertIn(
                f'[[ -L "$DAIA_RUNTIME_SOURCE/$helper" ]]',
                source,
            )
            self.assertIn(
                f'"$DAIA_RUNTIME_SOURCE/modules/{name}"',
                source,
            )

    def test_permissions(self):
        source = BUILDER.read_text()

        for name in (
            "searxng_observation.py",
            "searxng_retry_policy.py",
        ):
            destination = (
                f'"$DAIA_ROOT_TARGET/{name}" \\\n'
                "            0644 || return 1"
            )

            self.assertIn(destination, source)

    def test_modules_have_valid_syntax(self):
        for name in (
            "searxng_observation.py",
            "searxng_retry_policy.py",
            "searxng_registration.py",
        ):
            ast.parse(
                (MODULES / name).read_text()
            )

    def test_installed_layout_import(self):
        with tempfile.TemporaryDirectory() as directory:
            installed = Path(directory) / "opt/daia"
            installed.mkdir(parents=True)

            for name in (
                "searxng_observation.py",
                "searxng_retry_policy.py",
                "searxng_registration.py",
            ):
                shutil.copy2(
                    MODULES / name,
                    installed / name,
                )

            script = """
import importlib
from searxng_retry_policy import Decision, Observation, classify
from searxng_observation import observe, egress_state
from searxng_registration import DEFAULT_DIRECTORY

assert callable(observe)
assert callable(egress_state)
assert DEFAULT_DIRECTORY == "/var/lib/daia/searxng"
assert classify(Observation(
    assessment="ABSENT",
    egress_state="ABSENT",
    registration_directory="TRUSTED_EMPTY",
)) == Decision.FRESH
"""

            environment = dict(os.environ)
            environment["PYTHONPATH"] = str(installed)
            environment["PYTHONDONTWRITEBYTECODE"] = "1"

            result = subprocess.run(
                [sys.executable, "-B", "-c", script],
                cwd=directory,
                env=environment,
                capture_output=True,
                text=True,
                timeout=20,
            )

            self.assertEqual(
                result.returncode,
                0,
                result.stdout + result.stderr,
            )

    def test_runtime_activation_still_absent(self):
        source = (
            ROOT / "runtime/runtime.sh"
        ).read_text()

        self.assertNotIn(
            'RUNTIME_MODULES+=("searxng")',
            source,
        )


if __name__ == "__main__":
    unittest.main()
