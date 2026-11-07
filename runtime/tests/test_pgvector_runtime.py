"""Regression tests for DAIA pgvector runtime lifecycle."""

from pathlib import Path
import subprocess
import unittest


MODULE = (
    Path(__file__).resolve().parents[1]
    / "modules"
    / "pgvector.sh"
)


def run_module(operation):
    """Execute a module function without invoking the DAIA installer."""
    if operation not in ("validate", "install", "verify"):
        raise ValueError("Unsupported test operation")

    script = (
        'set -euo pipefail\n'
        'source "$1"\n'
        'pgvector_' + operation + '\n'
    )

    return subprocess.run(
        ["bash", "-c", script, "daia-test", str(MODULE)],
        capture_output=True,
        text=True,
        timeout=10,
    )


class PgvectorRuntimeContractTests(unittest.TestCase):

    def test_module_has_required_functions(self):
        result = subprocess.run(
            [
                "bash",
                "-c",
                'source "$1"; '
                'declare -F pgvector_validate; '
                'declare -F pgvector_install; '
                'declare -F pgvector_verify',
                "daia-test",
                str(MODULE),
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_install_fails_closed_without_root(self):
        import os

        if os.geteuid() == 0:
            self.skipTest("Non-root guard requires non-root execution")

        result = run_module("install")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "PostgreSQL provisioning requires root",
            result.stderr,
        )

    def test_verify_fails_closed_while_unimplemented(self):
        result = run_module("verify")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "not implemented yet",
            result.stderr,
        )


class PgvectorRuntimeValidationTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-validation-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.images = self.root / "images"
        self.images.mkdir()

        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.image = self.images / "pgvector.tar"

        self.environment = os.environ.copy()
        self.environment["DAIA_PAYLOAD_ROOT"] = str(self.root)
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )

    def make_mock_docker(self, exit_code=0):
        docker = self.bin / "docker"

        docker.write_text(
            "#!/bin/sh\n"
            "if [ \"$1\" = info ]; then\n"
            f"    exit {exit_code}\n"
            "fi\n"
            "exit 99\n"
        )

        docker.chmod(0o755)

    def validate(self):
        return subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; '
                'source "$1"; pgvector_validate',
                "daia-test",
                str(MODULE),
            ],
            env=self.environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_rejects_missing_image(self):
        self.make_mock_docker()

        result = self.validate()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "image is missing or empty",
            result.stderr,
        )

    def test_rejects_empty_image(self):
        self.image.touch()
        self.make_mock_docker()

        result = self.validate()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "image is missing or empty",
            result.stderr,
        )

    def test_rejects_unavailable_docker_daemon(self):
        self.image.write_bytes(b"test-image-placeholder")
        self.make_mock_docker(exit_code=1)

        result = self.validate()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "Docker daemon is not available",
            result.stderr,
        )

    def test_accepts_image_and_available_docker(self):
        self.image.write_bytes(b"test-image-placeholder")
        self.make_mock_docker()

        result = self.validate()

        self.assertEqual(
            result.returncode,
            0,
            result.stderr,
        )


class PgvectorContainerDiscoveryTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-discovery-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.log = self.root / "docker.log"

        self.environment = os.environ.copy()
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )
        self.environment["DAIA_TEST_DOCKER_LOG"] = str(self.log)

    def mock_docker(self, exit_code):
        docker = self.bin / "docker"

        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            f"exit {exit_code}\n"
        )
        docker.chmod(0o755)

    def discover(self, name="daia-pgvector"):
        environment = self.environment.copy()
        environment["DAIA_PGVECTOR_CONTAINER"] = name

        return subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; '
                'source "$1"; pgvector_container_exists',
                "daia-test",
                str(MODULE),
            ],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_detects_existing_container(self):
        self.mock_docker(0)

        result = self.discover()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.log.read_text().strip(),
            "container inspect daia-pgvector",
        )

    def test_reports_missing_container(self):
        self.mock_docker(1)

        result = self.discover()

        self.assertEqual(result.returncode, 1)
        self.assertEqual(
            self.log.read_text().strip(),
            "container inspect daia-pgvector",
        )

    def test_rejects_unsafe_container_name(self):
        self.mock_docker(0)

        result = self.discover("../unsafe")

        self.assertEqual(result.returncode, 2)
        self.assertIn(
            "Invalid pgvector container name",
            result.stderr,
        )
        self.assertFalse(self.log.exists())


class PgvectorContainerStatusTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-status-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.log = self.root / "docker.log"

        self.environment = os.environ.copy()
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )
        self.environment["DAIA_TEST_DOCKER_LOG"] = str(self.log)

    def mock_docker(self, inventory, exit_code=0):
        docker = self.bin / "docker"
        inventory_file = self.root / "inventory.txt"
        inventory_file.write_text(inventory + "\n")

        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            'cat "$DAIA_TEST_INVENTORY_FILE"\n'
            f"exit {exit_code}\n"
        )

        docker.chmod(0o755)
        self.environment["DAIA_TEST_INVENTORY_FILE"] = str(
            inventory_file
        )

    def status(self, name="daia-pgvector"):
        environment = self.environment.copy()
        environment["DAIA_PGVECTOR_CONTAINER"] = name

        return subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; '
                'source "$1"; pgvector_container_status',
                "daia-test",
                str(MODULE),
            ],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_existing_container(self):
        self.mock_docker("other-container\ndaia-pgvector")

        result = self.status()

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_missing_container(self):
        self.mock_docker("other-container")

        result = self.status()

        self.assertEqual(result.returncode, 1)

    def test_docker_inventory_failure(self):
        self.mock_docker("", exit_code=1)

        result = self.status()

        self.assertEqual(result.returncode, 2)
        self.assertIn(
            "Unable to inspect Docker container inventory",
            result.stderr,
        )

    def test_invalid_container_name(self):
        self.mock_docker("daia-pgvector")

        result = self.status("../unsafe")

        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())


class PgvectorContainerOwnershipTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-ownership-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.log = self.root / "docker.log"

        self.environment = os.environ.copy()
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )
        self.environment["DAIA_TEST_DOCKER_LOG"] = str(self.log)

    def mock_docker(
        self,
        component="pgvector",
        managed="true",
        exit_code=0,
    ):
        component_file = self.root / "component.txt"
        managed_file = self.root / "managed.txt"

        component_file.write_text(component + "\n")
        managed_file.write_text(managed + "\n")

        docker = self.bin / "docker"

        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            'case "$*" in\n'
            '  *org.daia.component*)\n'
            '    cat "$DAIA_TEST_COMPONENT_FILE"\n'
            '    ;;\n'
            '  *org.daia.managed*)\n'
            '    cat "$DAIA_TEST_MANAGED_FILE"\n'
            '    ;;\n'
            '  *) exit 99 ;;\n'
            'esac\n'
            f"exit {exit_code}\n"
        )

        docker.chmod(0o755)

        self.environment["DAIA_TEST_COMPONENT_FILE"] = str(
            component_file
        )
        self.environment["DAIA_TEST_MANAGED_FILE"] = str(
            managed_file
        )

    def check_ownership(self, name="daia-pgvector"):
        environment = self.environment.copy()
        environment["DAIA_PGVECTOR_CONTAINER"] = name

        return subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; '
                'source "$1"; pgvector_container_owned',
                "daia-test",
                str(MODULE),
            ],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_accepts_matching_ownership_labels(self):
        self.mock_docker()

        result = self.check_ownership()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            len(self.log.read_text().splitlines()),
            2,
        )

    def test_rejects_wrong_component_label(self):
        self.mock_docker(component="openwebui")

        result = self.check_ownership()

        self.assertEqual(result.returncode, 1)

    def test_rejects_wrong_management_label(self):
        self.mock_docker(managed="false")

        result = self.check_ownership()

        self.assertEqual(result.returncode, 1)

    def test_rejects_missing_labels(self):
        self.mock_docker(component="", managed="")

        result = self.check_ownership()

        self.assertEqual(result.returncode, 1)

    def test_fails_closed_on_docker_error(self):
        self.mock_docker(exit_code=1)

        result = self.check_ownership()

        self.assertEqual(result.returncode, 2)
        self.assertIn(
            "Unable to inspect pgvector",
            result.stderr,
        )

    def test_rejects_unsafe_container_name_without_docker(self):
        self.mock_docker()

        result = self.check_ownership("../unsafe")

        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())


class PgvectorContainerImageTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-image-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.log = self.root / "docker.log"
        self.image_file = self.root / "image.txt"

        self.environment = os.environ.copy()
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )
        self.environment["DAIA_TEST_DOCKER_LOG"] = str(self.log)
        self.environment["DAIA_TEST_IMAGE_FILE"] = str(
            self.image_file
        )

    def mock_docker(
        self,
        image="pgvector/pgvector:pg17",
        exit_code=0,
    ):
        self.image_file.write_text(image + "\n")

        docker = self.bin / "docker"

        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            'cat "$DAIA_TEST_IMAGE_FILE"\n'
            f"exit {exit_code}\n"
        )

        docker.chmod(0o755)

    def check_image(
        self,
        name="daia-pgvector",
        expected="pgvector/pgvector:pg17",
    ):
        environment = self.environment.copy()
        environment["DAIA_PGVECTOR_CONTAINER"] = name
        environment["DAIA_PGVECTOR_IMAGE_REF"] = expected

        return subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; '
                'source "$1"; pgvector_container_image_matches',
                "daia-test",
                str(MODULE),
            ],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_accepts_matching_image(self):
        self.mock_docker()

        result = self.check_image()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            len(self.log.read_text().splitlines()),
            1,
        )

    def test_rejects_mismatched_image(self):
        self.mock_docker(image="postgres:17")

        result = self.check_image()

        self.assertEqual(result.returncode, 1)

    def test_fails_closed_on_docker_error(self):
        self.mock_docker(exit_code=1)

        result = self.check_image()

        self.assertEqual(result.returncode, 2)
        self.assertIn(
            "Unable to inspect",
            result.stderr,
        )

    def test_rejects_unsafe_container_name(self):
        self.mock_docker()

        result = self.check_image(name="../unsafe")

        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())

    def test_rejects_unexpected_expected_image(self):
        self.mock_docker()

        result = self.check_image(expected="postgres:17")

        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())


class PgvectorVolumeOwnershipTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-volume-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.log = self.root / "docker.log"

        self.environment = os.environ.copy()
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )
        self.environment["DAIA_TEST_DOCKER_LOG"] = str(self.log)

    def mock_docker(
        self,
        component="pgvector",
        managed="true",
        exit_code=0,
    ):
        component_file = self.root / "component.txt"
        managed_file = self.root / "managed.txt"

        component_file.write_text(component + "\n")
        managed_file.write_text(managed + "\n")

        docker = self.bin / "docker"

        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            'case "$*" in\n'
            '  *org.daia.component*)\n'
            '    cat "$DAIA_TEST_COMPONENT_FILE"\n'
            '    ;;\n'
            '  *org.daia.managed*)\n'
            '    cat "$DAIA_TEST_MANAGED_FILE"\n'
            '    ;;\n'
            '  *) exit 99 ;;\n'
            'esac\n'
            f"exit {exit_code}\n"
        )

        docker.chmod(0o755)

        self.environment["DAIA_TEST_COMPONENT_FILE"] = str(
            component_file
        )
        self.environment["DAIA_TEST_MANAGED_FILE"] = str(
            managed_file
        )

    def check_volume(self, name="daia-pgvector-data"):
        environment = self.environment.copy()
        environment["DAIA_PGVECTOR_VOLUME"] = name

        return subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; '
                'source "$1"; pgvector_volume_owned',
                "daia-test",
                str(MODULE),
            ],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_accepts_matching_ownership_labels(self):
        self.mock_docker()

        result = self.check_volume()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            len(self.log.read_text().splitlines()),
            2,
        )

    def test_rejects_wrong_component_label(self):
        self.mock_docker(component="openwebui")

        result = self.check_volume()

        self.assertEqual(result.returncode, 1)

    def test_rejects_wrong_management_label(self):
        self.mock_docker(managed="false")

        result = self.check_volume()

        self.assertEqual(result.returncode, 1)

    def test_rejects_missing_labels(self):
        self.mock_docker(component="", managed="")

        result = self.check_volume()

        self.assertEqual(result.returncode, 1)

    def test_fails_closed_on_docker_error(self):
        self.mock_docker(exit_code=1)

        result = self.check_volume()

        self.assertEqual(result.returncode, 2)
        self.assertIn(
            "Unable to inspect pgvector volume",
            result.stderr,
        )

    def test_rejects_unsafe_volume_name_without_docker(self):
        self.mock_docker()

        result = self.check_volume("../unsafe")

        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())


class PgvectorContainerMountTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-mount-"
        )
        self.addCleanup(self.temp.cleanup)

        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()

        self.log = self.root / "docker.log"
        self.mount_file = self.root / "mounts.txt"

        self.environment = os.environ.copy()
        self.environment["PATH"] = (
            str(self.bin) + os.pathsep
            + self.environment.get("PATH", "")
        )
        self.environment["DAIA_TEST_DOCKER_LOG"] = str(self.log)
        self.environment["DAIA_TEST_MOUNT_FILE"] = str(
            self.mount_file
        )

    def mock_docker(self, mounts, exit_code=0):
        self.mount_file.write_text(mounts)

        docker = self.bin / "docker"

        docker.write_text(
            "#!/bin/sh\n"
            'printf "%s\\n" "$*" >> "$DAIA_TEST_DOCKER_LOG"\n'
            'cat "$DAIA_TEST_MOUNT_FILE"\n'
            f"exit {exit_code}\n"
        )
        docker.chmod(0o755)

    def check_mount(
        self,
        container="daia-pgvector",
        volume="daia-pgvector-data",
    ):
        environment = self.environment.copy()
        environment["DAIA_PGVECTOR_CONTAINER"] = container
        environment["DAIA_PGVECTOR_VOLUME"] = volume

        return subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; '
                'source "$1"; pgvector_container_mount_matches',
                "daia-test",
                str(MODULE),
            ],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def valid_mount(self):
        return (
            "volume|daia-pgvector-data|"
            "/var/lib/docker/volumes/daia-pgvector-data/_data|"
            "/var/lib/postgresql/data|true\n"
            "bind||/var/lib/daia/pgvector/postgres-password|"
            "/run/secrets/daia-pgvector-password|false\n"
        )

    def test_accepts_expected_writable_volume(self):
        self.mock_docker(self.valid_mount())

        result = self.check_mount()

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_missing_mount(self):
        self.mock_docker("")

        result = self.check_mount()

        self.assertEqual(result.returncode, 1)

    def test_rejects_wrong_volume(self):
        self.mock_docker(
            self.valid_mount().replace(
                "volume|daia-pgvector-data|",
                "volume|unrelated-data|",
                1,
            )
        )
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_bind_mount_instead_of_volume(self):
        self.mock_docker(
            self.valid_mount().replace(
                "volume|daia-pgvector-data|",
                "bind|daia-pgvector-data|",
                1,
            )
        )
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_read_only_data_volume(self):
        self.mock_docker(
            self.valid_mount().replace(
                "/var/lib/postgresql/data|true",
                "/var/lib/postgresql/data|false",
                1,
            )
        )
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_duplicate_mounts(self):
        self.mock_docker(self.valid_mount() * 2)
        self.assertEqual(self.check_mount().returncode, 1)

    def test_fails_closed_on_docker_error(self):
        self.mock_docker(self.valid_mount(), exit_code=1)
        self.assertEqual(self.check_mount().returncode, 2)

    def test_rejects_unsafe_volume_name_without_docker(self):
        self.mock_docker(self.valid_mount())
        self.assertEqual(
            self.check_mount(volume="../unsafe").returncode, 2
        )
        self.assertFalse(self.log.exists())

    def test_rejects_missing_secret_mount(self):
        self.mock_docker(self.valid_mount().splitlines(True)[0])
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_writable_secret_mount(self):
        self.mock_docker(
            self.valid_mount().replace(
                "/run/secrets/daia-pgvector-password|false",
                "/run/secrets/daia-pgvector-password|true",
            )
        )
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_wrong_secret_source(self):
        self.mock_docker(
            self.valid_mount().replace(
                "/var/lib/daia/pgvector/postgres-password",
                "/tmp/untrusted-password",
            )
        )
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_extra_mount(self):
        self.mock_docker(
            self.valid_mount()
            + "tmpfs||/tmp|/tmp|true\\n"
        )
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_malformed_mount_record(self):
        self.mock_docker(
            self.valid_mount() + "malformed-record\\n"
        )
        self.assertEqual(self.check_mount().returncode, 1)

    def test_rejects_empty_volume_source(self):
        self.mock_docker(
            self.valid_mount().replace(
                "/var/lib/docker/volumes/daia-pgvector-data/_data",
                "",
            )
        )
        self.assertEqual(self.check_mount().returncode, 1)


if __name__ == "__main__":
    unittest.main()
