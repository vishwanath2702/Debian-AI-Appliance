import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng.sh"

class SearxngRuntimeContractTests(unittest.TestCase):
    def run_shell(self, script, env=None):
        return subprocess.run(
            ["bash", "-c", script],
            cwd=ROOT,
            env={**os.environ, **(env or {})},
            capture_output=True,
            text=True,
            timeout=15,
        )

    def test_module_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(MODULE)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_disabled_component_rejected(self):
        result = self.run_shell(
            f'source "{MODULE}"; searxng_validate',
            {"DAIA_ENABLE_SEARXNG": "false"},
        )
        self.assertNotEqual(result.returncode, 0)

    def test_missing_archive_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_shell(
                f'source "{MODULE}"; searxng_validate',
                {
                    "DAIA_ENABLE_SEARXNG": "true",
                    "DAIA_PAYLOAD_ROOT": directory,
                    "DAIA_SEARXNG_DOCKER_IMAGE_ID": "sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d",
                    "DAIA_SEARXNG_ARCHIVE_SHA256": "fbd19b12819ae924f5b4bc52cc51e1e05fed244be29f17dfc6cb81fae2139c1b",
                },
            )
            self.assertNotEqual(result.returncode, 0)

    def test_image_identity_is_pinned(self):
        source = MODULE.read_text()
        self.assertIn("sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d", source)
        self.assertIn("fbd19b12819ae924f5b4bc52cc51e1e05fed244be29f17dfc6cb81fae2139c1b", source)

    def test_network_separation_contract(self):
        source = MODULE.read_text()
        self.assertIn("daia-rag-internal|bridge|true|rag|true", source)
        self.assertIn(
            "daia-searxng-egress|bridge|false|searxng|true",
            source,
        )

    def test_no_unguarded_container_mutation(self):
        source = MODULE.read_text()
        for forbidden in (
            "docker create ",
            "docker run ",
            "docker network create ",
            "docker network connect ",
        ):
            self.assertNotIn(forbidden, source)

        # Exactly one start, within the registered startup function.
        self.assertEqual(source.count("docker start "), 1)
        self.assertIn('docker start "$container_id"', source)
        self.assertIn("searxng_start_registered()", source)
        self.assertIn("--resolve-registered", source)
        self.assertIn('[[ "$verified_id" == "$container_id" ]]', source)

class SearxngStartupContractTests(unittest.TestCase):
    def test_startup_not_wired_into_runtime(self):
        runtime = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn("searxng_start_registered", runtime)

    def test_startup_requires_stopped_state(self):
        source = MODULE.read_text()
        self.assertIn(
            "false\\|false\\|false\\|created",
            source,
        )
        self.assertIn(
            "false\\|false\\|false\\|exited",
            source,
        )

    def test_startup_rechecks_registration(self):
        source = MODULE.read_text()
        self.assertGreaterEqual(
            source.count('--resolve-registered'), 3
        )
        self.assertIn(
            'running_id="$("$helper" --resolve-registered)"',
            source,
        )

    def test_startup_rejects_missing_identity(self):
        result = subprocess.run(
            ["bash", "-c",
             f'source "{MODULE}"; searxng_start_registered ""'],
            capture_output=True, text=True, timeout=10,
        )
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
