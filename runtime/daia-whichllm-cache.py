"""DAIA offline snapshot validation for WhichLLM v0.5.21.

This module validates snapshot content without modifying upstream code.
Snapshot freshness is a DAIA policy, not a licensing restriction.
"""
import hashlib
import json
import math
from pathlib import Path

MAX_MODELS_BYTES = 32 * 1024 * 1024
MAX_BENCHMARK_BYTES = 16 * 1024 * 1024

def _verified_json(path, digest, limit):
    path = Path(path)
    digest = Path(digest)
    if path.is_symlink() or digest.is_symlink():
        raise ValueError("Symlinked snapshot rejected")
    raw = path.read_bytes()
    if not raw or len(raw) > limit:
        raise ValueError("Snapshot size invalid")
    expected = digest.read_text(encoding="ascii").strip()
    if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
        raise ValueError("Invalid snapshot checksum")
    if hashlib.sha256(raw).hexdigest() != expected:
        raise ValueError("Snapshot checksum mismatch")
    return json.loads(raw)

def validate_models(path, digest):
    data = _verified_json(path, digest, MAX_MODELS_BYTES)
    if not isinstance(data, dict) or data.get("schema_version") != 4:
        raise ValueError("Unsupported upstream model cache schema")
    if type(data.get("cached_at")) not in (int, float):
        raise ValueError("Missing cache timestamp")
    if not isinstance(data.get("models"), list):
        raise ValueError("Invalid model list")
    return data

def validate_benchmarks(path, digest):
    data = _verified_json(path, digest, MAX_BENCHMARK_BYTES)
    if not isinstance(data, dict) or type(data.get("cached_at")) not in (int, float):
        raise ValueError("Invalid benchmark snapshot")
    scores = data.get("scores")
    if not isinstance(scores, dict):
        raise ValueError("Invalid benchmark scores")
    for key, value in scores.items():
        if not isinstance(key, str) or type(value) not in (int, float):
            raise ValueError("Invalid benchmark entry")
        if not math.isfinite(value) or not 0 <= value <= 100:
            raise ValueError("Invalid benchmark score")
    return data


def prepare_offline_cache(
    models_path,
    models_digest,
    benchmarks_path,
    benchmarks_digest,
    cache_root,
    *,
    now=None,
):
    """Prepare verified upstream-compatible caches for an isolated run.

    The packaged source snapshots are not modified. Only temporary
    copies receive fresh timestamps to prevent upstream refresh.
    Network isolation must still be enforced by the caller.
    """
    import time

    models = validate_models(models_path, models_digest)
    benchmarks = validate_benchmarks(benchmarks_path, benchmarks_digest)

    timestamp = time.time() if now is None else now
    if type(timestamp) not in (int, float) or not math.isfinite(timestamp):
        raise ValueError("Invalid preparation timestamp")

    root = Path(cache_root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("Cache root must be an existing directory")

    cache = root / "whichllm"
    cache.mkdir(mode=0o700, exist_ok=False)

    models_copy = dict(models)
    benchmarks_copy = dict(benchmarks)
    models_copy["cached_at"] = timestamp
    benchmarks_copy["cached_at"] = timestamp

    for filename, data in (
        ("models.json", models_copy),
        ("benchmark.json", benchmarks_copy),
    ):
        destination = cache / filename
        with destination.open("x", encoding="utf-8") as stream:
            json.dump(data, stream, ensure_ascii=False)

    return cache
