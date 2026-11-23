"""Trusted SearXNG configuration filesystem regression tests."""

import importlib.util
import os
from pathlib import Path
import stat
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "runtime/modules/searxng_config.py"
BUILDER = ROOT / "build/build-payload.sh"
CONFIG = ROOT / "payload/config/pragna.conf"

spec = importlib.util.spec_from_file_location("searxng_config", SOURCE)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class TrustedConfigurationTests(unittest.TestCase):

    def test_production_path(self):
        self.assertEqual(
            module.CONFIG_PATH,
            "/opt/daia/config/pragna.conf",
        )

    def test_builder_destination_and_mode(self):
        builder = BUILDER.read_text()
        pattern = (
            r'copy_file\s+\\\s*'
            r'"\$DAIA_CONFIG_FILE"\s+\\\s*'
            r'"\$DAIA_CONFIG_TARGET/pragna\.conf"\s+\\\s*'
            r'0644'
        )
        self.assertRegex(builder, pattern)
        self.assertIn(
            'DAIA_CONFIG_TARGET="$DAIA_ROOT_TARGET/config"',
            builder,
        )

    def test_source_flag_unique_and_enabled(self):
        contents = CONFIG.read_text()
        assignments = [
            line for line in contents.splitlines()
            if line.strip().startswith("DAIA_ENABLE_SEARXNG=")
        ]
        self.assertEqual(len(assignments), 1)
        self.assertIs(module._parse(contents), True)

    def test_valid_values(self):
        for value, expected in (
            ("true", True),
            ("false", False),
            ('"true"', True),
            ("'false'", False),
        ):
            with self.subTest(value=value):
                self.assertIs(
                    module._parse(
                        "DAIA_ENABLE_SEARXNG=" + value + "\n"
                    ),
                    expected,
                )

    def test_duplicate_assignment_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._parse(
                "DAIA_ENABLE_SEARXNG=true\n"
                "DAIA_ENABLE_SEARXNG=false\n"
            )

    def test_shell_expansion_rejected(self):
        with self.assertRaises(module.ConfigError):
            module._parse(
                "DAIA_ENABLE_SEARXNG=$(echo true)\n"
            )

    @staticmethod
    def trusted_directory(directory):
        os.chown(directory, 0, 0)
        directory.chmod(0o700)

    @staticmethod
    def trusted_file(path, contents):
        path.write_text(contents)
        os.chown(path, 0, 0)
        path.chmod(0o644)

    @unittest.skipUnless(
        os.geteuid() == 0,
        "Requires root-owned filesystem fixtures",
    )
    def test_root_owned_true_accepted(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-searxng-", dir="/root"
        ) as tmp:
            directory = Path(tmp)
            self.trusted_directory(directory)
            path = directory / "pragna.conf"
            self.trusted_file(
                path, "DAIA_ENABLE_SEARXNG=true\n"
            )
            self.assertTrue(
                module._parse(module._read_config(str(path)))
            )

    @unittest.skipUnless(
        os.geteuid() == 0,
        "Requires root-owned filesystem fixtures",
    )
    def test_root_owned_false_accepted(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-searxng-", dir="/root"
        ) as tmp:
            directory = Path(tmp)
            self.trusted_directory(directory)
            path = directory / "pragna.conf"
            self.trusted_file(
                path, "DAIA_ENABLE_SEARXNG=false\n"
            )
            self.assertFalse(
                module._parse(module._read_config(str(path)))
            )

    @unittest.skipUnless(
        os.geteuid() == 0,
        "Requires root-owned filesystem fixtures",
    )
    def test_real_pragna_config_accepted(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-searxng-", dir="/root"
        ) as tmp:
            directory = Path(tmp)
            self.trusted_directory(directory)
            path = directory / "pragna.conf"
            path.write_bytes(CONFIG.read_bytes())
            os.chown(path, 0, 0)
            path.chmod(0o644)

            self.assertTrue(
                module._parse(module._read_config(str(path)))
            )

    @unittest.skipUnless(
        os.geteuid() == 0,
        "Requires root-owned filesystem fixtures",
    )
    def test_writable_parent_rejected(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-searxng-", dir="/root"
        ) as tmp:
            directory = Path(tmp)
            self.trusted_directory(directory)
            path = directory / "pragna.conf"
            self.trusted_file(
                path, "DAIA_ENABLE_SEARXNG=true\n"
            )
            directory.chmod(0o777)

            with self.assertRaises(module.ConfigError):
                module._read_config(str(path))

    @unittest.skipUnless(
        os.geteuid() == 0,
        "Requires root-owned filesystem fixtures",
    )
    def test_wrong_file_mode_rejected(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-searxng-", dir="/root"
        ) as tmp:
            directory = Path(tmp)
            self.trusted_directory(directory)
            path = directory / "pragna.conf"
            self.trusted_file(
                path, "DAIA_ENABLE_SEARXNG=true\n"
            )
            path.chmod(0o664)

            with self.assertRaises(module.ConfigError):
                module._read_config(str(path))

    @unittest.skipUnless(
        os.geteuid() == 0,
        "Requires root-owned filesystem fixtures",
    )
    def test_symlink_file_rejected(self):
        with tempfile.TemporaryDirectory(
            prefix="daia-searxng-", dir="/root"
        ) as tmp:
            directory = Path(tmp)
            self.trusted_directory(directory)
            target = directory / "real.conf"
            self.trusted_file(
                target, "DAIA_ENABLE_SEARXNG=true\n"
            )
            link = directory / "pragna.conf"
            link.symlink_to(target)

            with self.assertRaises(module.ConfigError):
                module._read_config(str(link))


if __name__ == "__main__":
    unittest.main()
