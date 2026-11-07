import hashlib
import importlib.util
import io
import os
import sys
import tempfile
import unittest
from http.client import HTTPMessage
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_https_blob_stream", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

DATA = b"DAIA verified OCI blob"
DIGEST = "sha256:" + hashlib.sha256(DATA).hexdigest()
URL = (
    "https://registry-1.docker.io/v2/"
    "pgvector/pgvector/blobs/" + DIGEST
)


class MockResponse:
    status = 200

    def __init__(self, body, declared=None):
        self.body = io.BytesIO(body)
        self.headers = HTTPMessage()
        self.headers.add_header(
            "Content-Length",
            str(len(body) if declared is None else declared),
        )

    def read(self, size=-1):
        return self.body.read(size)


class MockConnection:
    def __init__(self, body, declared=None):
        self.response = MockResponse(body, declared)
        self.closed = False

    def request(self, method, target, body=None, headers=None):
        self.method = method
        self.target = target
        self.headers = headers

    def getresponse(self):
        return self.response

    def close(self):
        self.closed = True


class HTTPSBlobStreamTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        os.chmod(self.directory.name, 0o700)
        self.directory_fd = os.open(
            self.directory.name,
            os.O_RDONLY | os.O_DIRECTORY,
        )
        self.connections = []

    def tearDown(self):
        os.close(self.directory_fd)
        self.directory.cleanup()

    def fetch(self, body=DATA, digest=DIGEST, url=URL,
              expected_size=None, declared=None):
        def factory(host, **kwargs):
            connection = MockConnection(body, declared)
            self.connections.append(connection)
            return connection

        return MODULE.fetch_and_publish_oci_https_blob(
            url=url,
            trusted_digest=digest,
            expected_size=(
                len(DATA) if expected_size is None else expected_size
            ),
            directory_fd=self.directory_fd,
            connection_factory=factory,
        )

    def published(self):
        return sorted(os.listdir(self.directory.name))

    def test_publishes_verified_blob(self):
        result = self.fetch()
        expected_name = "sha256-" + DIGEST[7:]
        self.assertEqual(result, expected_name)
        self.assertEqual(self.published(), [expected_name])
        self.assertEqual(
            (Path(self.directory.name) / expected_name).read_bytes(),
            DATA,
        )
        self.assertTrue(self.connections[0].closed)

    def test_rejects_tampered_bytes_without_publication(self):
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            self.fetch(body=DATA[:-1] + b"!")
        self.assertEqual(self.published(), [])
        self.assertTrue(self.connections[0].closed)

    def test_rejects_truncated_response_without_publication(self):
        with self.assertRaises(
            (MODULE.OCIManifestError, MODULE.OCIBlobIntegrityError)
        ):
            self.fetch(body=DATA[:-1], declared=len(DATA))
        self.assertEqual(self.published(), [])
        self.assertTrue(self.connections[0].closed)

    def test_rejects_wrong_descriptor_size(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(expected_size=len(DATA) + 1)
        self.assertEqual(self.published(), [])
        self.assertTrue(self.connections[0].closed)

    def test_rejects_missing_trust_before_connection(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.fetch(digest="")
        self.assertEqual(self.connections, [])
        self.assertEqual(self.published(), [])

    def test_rejects_mismatched_url_before_connection(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(url=URL + "#")
        self.assertEqual(self.connections, [])
        self.assertEqual(self.published(), [])

    def test_rejects_wrong_digest_before_connection(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(digest="sha256:" + "0" * 64)
        self.assertEqual(self.connections, [])
        self.assertEqual(self.published(), [])


if __name__ == "__main__":
    unittest.main()
