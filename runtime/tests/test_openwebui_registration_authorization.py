"""Security contract for Open WebUI registration authorization."""

import ast
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/openwebui_registration.py"


class OpenWebUIRegistrationAuthorizationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = MODULE.read_text()
        cls.tree = ast.parse(cls.source)

    def test_registration_requires_authorized_identity(self):
        self.assertIn(
            "authorize_container_registration",
            self.source,
        )

    def test_authorization_requires_immutable_image_identity(self):
        self.assertIn("expected_image", self.source)
        self.assertIn("inspected_image", self.source)

    def test_authorization_requires_daia_container_labels(self):
        self.assertIn("org.daia.component", self.source)
        self.assertIn("org.daia.managed", self.source)

    def test_authorization_requires_restricted_network(self):
        self.assertIn("daia-rag-internal", self.source)

    def test_authorization_requires_no_new_privileges(self):
        self.assertIn("no-new-privileges:true", self.source)

    def test_authorization_precedes_registration_publication(self):
        functions = {
            node.name: node
            for node in self.tree.body
            if isinstance(node, ast.FunctionDef)
        }
        self.assertIn("register_authorized_container", functions)
        body = ast.unparse(
            functions["register_authorized_container"]
        )
        self.assertLess(
            body.index("authorize_container_registration("),
            body.index("publish_trusted_registration("),
        )


if __name__ == "__main__":
    unittest.main()
