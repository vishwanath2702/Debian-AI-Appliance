import importlib.util
import io
import ssl
import sys
import unittest
from http.client import HTTPMessage
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_https_security", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

DIGEST = "sha256:" + "a" * 64
REGISTRY = "https://registry-1.docker.io"
MANIFEST = REGISTRY + "/v2/pgvector/pgvector/manifests/" + DIGEST
TOKEN_URL = (
    "https://auth.docker.io/token"
    "?service=registry.docker.io"
    "&scope=repository%3Apgvector%2Fpgvector%3Apull"
)


class MockResponse:
    status = 200

    def __init__(self):
        self.headers = HTTPMessage()
        self.headers.add_header("Content-Length", "3")
        self.stream = io.BytesIO(b"abc")

    def read(self, size=-1):
        return self.stream.read(size)


class MockConnection:
    def __init__(self):
        self.requests = []
        self.closed = False

    def request(self, method, path, body=None, headers=None):
        self.requests.append((method, path, body, headers))

    def getresponse(self):
        return MockResponse()

    def close(self):
        self.closed = True


class OCIHTTPSSecurityTests(unittest.TestCase):
    def invoke(self, url, token=None):
        connection = MockConnection()
        captured = []

        def factory(host, **kwargs):
            captured.append((host, kwargs))
            return connection

        try:
            result = MODULE.fetch_oci_https_response(
                url,
                max_bytes=1024,
                bearer_token=token,
                connection_factory=factory,
            )
            return result, connection, captured
        except MODULE.OCIManifestError:
            raise

    def test_tls_verifies_certificate_and_hostname(self):
        result, connection, captured = self.invoke(MANIFEST)
        self.assertEqual(result, b"abc")
        context = captured[0][1]["context"]
        self.assertEqual(context.verify_mode, ssl.CERT_REQUIRED)
        self.assertTrue(context.check_hostname)
        self.assertEqual(captured[0][1]["timeout"], 30)
        self.assertTrue(connection.closed)

    def test_registry_token_is_sent_only_to_manifest(self):
        result, connection, _ = self.invoke(
            MANIFEST, token="test-token"
        )
        self.assertEqual(result, b"abc")
        self.assertEqual(
            connection.requests[0][3]["Authorization"],
            "Bearer test-token",
        )

    def test_auth_endpoint_never_receives_bearer_token(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(TOKEN_URL, token="test-token")

    def test_token_request_is_anonymous(self):
        result, connection, _ = self.invoke(TOKEN_URL)
        self.assertEqual(result, b"abc")
        self.assertNotIn(
            "Authorization", connection.requests[0][3]
        )

    def test_rejects_unrelated_registry_path_with_token(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(REGISTRY + "/admin", token="test-token")

    def test_rejects_unrelated_registry_path_without_token(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(REGISTRY + "/admin")

    def test_rejects_unrelated_auth_path(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke("https://auth.docker.io/unrelated")

    def test_rejects_registry_encoded_path_separator(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(
                REGISTRY + "/v2/pgvector%2Fpgvector/manifests/" + DIGEST,
                token="test-token",
            )

    def test_rejects_registry_path_traversal(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(
                REGISTRY + "/v2/pgvector/../admin",
                token="test-token",
            )

    def test_rejects_unexpected_auth_query(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(
                "https://auth.docker.io/token?scope=other"
            )

    def test_rejects_registry_query(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(MANIFEST + "?unexpected=1", token="test-token")

    def test_rejects_invalid_token_whitespace(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.invoke(MANIFEST, token="bad token")


if __name__ == "__main__":
    unittest.main()
