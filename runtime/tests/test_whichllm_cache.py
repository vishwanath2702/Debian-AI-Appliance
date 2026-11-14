import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "daia-whichllm-cache.py"
spec = importlib.util.spec_from_file_location("daia_whichllm_cache", SOURCE)
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)

class OfflineSnapshotTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)

    def snapshot(self, value):
        raw = json.dumps(value).encode()
        path = self.base / "snapshot.json"
        digest = self.base / "snapshot.sha256"
        path.write_bytes(raw)
        digest.write_text(hashlib.sha256(raw).hexdigest())
        return path, digest

    def test_stale_models_remain_accepted(self):
        path, digest = self.snapshot({
            "schema_version": 4, "cached_at": 1, "models": []
        })
        self.assertEqual(cache.validate_models(path, digest)["models"], [])

    def test_stale_benchmarks_remain_accepted(self):
        path, digest = self.snapshot({
            "cached_at": 1, "scores": {"example": 75.0}
        })
        self.assertEqual(
            cache.validate_benchmarks(path, digest)["scores"]["example"], 75.0
        )

    def test_tampering_rejected(self):
        path, digest = self.snapshot({
            "schema_version": 4, "cached_at": 1, "models": []
        })
        path.write_text("{}")
        with self.assertRaises(ValueError):
            cache.validate_models(path, digest)

    def test_symlink_rejected(self):
        path, digest = self.snapshot({
            "schema_version": 4, "cached_at": 1, "models": []
        })
        link = self.base / "linked.json"
        link.symlink_to(path)
        with self.assertRaises(ValueError):
            cache.validate_models(link, digest)

    def test_invalid_benchmark_rejected(self):
        path, digest = self.snapshot({
            "cached_at": 1, "scores": {"example": 150}
        })
        with self.assertRaises(ValueError):
            cache.validate_benchmarks(path, digest)

if __name__ == "__main__":
    unittest.main()


class CacheHandoffTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)

    def snapshot(self, name, value):
        raw = json.dumps(value).encode()
        path = self.base / (name + ".json")
        digest = self.base / (name + ".sha256")
        path.write_bytes(raw)
        digest.write_text(hashlib.sha256(raw).hexdigest())
        return path, digest

    def test_prepares_both_upstream_caches(self):
        models, models_hash = self.snapshot(
            "models",
            {"schema_version": 4, "cached_at": 1, "models": []},
        )
        benchmarks, benchmarks_hash = self.snapshot(
            "benchmarks",
            {"cached_at": 1, "scores": {"example": 75.0}},
        )
        cache_root = self.base / "private"
        cache_root.mkdir()

        destination = cache.prepare_offline_cache(
            models, models_hash,
            benchmarks, benchmarks_hash,
            cache_root, now=1234567890,
        )

        prepared_models = json.loads(
            (destination / "models.json").read_text()
        )
        prepared_benchmarks = json.loads(
            (destination / "benchmark.json").read_text()
        )

        self.assertEqual(prepared_models["schema_version"], 4)
        self.assertEqual(prepared_models["cached_at"], 1234567890)
        self.assertEqual(prepared_benchmarks["cached_at"], 1234567890)
        self.assertEqual(prepared_benchmarks["scores"]["example"], 75.0)

        self.assertEqual(
            json.loads(models.read_text())["cached_at"], 1
        )

    def test_rejects_modified_snapshot(self):
        models, models_hash = self.snapshot(
            "models",
            {"schema_version": 4, "cached_at": 1, "models": []},
        )
        benchmarks, benchmarks_hash = self.snapshot(
            "benchmarks",
            {"cached_at": 1, "scores": {}},
        )
        models.write_text("{}")
        cache_root = self.base / "private"
        cache_root.mkdir()

        with self.assertRaises(ValueError):
            cache.prepare_offline_cache(
                models, models_hash,
                benchmarks, benchmarks_hash,
                cache_root,
            )
