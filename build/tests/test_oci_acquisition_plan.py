import importlib.util
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location("daia_oci_plan", FETCHER)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OCIAcquisitionPlanTests(unittest.TestCase):
    REFERENCE = "pgvector/pgvector:pg17"
    DIGEST = "sha256:" + "a" * 64

    def plan(self, reference=None, digest=None):
        return MODULE.plan_pgvector_oci_acquisition(
            self.REFERENCE if reference is None else reference,
            self.DIGEST if digest is None else digest,
        )

    def test_constructs_digest_pinned_plan(self):
        plan = self.plan()
        self.assertEqual(
            plan.manifest_reference,
            "pgvector/pgvector@" + self.DIGEST,
        )
        self.assertEqual(plan.expected_digest, self.DIGEST)
        self.assertEqual(plan.runtime_reference, self.REFERENCE)

    def test_uses_canonical_docker_hub_repository(self):
        plan = self.plan()
        self.assertEqual(
            plan.repository, "pgvector/pgvector"
        )
        self.assertEqual(
            plan.registry, "registry-1.docker.io"
        )

    def test_rejects_missing_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.plan(digest="")

    def test_rejects_malformed_digest(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.plan(digest="sha256:invalid")

    def test_rejects_wrong_repository(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.plan(reference="library/postgres:17")

    def test_rejects_unexpected_tag(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.plan(reference="pgvector/pgvector:latest")

    def test_rejects_digest_reference_as_runtime_tag(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.plan(reference="pgvector/pgvector@" + self.DIGEST)

    def test_rejects_non_string_reference(self):
        with self.assertRaises(MODULE.ImageReferenceError):
            self.plan(reference=123)


if __name__ == "__main__":
    unittest.main()
