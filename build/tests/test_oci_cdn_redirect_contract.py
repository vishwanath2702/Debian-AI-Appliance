import importlib.util
import sys
import unittest
from pathlib import Path

FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_cdn_contract", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

DIGEST = "sha256:" + "a" * 64
SOURCE = (
    "https://registry-1.docker.io/v2/"
    "pgvector/pgvector/blobs/" + DIGEST
)
DESTINATION = (
    "https://production.cloudfront.docker.com/"
    "registry-v2/docker/registry/v2/blobs/"
    "sha256/aa/" + "a" * 64 + "/data"
    "?Expires=1893456000&Key-Pair-Id=test-key&Signature=test-signature"
)


class CDNRedirectContractTests(unittest.TestCase):
    def validate(self, destination=DESTINATION, source=SOURCE,
                 kind="blob", count=1):
        return MODULE.validate_oci_redirect(
            source, destination, kind, redirect_count=count
        )

    def test_accepts_exact_cdn_host_for_one_blob_redirect(self):
        self.assertIsNone(self.validate())

    def test_rejects_token_redirect(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(kind="token")

    def test_rejects_manifest_redirect(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(kind="manifest")

    def test_rejects_http_downgrade(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "https://", "http://", 1
            ))

    def test_rejects_lookalike_hostname(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "production.cloudfront.docker.com",
                "production.cloudfront.docker.com.example.org",
            ))

    def test_rejects_embedded_credentials(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "https://", "https://user:password@", 1
            ))

    def test_rejects_nondefault_port(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                ".com/", ".com:8443/", 1
            ))

    def test_rejects_fragment(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION + "#fragment")


    def test_rejects_cdn_hostname_with_trailing_dot(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                ".com/", ".com./", 1
            ))

    def test_rejects_uppercase_cdn_authority(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "production.cloudfront.docker.com",
                "PRODUCTION.CLOUDFRONT.DOCKER.COM",
            ))

    def test_rejects_explicit_default_port(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                ".com/", ".com:443/", 1
            ))

    def test_rejects_empty_destination_path(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(
                destination="https://production.cloudfront.docker.com"
            )

    def test_rejects_network_path_reference(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(
                destination="//production.cloudfront.docker.com/blob"
            )

    def test_rejects_backslash_in_destination(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION + "\\unsafe")

    def test_rejects_control_character_in_destination(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION + "\\n")

    def test_rejects_encoded_control_character(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION + "%0a")

    def test_rejects_empty_signed_query(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.split("?", 1)[0])

    def test_rejects_noninteger_redirect_count(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(count=True)


    def test_rejects_missing_cloudfront_signature(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "&Signature=test-signature", ""
            ))

    def test_rejects_duplicate_signature_parameter(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(
                destination=DESTINATION + "&Signature=another"
            )

    def test_rejects_unknown_query_parameter(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(
                destination=DESTINATION + "&unexpected=value"
            )

    def test_rejects_empty_signature(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "Signature=test-signature", "Signature="
            ))

    def test_rejects_non_registry_v2_path(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "/registry-v2/", "/unrelated/", 1
            ))

    def test_rejects_encoded_path_separator(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "/registry-v2/", "/registry-v2%2f", 1
            ))


    def test_rejects_wrong_cdn_blob_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "a" * 64 + "/data", "b" * 64 + "/data"
            ))

    def test_rejects_wrong_digest_prefix_directory(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "/sha256/aa/", "/sha256/bb/"
            ))

    def test_rejects_wrong_digest_algorithm_directory(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "/sha256/aa/", "/sha512/aa/"
            ))

    def test_rejects_extra_cdn_path_component(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "/data?", "/extra/data?"
            ))

    def test_rejects_cdn_path_suffix(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(destination=DESTINATION.replace(
                "/data?", "/data/extra?"
            ))

    def test_rejects_different_source_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(source=SOURCE.replace(
                "a" * 64, "b" * 64
            ))

    def test_rejects_redirect_chain(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(count=2)

    def test_rejects_noncanonical_source(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(source=SOURCE + "?unexpected=1")

    def test_rejects_unrelated_source_repository(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.validate(source=SOURCE.replace(
                "pgvector/pgvector", "other/repository"
            ))


if __name__ == "__main__":
    unittest.main()
