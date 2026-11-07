import importlib.util
from pathlib import Path
import unittest


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_fetcher", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
import sys
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class DigestReferenceTests(unittest.TestCase):
    DIGEST = "a" * 64

    def test_accepts_digest_pinned_repository(self):
        reference = MODULE.parse_image_reference(
            "pgvector/pgvector@sha256:" + self.DIGEST
        )
        self.assertEqual(reference.registry, "registry-1.docker.io")
        self.assertEqual(reference.repository, "pgvector/pgvector")
        self.assertEqual(reference.digest, "sha256:" + self.DIGEST)

    def test_canonicalizes_official_repository(self):
        reference = MODULE.parse_image_reference(
            "postgres@sha256:" + self.DIGEST
        )
        self.assertEqual(reference.repository, "library/postgres")
        self.assertEqual(reference.digest, "sha256:" + self.DIGEST)

    def test_rejects_tag_and_digest_combination(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            MODULE.parse_image_reference(
                "pgvector/pgvector:pg17@sha256:" + self.DIGEST
            )

    def test_rejects_malformed_digest(self):
        for digest in (
            "sha256:abc",
            "sha256:" + "g" * 64,
            "sha512:" + "a" * 64,
            "sha256:" + "A" * 64,
        ):
            with self.subTest(digest=digest):
                with self.assertRaises(MODULE.ImageReferenceError):
                    MODULE.parse_image_reference(
                        "pgvector/pgvector@" + digest
                    )

    def test_digest_reference_supports_docker_hub_token_request(self):
        reference = MODULE.parse_image_reference(
            "pgvector/pgvector@sha256:" + self.DIGEST
        )
        request = MODULE.build_docker_hub_token_request(reference)

        self.assertEqual(
            request.endpoint,
            "https://auth.docker.io/token",
        )
        self.assertEqual(
            request.parameters,
            (
                ("service", "registry.docker.io"),
                ("scope", "repository:pgvector/pgvector:pull"),
            ),
        )

    def test_tag_and_digest_use_same_repository_scope(self):
        tagged = MODULE.parse_image_reference(
            "pgvector/pgvector:pg17"
        )
        pinned = MODULE.parse_image_reference(
            "pgvector/pgvector@sha256:" + self.DIGEST
        )

        tagged_request = MODULE.build_docker_hub_token_request(tagged)
        pinned_request = MODULE.build_docker_hub_token_request(pinned)

        self.assertEqual(
            tagged_request.parameters,
            pinned_request.parameters,
        )

    def test_preserves_tag_reference_support(self):
        reference = MODULE.parse_image_reference(
            "pgvector/pgvector:pg17"
        )
        self.assertEqual(reference.tag, "pg17")


if __name__ == "__main__":
    unittest.main()
