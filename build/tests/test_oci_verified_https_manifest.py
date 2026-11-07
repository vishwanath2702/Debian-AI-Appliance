import hashlib
import importlib.util
import io
import sys
import unittest
from http.client import HTTPMessage
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_verified_https_manifest", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

DATA = b'{"schemaVersion":2}'
DIGEST = "sha256:" + hashlib.sha256(DATA).hexdigest()
URL = (
    "https://registry-1.docker.io/v2/"
    "pgvector/pgvector/manifests/" + DIGEST
)


class MockResponse:
    status = 200

    def __init__(self, body):
        self.headers = HTTPMessage()
        self.headers.add_header("Content-Length", str(len(body)))
        self.body = io.BytesIO(body)

    def read(self, size=-1):
        return self.body.read(size)


class MockConnection:
    def __init__(self, body):
        self.response = MockResponse(body)
        self.closed = False

    def request(self, method, path, body=None, headers=None):
        pass

    def getresponse(self):
        return self.response

    def close(self):
        self.closed = True


class VerifiedHTTPSManifestTests(unittest.TestCase):
    def fetch(self, body=DATA, digest=DIGEST, limit=1024):
        connection = MockConnection(body)

        def factory(host, **kwargs):
            return connection

        result = MODULE.fetch_verified_oci_manifest(
            URL,
            trusted_digest=digest,
            max_bytes=limit,
            connection_factory=factory,
        )
        return result, connection

    def test_accepts_verified_manifest(self):
        result, connection = self.fetch()
        self.assertEqual(result, DATA)
        self.assertTrue(connection.closed)

    def test_rejects_tampered_manifest(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            self.fetch(body=DATA + b" ")

    def test_rejects_missing_trusted_digest(self):
        with self.assertRaises(
            (MODULE.ImageReferenceError, MODULE.OCIBlobIntegrityError)
        ):
            self.fetch(digest="")

    def test_rejects_wrong_trusted_digest(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(digest="sha256:" + "0" * 64)

    def test_rejects_manifest_over_size_limit(self):
        with self.assertRaises(
            (MODULE.OCIManifestError, MODULE.OCIBlobIntegrityError)
        ):
            self.fetch(limit=4)


    def test_forwards_bearer_token_to_registry(self):
        connection = MockConnection(DATA)
        requests = []

        def request(method, path, body=None, headers=None):
            requests.append((method, path, dict(headers or {})))

        connection.request = request

        def factory(host, **kwargs):
            self.assertEqual(host, "registry-1.docker.io")
            return connection

        result = MODULE.fetch_verified_oci_manifest(
            URL,
            trusted_digest=DIGEST,
            max_bytes=1024,
            connection_factory=factory,
            bearer_token="test-registry-token",
        )

        self.assertEqual(result, DATA)
        self.assertTrue(connection.closed)
        self.assertEqual(len(requests), 1)
        self.assertEqual(requests[0][0], "GET")
        self.assertEqual(
            requests[0][2].get("Authorization"),
            "Bearer test-registry-token",
        )



if __name__ == "__main__":
    unittest.main()
