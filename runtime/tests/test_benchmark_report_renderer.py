"""Regression tests for standalone DAIA benchmark HTML rendering."""

import importlib.util
import unittest
from pathlib import Path


RENDERER = Path(__file__).resolve().parents[1] / "daia-benchmark-report.py"

spec = importlib.util.spec_from_file_location(
    "daia_benchmark_report_renderer_tests", RENDERER
)
if spec is None or spec.loader is None:
    raise RuntimeError("Unable to load benchmark renderer")

module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class BenchmarkReportRendererTests(unittest.TestCase):
    def events(self, gpus="MISSING"):
        sample = {
            "schema_version": 1,
            "event": "hardware_sample",
            "cpu_utilization_percent": 25,
            "ram_used_bytes": 1073741824,
            "ram_total_bytes": 4294967296,
        }

        if gpus != "MISSING":
            sample["gpus"] = gpus

        return [
            {
                "schema_version": 1,
                "event": "started",
                "model": "example:latest",
            },
            sample,
            {
                "schema_version": 1,
                "event": "completed",
                "model": "example:latest",
                "runs": [{
                    "kind": "measured",
                    "eval_count": 100,
                    "eval_duration_ns": 4000000000,
                    "total_duration_ns": 5000000000,
                }],
            },
        ]

    def test_cpu_only_report(self):
        html = module.render(self.events([]))
        self.assertIn('id="gpu"', html)
        self.assertIn("No GPU telemetry available", html)

    def test_missing_gpu_field(self):
        html = module.render(self.events())
        self.assertIn("No GPU telemetry available", html)

    def test_malformed_gpu_fields(self):
        for value in (None, 42, {"identifier": "Invalid"}, "Invalid"):
            with self.subTest(value=value):
                html = module.render(self.events(value))
                self.assertIn("No GPU telemetry available", html)

    def test_valid_gpu_record(self):
        html = module.render(
            self.events([{"identifier": "TestGPU4315"}])
        )
        self.assertIn("TestGPU4315", html)

    def test_gpu_identifier_is_html_escaped(self):
        html = module.render(
            self.events([{"identifier": "<script>alert(1)</script>"}])
        )
        self.assertNotIn("<script>", html)
        self.assertIn("&lt;script&gt;", html)

    def test_navigation_anchors(self):
        html = module.render(self.events([]))
        for section in ("model", "performance", "hardware", "gpu"):
            with self.subTest(section=section):
                self.assertIn(f'href="#{section}"', html)
                self.assertIn(f'id="{section}"', html)


if __name__ == "__main__":
    unittest.main()
