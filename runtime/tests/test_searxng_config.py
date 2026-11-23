"""Behavioral security tests for SearXNG configuration."""

from pathlib import Path
import importlib.util
import os
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "runtime/modules/searxng_config.py"

spec = importlib.util.spec_from_file_location("searxng_config", SOURCE)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ConfigTests(unittest.TestCase):

    def test_true(self):
        self.assertTrue(module._parse("DAIA_ENABLE_SEARXNG=true\n"))

    def test_false(self):
        self.assertFalse(module._parse("DAIA_ENABLE_SEARXNG=false\n"))

    def test_double_quoted(self):
        self.assertTrue(module._parse(
            'DAIA_ENABLE_SEARXNG="true"\n'
        ))

    def test_single_quoted(self):
        self.assertFalse(module._parse(
            "DAIA_ENABLE_SEARXNG='false'\n"
        ))

    def test_comments_and_other_settings(self):
        self.assertTrue(module._parse(
            "# comment\nOTHER_SETTING=value\n"
            "DAIA_ENABLE_SEARXNG=true\n"
        ))

    def test_missing_flag_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._parse("OTHER_SETTING=true\n")

    def test_duplicate_flag_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._parse(
                "DAIA_ENABLE_SEARXNG=true\n"
                "DAIA_ENABLE_SEARXNG=false\n"
            )

    def test_invalid_value_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._parse("DAIA_ENABLE_SEARXNG=maybe\n")

    def test_shell_substitution_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._parse(
                "DAIA_ENABLE_SEARXNG=$(touch /tmp/unsafe)\n"
            )

    def test_trailing_shell_command_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._parse(
                "DAIA_ENABLE_SEARXNG=true;touch /tmp/unsafe\n"
            )

    def test_no_shell_execution(self):
        text = SOURCE.read_text()
        for forbidden in (
            "subprocess", "os.system(", "eval(", "exec("
        ):
            self.assertNotIn(forbidden, text)

    def test_relative_path_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._read_config("relative/pragna.conf")

    def test_parent_traversal_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._read_config("/tmp/../pragna.conf")

    def test_nonroot_production_rejected(self):
        with patch.object(module.os, "geteuid", return_value=1000):
            with self.assertRaises(module.ConfigError):
                module.enabled()

    def test_production_path_fixed(self):
        self.assertEqual(
            module.CONFIG_PATH,
            "/opt/daia/config/pragna.conf",
        )

    def test_symlink_file_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "real.conf"
            target.write_text("DAIA_ENABLE_SEARXNG=true\n")
            link = root / "pragna.conf"
            link.symlink_to(target)

            with self.assertRaises(module.ConfigError):
                module._read_config(str(link))

    def test_nonroot_owned_file_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pragna.conf"
            path.write_text("DAIA_ENABLE_SEARXNG=true\n")

            with self.assertRaises(module.ConfigError):
                module._read_config(str(path))

    def test_writable_parent_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pragna.conf"
            path.write_text("DAIA_ENABLE_SEARXNG=true\n")

            with self.assertRaises(module.ConfigError):
                module._read_config(str(path))

    def test_directory_descriptor_checks(self):
        text = SOURCE.read_text()
        self.assertIn("os.O_NOFOLLOW", text)
        self.assertIn("dir_fd=fd", text)
        self.assertIn("os.fstat", text)

    def test_no_firstboot_activation(self):
        firstboot = (ROOT / "runtime/firstboot.sh").read_text()
        self.assertNotIn("searxng_config.py", firstboot)


if __name__ == "__main__":
    unittest.main()
