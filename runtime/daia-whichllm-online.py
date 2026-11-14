#!/usr/bin/env python3
"""Online WhichLLM recommendations for the DAIA Model Manager."""

import json
import os
from pathlib import Path
import subprocess
import sys

RUNTIME = Path("/usr/libexec/daia/whichllm-runtime")
MAX_OUTPUT = 1024 * 1024


def main():
    if len(sys.argv) != 1:
        raise ValueError("Unexpected arguments")

    if not (RUNTIME / "whichllm" / "cli.py").is_file():
        raise RuntimeError("WhichLLM runtime unavailable")

    env = os.environ.copy()
    env["PYTHONPATH"] = str(RUNTIME)
    env["PYTHONNOUSERSITE"] = "1"
    env["PYTHONDONTWRITEBYTECODE"] = "1"

    command = [
        sys.executable, "-B", "-c",
        "from whichllm.cli import app; app()",
        "--context-length", "8192",
        "--fit", "any",
        "--top", "10",
        "--json",
    ]

    result = subprocess.run(
        command,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=180,
        check=False,
    )

    if result.returncode != 0 or len(result.stdout) > MAX_OUTPUT:
        raise RuntimeError(
            "WhichLLM online recommendations failed: "
            + result.stderr.decode("utf-8", "replace")[-300:]
        )

    data = json.loads(result.stdout)
    if not isinstance(data, dict) or not isinstance(data.get("models"), list):
        raise ValueError("Invalid WhichLLM recommendation response")

    models = data["models"]
    if len(models) > 10:
        raise ValueError("Too many recommendations")

    cleaned = []
    for model in models:
        if not isinstance(model, dict):
            raise ValueError("Invalid model entry")
        name = model.get("model_id")
        if not isinstance(name, str) or not name or len(name) > 256:
            raise ValueError("Invalid model identifier")
        cleaned.append({
            "model_id": name,
            "estimated_tok_per_sec": model.get("estimated_tok_per_sec"),
            "quality_score": model.get("quality_score"),
            "fit_type": model.get("fit_type"),
        })

    print(json.dumps({"schema_version": 1, "models": cleaned}))


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError,
            subprocess.TimeoutExpired) as error:
        print("DAIA online recommendation unavailable: " + str(error),
              file=sys.stderr)
        sys.exit(1)
