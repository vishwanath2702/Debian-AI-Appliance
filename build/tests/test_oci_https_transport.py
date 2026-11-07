import importlib.util
import io
import sys
import unittest
from http.client import HTTPMessage
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_https_transport", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class MockResponse:
    def __init__(self, status=200, body=b"abc", headers=None):
        self.status = status
        self.body = io.BytesIO(body)
        self.headers = HTTPMessage()
        pairs = headers if headers is not None else [
            ("Content-Length", str(len(body)))
        ]
        for key, value in pairs:
            self.headers.add_header(key, value)

    def read(self, size=-1):
        return self.body.read(size)


class MockConnection:
    def __init__(self, response):
        self.response = response
        self.requests = []
        self.closed = False

    def request(self, method, path, body=None, headers=None):
        self.requests.append((method, path, body, headers))

    def getresponse(self):
        return self.response

    def close(self):
        self.closed = True


class OCIHTTPSTransportTests(unittest.TestCase):
    def fetch(self, response, url=None, token=None, limit=1024):
        connection = MockConnection(response)
        captured = []

        def factory(host, **kwargs):
            captured.append((host, kwargs))
            return connection

        result = MODULE.fetch_oci_https_response(
            url or (
                "https://registry-1.docker.io/v2/"
                "pgvector/pgvector/manifests/sha256:" + "a" * 64
            ),
            max_bytes=limit,
            bearer_token=token,
            connection_factory=factory,
        )
        return result, connection, captured

    def test_fetches_bounded_response(self):
        result, connection, captured = self.fetch(
            MockResponse(body=b"abc")
        )
        self.assertEqual(result, b"abc")
        self.assertEqual(captured[0][0], "registry-1.docker.io")
        self.assertEqual(connection.requests[0][0], "GET")
        self.assertTrue(connection.closed)

    def test_rejects_redirect(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(status=302, body=b""))

    def test_rejects_oversized_body(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(
                MockResponse(
                    body=b"abcd",
                    headers=[("Content-Length", "3")],
                )
            )

    def test_rejects_duplicate_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(headers=[
                ("Content-Length", "3"),
                ("Content-Length", "4"),
            ]))

    def test_accepts_bounded_chunked_response(self):
        result, connection, captured = self.fetch(MockResponse(
            body=b"abc",
            headers=[("Transfer-Encoding", "chunked")],
        ))
        self.assertEqual(result, b"abc")
        self.assertTrue(connection.closed)
        self.assertEqual(captured[0][0], "registry-1.docker.io")

    def test_rejects_ambiguous_chunked_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(
                body=b"abc",
                headers=[
                    ("Transfer-Encoding", "chunked"),
                    ("Content-Length", "3"),
                ],
            ))

    def test_rejects_unsupported_transfer_encoding(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(
                body=b"abc",
                headers=[("Transfer-Encoding", "gzip")],
            ))

    def test_rejects_oversized_chunked_response(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(
                body=b"abcd",
                headers=[("Transfer-Encoding", "chunked")],
            ), limit=3)

    def test_rejects_unapproved_host(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(
                MockResponse(),
                url="https://example.com/v2/test",
            )

    def test_rejects_plain_http(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(
                MockResponse(),
                url="http://registry-1.docker.io/v2/test",
            )

    def test_rejects_url_credentials(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(
                MockResponse(),
                url="https://user:pass@registry-1.docker.io/v2/test",
            )

    def test_rejects_truncated_response(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(
                body=b"ab",
                headers=[("Content-Length", "3")],
            ))


if __name__ == "__main__":
    unittest.main()
