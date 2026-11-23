
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng_provision.sh"
IMAGE = "sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d"
ROOT_GUARD = '[[ "$EUID" -eq 0 ]] || return 1'

FAKE_DOCKER = r"""#!/usr/bin/env bash
printf '%s\n' "$*" >> "$MOCK_LOG"

if [[ "$1" == "info" && "$#" -eq 1 ]]; then
    exit 0
fi

case "$1 $2" in
    "image inspect")
        printf '%s\n' "$DAIA_SEARXNG_DOCKER_IMAGE_ID"
        ;;
    "container ls")
        [[ "${MOCK_INVENTORY_FAIL:-0}" != 1 ]] || exit 42
        [[ "${MOCK_EXISTING_CONTAINER:-0}" != 1 ]] || echo daia-searxng
        ;;
    "network ls")
        [[ "${MOCK_INVENTORY_FAIL:-0}" != 1 ]] || exit 42
        [[ "${MOCK_EXISTING_NETWORK:-0}" != 1 ]] || echo daia-searxng-egress
        ;;
    "network create")
        [[ "${MOCK_CREATE_FAIL:-0}" != 1 ]] || exit 43
        ;;
    "network inspect")
        if [[ "$*" == *daia-rag-internal* ]]; then
            if [[ "${MOCK_BAD_INTERNAL:-0}" == 1 ]]; then
                echo "foreign|bridge|false|rag|true"
            else
                echo "daia-rag-internal|bridge|true|rag|true"
            fi
        elif [[ "$*" == *daia-searxng-egress* ]]; then
            if [[ "$*" == *".Containers"* ]]; then
                [[ "${MOCK_MEMBERS:-0}" != 1 ]] || echo foreign-container
            elif [[ "${MOCK_BAD_EGRESS:-0}" == 1 ]]; then
                echo "foreign|bridge|false|searxng|true"
            else
                echo "daia-searxng-egress|bridge|false|searxng|true"
            fi
        else
            exit 44
        fi
        ;;
    *)
        echo "UNEXPECTED_DOCKER_COMMAND: $*" >&2
        exit 99
        ;;
esac
"""

class ProvisioningTests(unittest.TestCase):
    def run_shell(self, expression, overrides=None):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            fake = directory / "docker"
            fake.write_text(FAKE_DOCKER)
            fake.chmod(0o700)

            source = MODULE.read_text()
            self.assertEqual(source.count(ROOT_GUARD), 1)

            test_module = directory / "test-provision.sh"
            test_module.write_text(
                source.replace(ROOT_GUARD, "true # temporary test copy")
            )

            logfile = directory / "docker.log"
            env = dict(os.environ)
            env.update({
                "PATH": str(directory) + os.pathsep + env.get("PATH", ""),
                "MOCK_LOG": str(logfile),
                "DAIA_ENABLE_SEARXNG": "true",
                "DAIA_SEARXNG_DOCKER_IMAGE_ID": IMAGE,
            })
            env.update(overrides or {})

            result = subprocess.run(
                ["bash", "-c", f'source "{test_module}"; {expression}'],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
            )

            return result, logfile.read_text() if logfile.exists() else ""

    def test_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(MODULE)],
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_production_root_guard_preserved(self):
        self.assertEqual(MODULE.read_text().count(ROOT_GUARD), 1)

    def test_pinned_image(self):
        self.assertIn(IMAGE, MODULE.read_text())

    def test_preflight_success(self):
        result, log = self.run_shell("searxng_provision_preflight")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("network create", log)

    def test_existing_container_rejected(self):
        result, log = self.run_shell(
            "searxng_provision_preflight",
            {"MOCK_EXISTING_CONTAINER": "1"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("network create", log)

    def test_inventory_failure_rejected(self):
        result, log = self.run_shell(
            "searxng_provision_egress_network",
            {"MOCK_INVENTORY_FAIL": "1"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("network create", log)

    def test_internal_network_rejected(self):
        result, log = self.run_shell(
            "searxng_provision_egress_network",
            {"MOCK_BAD_INTERNAL": "1"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("network create", log)

    def test_existing_egress_network_rejected(self):
        result, log = self.run_shell(
            "searxng_provision_egress_network",
            {"MOCK_EXISTING_NETWORK": "1"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("network create", log)

    def test_successful_network_creation(self):
        result, log = self.run_shell("searxng_provision_egress_network")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(log.count("network create "), 1)
        self.assertIn("--driver bridge", log)
        self.assertIn("org.daia.component=searxng", log)
        self.assertIn("org.daia.managed=true", log)
        self.assertNotIn("--internal", log)
        self.assertIn("network inspect", log)

    def test_network_creation_failure(self):
        result, log = self.run_shell(
            "searxng_provision_egress_network",
            {"MOCK_CREATE_FAIL": "1"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(log.count("network create "), 1)

    def test_created_network_metadata_rejected(self):
        result, log = self.run_shell(
            "searxng_provision_egress_network",
            {"MOCK_BAD_EGRESS": "1"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(log.count("network create "), 1)

    def test_unexpected_network_members_rejected(self):
        result, log = self.run_shell(
            "searxng_provision_egress_network",
            {"MOCK_MEMBERS": "1"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(log.count("network create "), 1)

    def test_disabled_component_rejected(self):
        result, log = self.run_shell(
            "searxng_provision_egress_network",
            {"DAIA_ENABLE_SEARXNG": "false"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("network create", log)

    def test_container_provisioning_not_automatically_invoked(self):
        runtime = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn(
            "searxng_provision_container",
            runtime,
        )
        self.assertIn(
            "searxng_provision_preflight || return 1",
            MODULE.read_text(),
        )

    def test_no_start_or_destructive_cleanup(self):
        source = MODULE.read_text()

        self.assertIn("docker container create", source)
        self.assertIn("docker network connect", source)

        for forbidden in (
            "docker container start",
            "docker container rm",
            "docker network rm ",
            "docker rm ",
            "docker start ",
        ):
            self.assertNotIn(forbidden, source)

    def test_not_wired_into_runtime(self):
        runtime = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn("searxng_provision_egress_network", runtime)
        self.assertNotIn("searxng_provision_container", runtime)

if __name__ == "__main__":
    unittest.main()
