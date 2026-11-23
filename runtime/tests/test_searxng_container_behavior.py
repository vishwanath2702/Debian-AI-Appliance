import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng_provision.sh"

IDENTITY = "a" * 64
OTHER_IDENTITY = "b" * 64
IMAGE = (
    "sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d"
)
ROOT_GUARD = '[[ "$EUID" -eq 0 ]] || return 1'

FAKE_DOCKER = r"""#!/usr/bin/env python3
import os
import sys

args = sys.argv[1:]
command = " ".join(args)
log = os.environ["MOCK_LOG"]

with open(log, "a", encoding="utf-8") as stream:
    stream.write("docker " + command + "\n")

def fail(name):
    if os.environ.get("MOCK_FAIL") == name:
        sys.exit(43)

if args == ["info"]:
    fail("info")
elif args[:2] == ["image", "inspect"]:
    fail("image_inspect")
    print(os.environ["DAIA_SEARXNG_DOCKER_IMAGE_ID"])
elif args[:2] == ["container", "ls"]:
    fail("container_inventory")
    if os.environ.get("MOCK_EXISTING_CONTAINER") == "1":
        print("daia-searxng")
elif args[:2] == ["network", "inspect"]:
    fail("network_inspect")
    name = args[-1]
    if name == "daia-rag-internal":
        if os.environ.get("MOCK_BAD_INTERNAL") == "1":
            print("foreign|bridge|false|rag|true")
        else:
            print("daia-rag-internal|bridge|true|rag|true")
    elif name == "daia-searxng-egress":
        if ".Containers" in command:
            if os.environ.get("MOCK_EGRESS_MEMBERS") == "1":
                print("foreign-member")
        elif os.environ.get("MOCK_BAD_EGRESS") == "1":
            print("foreign|bridge|false|searxng|true")
        else:
            print("daia-searxng-egress|bridge|false|searxng|true")
    else:
        sys.exit(44)
elif args[:2] == ["container", "create"]:
    fail("create")
    print(os.environ.get("MOCK_CREATED_ID", "a" * 64))
elif args[:2] == ["container", "inspect"]:
    fail("container_inspect")
    print(os.environ.get("MOCK_INSPECTED_ID", "a" * 64))
elif args[:2] == ["network", "connect"]:
    fail("connect")
else:
    print("UNEXPECTED_DOCKER_COMMAND: " + command, file=sys.stderr)
    sys.exit(99)
"""

FAKE_VERIFIER = r"""#!/usr/bin/env python3
import os
import sys

with open(os.environ["MOCK_LOG"], "a", encoding="utf-8") as stream:
    stream.write("verify " + " ".join(sys.argv[1:]) + "\n")

if os.environ.get("MOCK_FAIL") == "verify":
    sys.exit(45)

print(os.environ.get("MOCK_VERIFIED_ID", "a" * 64))
"""

FAKE_PUBLISHER = r"""#!/usr/bin/env python3
import os
import sys

with open(os.environ["MOCK_LOG"], "a", encoding="utf-8") as stream:
    stream.write("publish " + " ".join(sys.argv[1:]) + "\n")

if os.environ.get("MOCK_FAIL") == "publish":
    sys.exit(46)
"""


class ContainerBehaviorTests(unittest.TestCase):

    def run_provision(self, overrides=None):
        with tempfile.TemporaryDirectory(prefix="daia-searxng-test-") as tmp:
            directory = Path(tmp)
            docker = directory / "docker"
            verifier = directory / "verify-container"
            publisher = directory / "publish-registration"
            test_module = directory / "searxng-provision.sh"
            log = directory / "operations.log"

            for path, source in (
                (docker, FAKE_DOCKER),
                (verifier, FAKE_VERIFIER),
                (publisher, FAKE_PUBLISHER),
            ):
                path.write_text(source)
                path.chmod(0o700)

            source = MODULE.read_text()

            self.assertEqual(source.count(ROOT_GUARD), 1)

            production_verifier = (
                "/usr/libexec/daia-searxng-verify-container"
            )
            production_publisher = (
                "/usr/libexec/daia-searxng-publish-registration"
            )

            self.assertGreaterEqual(
                source.count(production_verifier), 2
            )
            self.assertGreaterEqual(
                source.count(production_publisher), 2
            )

            # All substitutions affect only the temporary test copy.
            source = source.replace(
                ROOT_GUARD,
                "true # test-only root bypass",
                1,
            )
            source = source.replace(
                production_verifier,
                str(verifier),
            )
            source = source.replace(
                production_publisher,
                str(publisher),
            )

            test_module.write_text(source)

            syntax = subprocess.run(
                ["bash", "-n", str(test_module)],
                capture_output=True,
                text=True,
            )
            self.assertEqual(syntax.returncode, 0, syntax.stderr)

            env = dict(os.environ)
            env.update({
                "PATH": str(directory) + os.pathsep + env.get("PATH", ""),
                "MOCK_LOG": str(log),
                "DAIA_ENABLE_SEARXNG": "true",
                "DAIA_SEARXNG_DOCKER_IMAGE_ID": IMAGE,
            })
            env.update(overrides or {})

            result = subprocess.run(
                [
                    "bash", "-c",
                    'source "$1"; searxng_provision_container',
                    "test",
                    str(test_module),
                ],
                cwd=directory,
                env=env,
                capture_output=True,
                text=True,
                timeout=15,
            )

            operations = (
                log.read_text().splitlines() if log.exists() else []
            )
            return result, operations

    def assert_no_registration(self, operations):
        self.assertFalse(
            any(line.startswith("publish ") for line in operations),
            operations,
        )

    def assert_no_unsafe_operations(self, operations):
        for line in operations:
            self.assertFalse(
                line.startswith((
                    "docker container start",
                    "docker container rm",
                    "docker network rm",
                    "docker network create",
                    "docker container run",
                )),
                line,
            )

    def test_successful_create_attach_verify_publish(self):
        result, operations = self.run_provision()
        self.assertEqual(result.returncode, 0, result.stderr)

        create = next(
            i for i, line in enumerate(operations)
            if line.startswith("docker container create ")
        )
        attach = next(
            i for i, line in enumerate(operations)
            if line.startswith("docker network connect ")
        )
        verify = operations.index("verify " + IDENTITY)
        publish = operations.index("publish " + IDENTITY)

        self.assertLess(create, attach)
        self.assertLess(attach, verify)
        self.assertLess(verify, publish)

        self.assertIn(
            "docker network connect daia-searxng-egress " + IDENTITY,
            operations,
        )
        self.assertEqual(
            sum(line.startswith("publish ") for line in operations),
            1,
        )
        self.assert_no_unsafe_operations(operations)

    def test_disabled_component_rejected_before_creation(self):
        result, operations = self.run_provision({
            "DAIA_ENABLE_SEARXNG": "false",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("container create" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_existing_container_rejected(self):
        result, operations = self.run_provision({
            "MOCK_EXISTING_CONTAINER": "1",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("container create" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_untrusted_egress_rejected(self):
        result, operations = self.run_provision({
            "MOCK_BAD_EGRESS": "1",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("container create" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_egress_members_rejected(self):
        result, operations = self.run_provision({
            "MOCK_EGRESS_MEMBERS": "1",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("container create" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_creation_failure_stops_provisioning(self):
        result, operations = self.run_provision({
            "MOCK_FAIL": "create",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("network connect" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_invalid_created_identity_rejected(self):
        result, operations = self.run_provision({
            "MOCK_CREATED_ID": "invalid-id",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("network connect" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_container_identity_drift_rejected(self):
        result, operations = self.run_provision({
            "MOCK_INSPECTED_ID": OTHER_IDENTITY,
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("network connect" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_egress_attachment_failure_blocks_registration(self):
        result, operations = self.run_provision({
            "MOCK_FAIL": "connect",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assert_no_registration(operations)
        self.assertFalse(
            any(line.startswith("verify ") for line in operations)
        )

    def test_verifier_failure_blocks_registration(self):
        result, operations = self.run_provision({
            "MOCK_FAIL": "verify",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assert_no_registration(operations)

    def test_verifier_identity_mismatch_blocks_registration(self):
        result, operations = self.run_provision({
            "MOCK_VERIFIED_ID": OTHER_IDENTITY,
        })
        self.assertNotEqual(result.returncode, 0)
        self.assert_no_registration(operations)

    def test_publisher_failure_propagates(self):
        result, operations = self.run_provision({
            "MOCK_FAIL": "publish",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("verify " + IDENTITY, operations)
        self.assertIn("publish " + IDENTITY, operations)

    def test_inventory_failure_is_fail_closed(self):
        result, operations = self.run_provision({
            "MOCK_FAIL": "container_inventory",
        })
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any("container create" in line for line in operations)
        )
        self.assert_no_registration(operations)

    def test_no_automatic_runtime_activation(self):
        runtime = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn("searxng_provision_container", runtime)
        self.assertNotIn("searxng_provision_egress_network", runtime)


if __name__ == "__main__":
    unittest.main()
