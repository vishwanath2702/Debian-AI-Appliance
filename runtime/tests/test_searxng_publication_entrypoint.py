import ast
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
PUBLISHER = ROOT / "runtime/daia-searxng-publish-registration"
VALID_ID = "a" * 64
ROOT_CHECK = "os.geteuid() != 0"

STUB = """
from pathlib import Path
import os

DEFAULT_DIRECTORY = "/var/lib/daia/searxng"

def publish_registration(container_id, directory):
    if os.environ.get("MOCK_REJECT") == "1":
        raise PermissionError(
            "mock independent verification rejected"
        )
    Path(os.environ["MOCK_PUBLICATION_LOG"]).write_text(
        container_id + "|" + directory
    )
"""


class PublicationEntrypointTests(unittest.TestCase):

    def run_publisher(self, arguments, overrides=None):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)

            script = (
                base / "daia-searxng-publish-registration"
            )
            modules = base / "modules"
            modules.mkdir()

            source = PUBLISHER.read_text()
            self.assertEqual(
                source.count(ROOT_CHECK), 1
            )

            # Root bypass exists only in the temporary test copy.
            script.write_text(
                source.replace(ROOT_CHECK, "False")
            )

            (
                modules / "searxng_registration.py"
            ).write_text(STUB)

            log = base / "publication.log"

            env = dict(os.environ)
            env["MOCK_PUBLICATION_LOG"] = str(log)
            env.update(overrides or {})

            result = subprocess.run(
                [
                    "python3",
                    str(script),
                    *arguments,
                ],
                env=env,
                capture_output=True,
                text=True,
                timeout=10,
            )

            published = (
                log.read_text()
                if log.exists()
                else None
            )

            return result, published

    def test_python_syntax(self):
        ast.parse(PUBLISHER.read_text())

    def test_production_root_guard(self):
        self.assertEqual(
            PUBLISHER.read_text().count(ROOT_CHECK),
            1,
        )

    def test_exact_identity_pattern(self):
        source = PUBLISHER.read_text()
        self.assertIn(
            r'[0-9a-f]{64}\Z',
            source,
        )

    def test_valid_identity_published(self):
        result, published = self.run_publisher(
            [VALID_ID]
        )

        self.assertEqual(
            result.returncode,
            0,
            result.stderr,
        )

        self.assertEqual(
            published,
            VALID_ID + "|/var/lib/daia/searxng",
        )

    def test_invalid_identity_rejected(self):
        result, published = self.run_publisher(
            ["invalid"]
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(published)

    def test_uppercase_identity_rejected(self):
        result, published = self.run_publisher(
            ["A" * 64]
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(published)

    def test_missing_identity_rejected(self):
        result, published = self.run_publisher([])
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(published)

    def test_extra_arguments_rejected(self):
        result, published = self.run_publisher(
            [VALID_ID, "extra"]
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(published)

    def test_verification_rejection_propagates(self):
        result, published = self.run_publisher(
            [VALID_ID],
            {"MOCK_REJECT": "1"},
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(published)

        self.assertIn(
            "mock independent verification rejected",
            result.stderr,
        )

    def test_no_replacement_implementation(self):
        source = PUBLISHER.read_text()

        for forbidden in (
            "os.replace(",
            "os.rename(",
            "unlink(",
        ):
            self.assertNotIn(forbidden, source)

        self.assertIn(
            "module.publish_registration(",
            source,
        )

    def test_publication_follows_container_verification(self):
        source = (ROOT / "runtime/modules/searxng_provision.sh").read_text()

        function = source.split(
            "searxng_provision_container() {", 1
        )[1].split("\n}", 1)[0]

        verifier = function.index(
            'verified_id="$(/usr/libexec/daia-searxng-verify-container'
        )
        publisher = function.index(
            '/usr/libexec/daia-searxng-publish-registration \\',
            verifier,
        )

        self.assertLess(verifier, publisher)
        self.assertIn(
            '[[ -x /usr/libexec/daia-searxng-publish-registration ]]',
            function,
        )

    def test_not_connected_to_runtime(self):
        source = (
            ROOT / "runtime/runtime.sh"
        ).read_text()

        self.assertNotIn(
            "daia-searxng-publish-registration",
            source,
        )


if __name__ == "__main__":
    unittest.main()
