import importlib.util
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_header_syntax", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OCIHTTPHeaderSyntaxTests(unittest.TestCase):
    def check(self, headers):
        return MODULE.validate_oci_http_response(
            200, headers, 1024
        )

    def test_accepts_normal_headers(self):
        self.assertEqual(
            self.check({"Content-Length": "128"}),
            128,
        )

    def test_rejects_header_name_with_space(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check({"Content Length": "128"})

    def test_rejects_header_name_with_newline(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check({"Content-Length\nInjected": "128"})

    def test_rejects_header_value_with_newline(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check({"Content-Length": "128\nInjected"})

    def test_rejects_header_value_with_carriage_return(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check({"Content-Length": "128\rInjected"})

    def test_rejects_header_name_with_non_ascii(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check({"Content-Léngth": "128"})

    def test_rejects_header_value_with_nul(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check({
                "Content-Length": "128",
                "X-Extra": "abc\x00def",
            })

    def test_rejects_header_value_with_control_character(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.check({
                "Content-Length": "128",
                "X-Extra": "abc\x01def",
            })


if __name__ == "__main__":
    unittest.main()
