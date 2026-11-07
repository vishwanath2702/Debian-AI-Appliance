import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/pgvector.sh"


class PgvectorVolumeProvisioningTests(unittest.TestCase):

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-volume-"
        )
        self.directory = Path(self.temp.name)
        self.log = self.directory / "docker.log"
        self.docker = self.directory / "docker"

        self.docker.write_text(r"""#!/bin/bash
set -eu
printf '%s\n' "$*" >> "$DAIA_DOCKER_LOG"

case "$1 $2" in
    "volume ls")
        [[ "${DAIA_VOLUME_INVENTORY_ERROR:-0}" == 0 ]] || exit 1
        printf '%s\n' "${DAIA_VOLUME_INVENTORY:-}"
        ;;
    "volume create")
        [[ "${DAIA_VOLUME_CREATE_ERROR:-0}" == 0 ]] || exit 1
        printf '%s\n' "${5:-}"
        ;;
    "volume inspect")
        [[ "${DAIA_VOLUME_INSPECT_ERROR:-0}" == 0 ]] || exit 1
        [[ "${DAIA_VOLUME_OWNED:-0}" == 1 ]] || exit 1

        case "$4" in
            *org.daia.component*)
                printf 'pgvector\n'
                ;;
            *org.daia.managed*)
                printf 'true\n'
                ;;
            *)
                exit 2
                ;;
        esac
        ;;
    *)
        exit 2
        ;;
esac
""")
        self.docker.chmod(0o755)

        self.env = os.environ.copy()
        self.env["PATH"] = (
            str(self.directory) + os.pathsep + self.env["PATH"]
        )
        self.env["DAIA_DOCKER_LOG"] = str(self.log)
        self.env["DAIA_PGVECTOR_VOLUME"] = "daia-pgvector-data"

    def tearDown(self):
        self.temp.cleanup()

    def invoke(self, **overrides):
        environment = self.env.copy()
        environment.update(overrides)

        return subprocess.run(
            [
                "bash", "-c",
                'source "$1"; pgvector_ensure_volume',
                "daia-test", str(MODULE),
            ],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def calls(self):
        if not self.log.exists():
            return []
        return self.log.read_text().splitlines()

    def test_creates_missing_volume_with_daia_labels(self):
        result = self.invoke(DAIA_VOLUME_OWNED="1")
        self.assertEqual(result.returncode, 0, result.stderr)

        calls = self.calls()
        creation = next(
            (c for c in calls if c.startswith("volume create")), ""
        )
        self.assertIn("org.daia.component=pgvector", creation)
        self.assertIn("org.daia.managed=true", creation)

    def test_reuses_existing_owned_volume(self):
        result = self.invoke(
            DAIA_VOLUME_INVENTORY="daia-pgvector-data",
            DAIA_VOLUME_OWNED="1",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(
            any(c.startswith("volume create") for c in self.calls())
        )

    def test_rejects_existing_unowned_volume(self):
        result = self.invoke(
            DAIA_VOLUME_INVENTORY="daia-pgvector-data",
            DAIA_VOLUME_OWNED="0",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any(c.startswith("volume create") for c in self.calls())
        )

    def test_inventory_failure_prevents_creation(self):
        result = self.invoke(DAIA_VOLUME_INVENTORY_ERROR="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any(c.startswith("volume create") for c in self.calls())
        )

    def test_creation_failure_is_reported(self):
        result = self.invoke(DAIA_VOLUME_CREATE_ERROR="1")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
