import ast
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILDER = ROOT / "build/build-payload.sh"
PUBLISHER = ROOT / "runtime/daia-searxng-publish-registration"
REGISTRATION = ROOT / "runtime/modules/searxng_registration.py"
PROVISION = ROOT / "runtime/modules/searxng_provision.sh"


class SearxngPublisherPackagingTests(unittest.TestCase):

    def test_publisher_is_staged_exactly_once(self):
        source = BUILDER.read_text()
        self.assertEqual(
            source.count(
                '"$DAIA_RUNTIME_SOURCE/'
                'daia-searxng-publish-registration"'
            ),
            1,
        )
        self.assertEqual(
            source.count(
                '"$DAIA_WORKSPACE/usr/libexec/'
                'daia-searxng-publish-registration"'
            ),
            1,
        )

    def test_publisher_staging_is_feature_gated(self):
        source = BUILDER.read_text()
        start = source.index(
            'if [[ "${DAIA_ENABLE_SEARXNG:-false}" == "true" ]]'
        )
        end = source.index(
            'record_staged_component',
            start,
        )
        block = source[start:end]
        self.assertIn(
            'daia-searxng-publish-registration',
            block,
        )
        self.assertIn('0755 || return 1', block)

    def test_registration_module_staging_retained(self):
        source = BUILDER.read_text()
        self.assertIn(
            '"$DAIA_ROOT_TARGET/searxng_registration.py"',
            source,
        )

    def test_publisher_installed_module_path(self):
        source = PUBLISHER.read_text()
        self.assertIn(
            'Path("/opt/daia/searxng_registration.py")',
            source,
        )

    def test_registration_verifies_before_publication(self):
        tree = ast.parse(REGISTRATION.read_text())
        function = next(
            node for node in tree.body
            if isinstance(node, ast.FunctionDef)
            and node.name == "publish_registration"
        )
        calls = [
            node for node in ast.walk(function)
            if isinstance(node, ast.Call)
        ]
        names = [
            node.func.id
            for node in calls
            if isinstance(node.func, ast.Name)
        ]
        self.assertIn("_verify_container", names)
        self.assertIn("open", [
            node.func.attr for node in calls
            if isinstance(node.func, ast.Attribute)
        ])

    def test_container_provisioning_requires_preflight(self):
        source = PROVISION.read_text()
        self.assertIn(
            "searxng_provision_preflight || return 1",
            source,
        )

    def test_publisher_not_automatically_runtime_invoked(self):
        runtime = (ROOT / "runtime/runtime.sh").read_text()
        self.assertNotIn(
            "searxng_provision_container",
            runtime,
        )

    def test_publisher_is_valid_python(self):
        ast.parse(PUBLISHER.read_text())


if __name__ == "__main__":
    unittest.main()
