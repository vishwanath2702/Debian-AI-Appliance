import hashlib
import importlib.util
import io
import os
import ssl
import sys
import tempfile
import unittest
from http.client import HTTPMessage
from pathlib import Path

FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_cdn_transport", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

DATA = b"DAIA CDN verified blob"
DIGEST_HEX = hashlib.sha256(DATA).hexdigest()
DIGEST = "sha256:" + DIGEST_HEX
SOURCE = (
    "https://registry-1.docker.io/v2/"
    "pgvector/pgvector/blobs/" + DIGEST
)
DESTINATION = (
    "https://production.cloudfront.docker.com/"
    "registry-v2/docker/registry/v2/blobs/"
    "sha256/" + DIGEST_HEX[:2] + "/" + DIGEST_HEX + "/data"
    "?Expires=1893456000&Key-Pair-Id=test-key&Signature=test-signature"
)


class MockResponse:
    def __init__(self, status, body=b"", location=None):
        self.status = status
        self.body = io.BytesIO(body)
        self.headers = HTTPMessage()
        if location is not None:
            self.headers.add_header("Location", location)
        else:
            self.headers.add_header("Content-Length", str(len(body)))

    def read(self, size=-1):
        return self.body.read(size)


class MockConnection:
    def __init__(self, response):
        self.response = response
        self.requests = []
        self.closed = False

    def request(self, method, target, body=None, headers=None):
        self.requests.append((method, target, body, dict(headers or {})))

    def getresponse(self):
        return self.response

    def close(self):
        self.closed = True


class CDNTransportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        os.chmod(self.temp.name, 0o700)
        self.fd = os.open(
            self.temp.name, os.O_RDONLY | os.O_DIRECTORY
        )
        self.connections = []
        self.contexts = []

    def tearDown(self):
        os.close(self.fd)
        self.temp.cleanup()

    def fetch(self, cdn_response=None):
        responses = [
            MockResponse(307, location=DESTINATION),
            cdn_response or MockResponse(200, body=DATA),
        ]

        def factory(host, **kwargs):
            self.contexts.append(kwargs.get("context"))
            if len(self.connections) >= len(responses):
                raise AssertionError("Unexpected third connection")
            connection = MockConnection(responses[len(self.connections)])
            connection.host = host
            self.connections.append(connection)
            return connection

        return MODULE.fetch_and_publish_oci_https_blob(
            SOURCE,
            DIGEST,
            len(DATA),
            self.fd,
            connection_factory=factory,
            bearer_token="registry-secret-token",
        )

    def test_redirect_uses_separate_credential_free_cdn_connection(self):
        result = self.fetch()
        self.assertEqual(result, "sha256-" + DIGEST_HEX)
        self.assertEqual(
            [connection.host for connection in self.connections],
            ["registry-1.docker.io", "production.cloudfront.docker.com"],
        )
        registry, cdn = self.connections
        self.assertEqual(
            registry.requests[0][3].get("Authorization"),
            "Bearer registry-secret-token",
        )
        self.assertNotIn("Authorization", cdn.requests[0][3])
        self.assertNotIn("Proxy-Authorization", cdn.requests[0][3])
        self.assertEqual(cdn.requests[0][0], "GET")
        self.assertEqual(
            cdn.requests[0][1],
            DESTINATION.split("production.cloudfront.docker.com", 1)[1],
        )
        self.assertTrue(all(connection.closed for connection in self.connections))
        self.assertTrue(all(context is not None for context in self.contexts))
        self.assertTrue(all(
            context.verify_mode == ssl.CERT_REQUIRED
            and context.check_hostname
            for context in self.contexts
        ))
        self.assertEqual(
            (Path(self.temp.name) / result).read_bytes(), DATA
        )

    def test_rejects_second_redirect_without_publication(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(307, location=DESTINATION))
        self.assertEqual(os.listdir(self.temp.name), [])
        self.assertTrue(all(c.closed for c in self.connections))

    def test_rejects_cdn_digest_mismatch_without_publication(self):
        altered = DATA[:-1] + b"!"
        with self.assertRaises(MODULE.OCIBlobIntegrityError):
            self.fetch(MockResponse(200, body=altered))
        self.assertEqual(os.listdir(self.temp.name), [])
        self.assertTrue(all(c.closed for c in self.connections))


    def test_rejects_duplicate_location_header(self):
        response = MockResponse(307, location=DESTINATION)
        response.headers.add_header("Location", DESTINATION)

        connections = []

        def factory(host, **kwargs):
            if connections:
                raise AssertionError("Unexpected CDN connection")
            connection = MockConnection(response)
            connections.append(connection)
            return connection

        with self.assertRaises(MODULE.OCIManifestError):
            MODULE.fetch_and_publish_oci_https_blob(
                SOURCE, DIGEST, len(DATA), self.fd,
                connection_factory=factory,
                bearer_token="registry-secret-token",
            )

        self.assertEqual(len(connections), 1)
        self.assertTrue(connections[0].closed)
        self.assertEqual(os.listdir(self.temp.name), [])

    def test_rejects_missing_location_header(self):
        response = MockResponse(307)

        connections = []

        def factory(host, **kwargs):
            if connections:
                raise AssertionError("Unexpected CDN connection")
            connection = MockConnection(response)
            connections.append(connection)
            return connection

        with self.assertRaises(MODULE.OCIManifestError):
            MODULE.fetch_and_publish_oci_https_blob(
                SOURCE, DIGEST, len(DATA), self.fd,
                connection_factory=factory,
                bearer_token="registry-secret-token",
            )

        self.assertEqual(len(connections), 1)
        self.assertTrue(connections[0].closed)
        self.assertEqual(os.listdir(self.temp.name), [])

    def test_rejects_unapproved_cdn_destination(self):
        response = MockResponse(
            307,
            location=DESTINATION.replace(
                "production.cloudfront.docker.com",
                "untrusted.example.org",
            ),
        )

        connections = []

        def factory(host, **kwargs):
            if connections:
                raise AssertionError("Unexpected CDN connection")
            connection = MockConnection(response)
            connections.append(connection)
            return connection

        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.fetch_and_publish_oci_https_blob(
                SOURCE, DIGEST, len(DATA), self.fd,
                connection_factory=factory,
                bearer_token="registry-secret-token",
            )

        self.assertEqual(len(connections), 1)
        self.assertTrue(connections[0].closed)
        self.assertEqual(os.listdir(self.temp.name), [])

    def test_rejects_cdn_http_error_without_publication(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(MockResponse(403, body=b""))

        self.assertEqual(
            [connection.host for connection in self.connections],
            ["registry-1.docker.io", "production.cloudfront.docker.com"],
        )
        self.assertTrue(all(c.closed for c in self.connections))
        self.assertEqual(os.listdir(self.temp.name), [])

    def test_rejects_cdn_connection_failure_without_publication(self):
        registry_response = MockResponse(
            307, location=DESTINATION
        )
        connections = []

        def factory(host, **kwargs):
            if host == "production.cloudfront.docker.com":
                raise OSError("simulated CDN connection failure")

            connection = MockConnection(registry_response)
            connections.append(connection)
            return connection

        with self.assertRaises(OSError):
            MODULE.fetch_and_publish_oci_https_blob(
                SOURCE, DIGEST, len(DATA), self.fd,
                connection_factory=factory,
                bearer_token="registry-secret-token",
            )

        self.assertEqual(len(connections), 1)
        self.assertTrue(connections[0].closed)
        self.assertEqual(os.listdir(self.temp.name), [])


    def assert_rejects_redirect_headers(self, extra_headers):
        response = MockResponse(307, location=DESTINATION)

        for name, value in extra_headers:
            response.headers.add_header(name, value)

        connections = []

        def factory(host, **kwargs):
            if connections:
                raise AssertionError(
                    "Unsafe redirect reached CDN connection"
                )

            connection = MockConnection(response)
            connections.append(connection)
            return connection

        with self.assertRaises(MODULE.OCIManifestError):
            MODULE.fetch_and_publish_oci_https_blob(
                SOURCE,
                DIGEST,
                len(DATA),
                self.fd,
                connection_factory=factory,
                bearer_token="registry-secret-token",
            )

        self.assertEqual(len(connections), 1)
        self.assertTrue(connections[0].closed)
        self.assertEqual(os.listdir(self.temp.name), [])

    def test_rejects_redirect_transfer_encoding(self):
        self.assert_rejects_redirect_headers([
            ("Transfer-Encoding", "chunked"),
            ("Content-Length", "0"),
        ])

    def test_rejects_redirect_content_encoding(self):
        self.assert_rejects_redirect_headers([
            ("Content-Encoding", "gzip"),
        ])

    def test_rejects_redirect_header_injection(self):
        self.assert_rejects_redirect_headers([
            ("X-Test", "safe\r\nInjected: unsafe"),
        ])

    def test_rejects_redirect_connection_upgrade(self):
        self.assert_rejects_redirect_headers([
            ("Connection", "Upgrade"),
            ("Upgrade", "websocket"),
        ])


    def test_rejects_truncated_cdn_body(self):
        response = MockResponse(200, body=DATA[:-1])
        response.headers.replace_header(
            "Content-Length", str(len(DATA))
        )

        with self.assertRaises(
            (MODULE.OCIManifestError, MODULE.OCIBlobIntegrityError)
        ):
            self.fetch(response)

        self.assertEqual(os.listdir(self.temp.name), [])
        self.assertTrue(all(c.closed for c in self.connections))

    def test_rejects_cdn_content_length_mismatch(self):
        response = MockResponse(200, body=DATA)
        response.headers.replace_header(
            "Content-Length", str(len(DATA) + 1)
        )

        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(response)

        self.assertEqual(os.listdir(self.temp.name), [])
        self.assertTrue(all(c.closed for c in self.connections))

    def test_rejects_duplicate_cdn_content_length(self):
        response = MockResponse(200, body=DATA)
        response.headers.add_header(
            "Content-Length", str(len(DATA))
        )

        with self.assertRaises(MODULE.OCIManifestError):
            self.fetch(response)

        self.assertEqual(os.listdir(self.temp.name), [])
        self.assertTrue(all(c.closed for c in self.connections))


    def test_preserves_existing_blob_on_cdn_collision(self):
        filename = "sha256-" + DIGEST_HEX
        destination = Path(self.temp.name) / filename
        destination.write_bytes(b"existing trusted content")

        with self.assertRaises(
            (FileExistsError, MODULE.OCIManifestError,
             MODULE.OCIBlobIntegrityError, OSError)
        ):
            self.fetch()

        self.assertEqual(
            destination.read_bytes(),
            b"existing trusted content",
        )
        self.assertEqual(os.listdir(self.temp.name), [filename])
        self.assertTrue(all(c.closed for c in self.connections))

    def test_rejects_cdn_body_read_failure(self):
        class FailingResponse(MockResponse):
            def read(self, size=-1):
                raise OSError("simulated CDN body read failure")

        with self.assertRaises(OSError):
            self.fetch(FailingResponse(200, body=DATA))

        self.assertEqual(os.listdir(self.temp.name), [])
        self.assertTrue(all(c.closed for c in self.connections))

    def test_cleans_staging_after_file_fsync_failure(self):
        from unittest.mock import patch

        real_fsync = os.fsync
        calls = []

        def failing_fsync(fd):
            calls.append(fd)
            if len(calls) == 1:
                raise OSError("simulated staging fsync failure")
            return real_fsync(fd)

        with patch.object(MODULE.os, "fsync", side_effect=failing_fsync):
            with self.assertRaises(OSError):
                self.fetch()

        self.assertGreaterEqual(len(calls), 1)
        self.assertEqual(os.listdir(self.temp.name), [])
        self.assertTrue(all(c.closed for c in self.connections))


if __name__ == "__main__":
    unittest.main()
