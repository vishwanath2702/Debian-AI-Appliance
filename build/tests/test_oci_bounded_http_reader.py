import importlib.util
import io
import sys
import unittest
from pathlib import Path


FETCHER = Path(__file__).resolve().parents[1] / "fetch-oci-image.py"
SPEC = importlib.util.spec_from_file_location(
    "daia_oci_bounded_http_reader", FETCHER
)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class BoundedOCIHTTPReaderTests(unittest.TestCase):
    def reader(self, body, declared):
        return MODULE.BoundedOCIHTTPReader(
            io.BytesIO(body), declared
        )

    def test_accepts_exact_body(self):
        reader = self.reader(b"abcdef", 6)
        self.assertEqual(reader.read(3), b"abc")
        self.assertEqual(reader.read(3), b"def")
        self.assertEqual(reader.read(1), b"")

    def test_rejects_truncated_body(self):
        reader = self.reader(b"ab", 3)
        self.assertEqual(reader.read(3), b"ab")
        with self.assertRaises(MODULE.OCIManifestError):
            reader.read(1)

    def test_never_reads_beyond_declared_length(self):
        raw = io.BytesIO(b"abcdef")
        reader = MODULE.BoundedOCIHTTPReader(raw, 3)
        self.assertEqual(reader.read(10), b"abc")
        self.assertEqual(reader.read(1), b"")
        self.assertEqual(raw.read(), b"def")

    def test_rejects_negative_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.reader(b"", -1)

    def test_rejects_boolean_length(self):
        with self.assertRaises(MODULE.OCIManifestError):
            self.reader(b"", True)

    def test_rejects_invalid_read_size(self):
        reader = self.reader(b"abc", 3)
        with self.assertRaises(MODULE.OCIManifestError):
            reader.read(-1)

    def test_rejects_nonbyte_reader_output(self):
        reader = MODULE.BoundedOCIHTTPReader(
            io.StringIO("abc"), 3
        )
        with self.assertRaises(MODULE.OCIManifestError):
            reader.read(3)

    def test_rejects_reader_exceeding_requested_size(self):
        class Oversized:
            def read(self, size):
                return b"x" * (size + 1)

        reader = MODULE.BoundedOCIHTTPReader(Oversized(), 3)
        with self.assertRaises(MODULE.OCIManifestError):
            reader.read(2)


if __name__ == "__main__":
    unittest.main()
