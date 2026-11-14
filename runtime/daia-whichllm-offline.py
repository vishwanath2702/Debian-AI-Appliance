#!/usr/bin/env python3
"""DAIA offline WhichLLM runner; fail closed without verified snapshots."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

SNAPSHOTS = Path("/usr/share/daia/whichllm")
RUNTIME = Path("/usr/libexec/daia/whichllm-runtime/bin/whichllm")
MAX_OUTPUT = 1024 * 1024

def load_cache_helper():
    source = Path(__file__).resolve().with_name("daia-whichllm-cache.py")
    if not source.is_file() or source.is_symlink():
        raise RuntimeError("DAIA cache helper unavailable")
    spec = importlib.util.spec_from_file_location("daia_whichllm_cache", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def recommendation_command(bwrap, home):
    command = [
        bwrap, "--die-with-parent", "--unshare-user",
        "--unshare-net", "--unshare-pid", "--new-session",
    ]
    for directory in ("/usr", "/lib", "/lib64"):
        if Path(directory).exists():
            command += ["--ro-bind", directory, directory]
    command += [
        "--ro-bind", str(home), "/cache",
        "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
        "--setenv", "HOME", "/tmp",
        "--setenv", "XDG_CACHE_HOME", "/cache",
        "--", str(RUNTIME),
        "--context-length", "8192", "--top", "10",
        "--fit", "any", "--json",
    ]
    return command

def main():
    if len(sys.argv) != 1:
        raise ValueError("No user-controlled command arguments accepted")
    bwrap = shutil.which("bwrap")
    if not bwrap or not RUNTIME.is_file():
        raise RuntimeError("Offline WhichLLM runtime unavailable")
    helper = load_cache_helper()
    with tempfile.TemporaryDirectory(prefix="daia-whichllm-") as home:
        cache_root = Path(home)
        helper.prepare_offline_cache(
            SNAPSHOTS / "models.json",
            SNAPSHOTS / "models.sha256",
            SNAPSHOTS / "benchmark.json",
            SNAPSHOTS / "benchmark.sha256",
            cache_root,
        )
        command = recommendation_command(bwrap, cache_root)
        result = subprocess.run(
            command, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, timeout=45, check=False,
        )
        if result.returncode != 0 or len(result.stdout) > MAX_OUTPUT:
            raise RuntimeError("Offline recommendation execution failed")
        output = json.loads(result.stdout)
        if not isinstance(output, dict) or not isinstance(output.get("models"), list):
            raise ValueError("Unexpected recommendation output")
        if len(output["models"]) > 10:
            raise ValueError("Too many recommendations")
        cleaned = []
        for model in output["models"]:
            if not isinstance(model, dict) or not isinstance(model.get("model_id"), str):
                raise ValueError("Invalid recommendation")
            if not model["model_id"] or len(model["model_id"]) > 256:
                raise ValueError("Invalid model identifier")
            cleaned.append({
                key: model.get(key) for key in (
                    "model_id", "estimated_tok_per_sec", "quality_score",
                    "speed_confidence", "fit_type", "benchmark_source", "can_run",
                )
            })
        print(json.dumps({
            "schema_version": 1,
            "source": "whichllm-offline",
            "context_length": 8192,
            "models": cleaned,
        }))

if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        print("DAIA recommendation unavailable: " + str(error), file=sys.stderr)
        sys.exit(1)
