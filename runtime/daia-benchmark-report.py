#!/usr/bin/env python3
"""Render a completed DAIA benchmark as a standalone HTML report."""

import html
import json
import math
from pathlib import Path


def escape(value):
    return html.escape(str(value), quote=True)


def number(value, digits=2):
    if type(value) not in (int, float) or not math.isfinite(value):
        return "Unavailable"
    return f"{value:.{digits}f}"


def gib(value):
    if type(value) not in (int, float) or value < 0:
        return "Unavailable"
    return number(value / (1024 ** 3)) + " GiB"


def render(events):
    if not isinstance(events, list):
        raise ValueError("Expected JSONL event list")

    completed = [
        event for event in events
        if isinstance(event, dict)
        and event.get("schema_version") == 1
        and event.get("event") == "completed"
    ]

    if len(completed) != 1:
        raise ValueError("Exactly one completed event required")

    result = completed[0]
    model = result.get("model")
    runs = result.get("runs")

    for event in events:
        if not isinstance(event, dict):
            raise ValueError("Invalid benchmark event")
        if event.get("schema_version") != 1:
            raise ValueError("Unsupported benchmark event version")
        if event.get("event") == "failed":
            raise ValueError("Failed benchmark cannot produce a report")
        if event.get("event") not in (
            "started", "hardware_sample", "completed"
        ):
            raise ValueError("Unexpected benchmark event")
        if event.get("event") == "started":
            if event.get("model") != model:
                raise ValueError("Benchmark model mismatch")

    if not isinstance(model, str) or not model:
        raise ValueError("Invalid model")
    if not isinstance(runs, list):
        raise ValueError("Invalid benchmark runs")

    measured = [
        run for run in runs
        if isinstance(run, dict) and run.get("kind") == "measured"
    ]
    if not measured:
        raise ValueError("No measured benchmark runs")

    samples = [
        event for event in events
        if isinstance(event, dict)
        and event.get("schema_version") == 1
        and event.get("event") == "hardware_sample"
    ]

    rows = []
    rates = []

    for index, run in enumerate(measured, 1):
        tokens = run.get("eval_count")
        duration = run.get("eval_duration_ns")

        rate = None
        if (
            type(tokens) in (int, float)
            and type(duration) in (int, float)
            and duration > 0
            and tokens >= 0
            and math.isfinite(tokens)
            and math.isfinite(duration)
        ):
            rate = tokens * 1e9 / duration
            if math.isfinite(rate):
                rates.append(rate)

        rows.append(
            "<tr><td>" + str(index) + "</td><td>" +
            escape(tokens) + "</td><td>" +
            number(rate) + "</td><td>" +
            number(run.get("total_duration_ns", 0) / 1e9)
            + "</td></tr>"
            if type(run.get("total_duration_ns")) in (int, float)
            else
            "<tr><td>" + str(index) + "</td><td>" +
            escape(tokens) + "</td><td>" +
            number(rate) + "</td><td>Unavailable</td></tr>"
        )

    average = sum(rates) / len(rates) if rates else None

    hardware_rows = []
    for sample in samples:
        hardware_rows.append(
            "<tr><td>" +
            number(sample.get("cpu_utilization_percent")) +
            "%</td><td>" +
            gib(sample.get("ram_used_bytes")) +
            "</td><td>" +
            gib(sample.get("ram_total_bytes")) +
            "</td></tr>"
        )

    gpu_rows = []
    for sample in samples:
        gpus = sample.get("gpus")
        if not isinstance(gpus, list):
            continue
        for gpu in gpus:
            if not isinstance(gpu, dict):
                continue
            gpu_rows.append(
                "<tr><td>" + escape(gpu.get("identifier", "Unknown")) +
                "</td><td>" +
                number(gpu.get("utilization_percent")) +
                "%</td><td>" +
                gib(gpu.get("memory_used_bytes")) +
                "</td><td>" +
                number(gpu.get("temperature_celsius")) +
                " °C</td></tr>"
            )

    return """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy"
      content="default-src 'none'; style-src 'unsafe-inline'">
<title>DAIA Benchmark Report</title>
<style>
body {font-family:system-ui,sans-serif;max-width:1100px;
      margin:40px auto;padding:0 20px;line-height:1.6}
nav a {margin-right:20px}
section {margin-top:36px;padding:20px;border:1px solid #aaa;
         border-radius:10px}
table {border-collapse:collapse;width:100%}
th,td {padding:10px;text-align:left;border-bottom:1px solid #aaa}
</style>
</head>
<body>
<h1>DAIA Model Benchmark Report</h1>
<nav>
<a href="#model">Model</a>
<a href="#performance">Performance</a>
<a href="#hardware">Hardware</a>
<a href="#gpu">GPU</a>
</nav>
<section id="model">
<h2>Model</h2><p>""" + escape(model) + """</p>
</section>
<section id="performance">
<h2>Performance</h2>
<p>Measured runs: """ + str(len(measured)) + """</p>
<p>Average measured generation throughput: """ + number(average) + """ tokens/s</p>
<table><tr><th>Run</th><th>Generated tokens</th>
<th>Tokens/s</th><th>Total duration (s)</th></tr>""" + (
        "".join(rows)
    ) + """</table>
</section>
<section id="hardware">
<h2>Hardware observations</h2>
<p>Recorded samples: """ + str(len(samples)) + """</p>
<table><tr><th>CPU utilization</th><th>RAM used</th>
<th>RAM total</th></tr>""" + (
        "".join(hardware_rows) or
        '<tr><td colspan="3">No hardware samples</td></tr>'
    ) + """</table>
</section>
<section id="gpu">
<h2>GPU observations</h2>
<table><tr><th>GPU</th><th>Utilization</th>
<th>Memory used</th><th>Temperature</th></tr>""" + (
        "".join(gpu_rows) or
        '<tr><td colspan="4">No GPU telemetry available</td></tr>'
    ) + """</table>
</section>
</body></html>"""


def render_jsonl(text):
    events = []
    for line in text.splitlines():
        if line.strip():
            events.append(json.loads(line))
    return render(events)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("events", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    result = render_jsonl(args.events.read_text(encoding="utf-8"))
    args.output.write_text(result, encoding="utf-8")
