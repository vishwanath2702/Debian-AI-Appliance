import ast
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/searxng_provision.sh"
RUNTIME = ROOT / "runtime/runtime.sh"
PUBLISHER = ROOT / "runtime/daia-searxng-publish-registration"


def creation_source():
    return MODULE.read_text().split(
        "searxng_provision_container() {", 1
    )[1].split("\n}", 1)[0]


class ContainerCreationTests(unittest.TestCase):

    def test_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(MODULE)],
            capture_output=True, text=True
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_root_guard(self):
        self.assertEqual(
            MODULE.read_text().count(
                '[[ "$EUID" -eq 0 ]] || return 1'
            ), 1
        )

    def test_preflight(self):
        self.assertIn(
            "searxng_provision_preflight || return 1",
            creation_source()
        )

    def test_create_only_flags(self):
        source = creation_source()
        for required in (
            "docker container create",
            "--restart no",
            "--security-opt no-new-privileges:true",
            "--network daia-rag-internal",
            "org.daia.component=searxng",
            "org.daia.managed=true",
        ):
            self.assertIn(required, source)

    def test_forbidden_operations(self):
        source = creation_source()
        for forbidden in (
            "--privileged",
            "--publish",
            "--volume",
            "docker container start",
            "docker container rm",
            "docker network rm",
            "docker network create",
        ):
            self.assertNotIn(forbidden, source)

    def test_immutable_egress_attachment(self):
        self.assertIn(
            'docker network connect "$egress" "$container_id"',
            creation_source()
        )

    def test_verification_before_publication(self):
        source = creation_source()
        self.assertLess(
            source.index(
                'verified_id="$(/usr/libexec/daia-searxng-verify-container'
            ),
            source.index(
                '\n    /usr/libexec/daia-searxng-publish-registration'
            )
        )

    def test_pinned_image(self):
        self.assertIn(
            "sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d",
            creation_source()
        )

    def test_not_runtime_wired(self):
        self.assertNotIn(
            "searxng_provision_container",
            RUNTIME.read_text()
        )

    def test_publisher_syntax(self):
        ast.parse(PUBLISHER.read_text())


if __name__ == "__main__":
    unittest.main()
