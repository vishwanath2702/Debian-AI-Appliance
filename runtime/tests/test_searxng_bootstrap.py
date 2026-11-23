"""Unit tests for the SearXNG bootstrap decision."""

import importlib.util
from pathlib import Path
import stat
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "runtime/modules/searxng_bootstrap.py"

spec = importlib.util.spec_from_file_location("searxng_bootstrap", SOURCE)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

Decision = module.BootstrapDecision


def metadata(mode, uid=0, dev=1, ino=1):
    class Info:
        st_mode = mode
        st_uid = uid
        st_dev = dev
        st_ino = ino
    return Info()


class BootstrapTests(unittest.TestCase):

    def test_nonroot_refused(self):
        with patch.object(module.os, "geteuid", return_value=1000):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_custom_path_refused(self):
        with patch.object(module.os, "geteuid", return_value=0):
            self.assertEqual(
                module.decide("/tmp/searxng"),
                Decision.UNTRUSTED,
            )

    def test_missing_parent_refused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", side_effect=FileNotFoundError),
        ):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_no_mutating_operations(self):
        text = SOURCE.read_text()
        for forbidden in (
            "os.mkdir(",
            "os.makedirs(",
            "subprocess.",
            "docker ",
            "systemctl ",
            "os.chmod(",
            "os.chown(",
        ):
            self.assertNotIn(forbidden, text)

    def test_enum_is_explicit(self):
        self.assertEqual(
            {decision.value for decision in Decision},
            {
                "INITIALIZE_ELIGIBLE",
                "OBSERVE_EXISTING",
                "UNTRUSTED",
            },
        )

    def test_parent_permissions_refused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         return_value=metadata(stat.S_IFDIR | 0o777)),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_parent_owner_refused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o755, uid=1000
                         )),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_missing_registration_is_only_eligible(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o755
                         )),
            patch.object(module.os, "stat",
                         side_effect=FileNotFoundError),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(
                module.decide(),
                Decision.INITIALIZE_ELIGIBLE,
            )

    def test_symlink_registration_refused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o755
                         )),
            patch.object(module.os, "stat",
                         return_value=metadata(
                             stat.S_IFLNK | 0o777
                         )),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_existing_trusted_directory(self):
        calls = iter([
            metadata(stat.S_IFDIR | 0o755),
            metadata(stat.S_IFDIR | 0o755),
            metadata(stat.S_IFDIR | 0o755),
            metadata(stat.S_IFDIR | 0o755),
            metadata(stat.S_IFDIR | 0o700),
        ])
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         side_effect=lambda fd: next(calls)),
            patch.object(module.os, "stat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o700
                         )),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(
                module.decide(),
                Decision.OBSERVE_EXISTING,
            )

    def test_existing_wrong_mode_refused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o755
                         )),
            patch.object(module.os, "stat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o755
                         )),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_existing_foreign_owner_refused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o755
                         )),
            patch.object(module.os, "stat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o700, uid=1000
                         )),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_stat_failure_refused(self):
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module.os, "open", return_value=10),
            patch.object(module.os, "fstat",
                         return_value=metadata(
                             stat.S_IFDIR | 0o755
                         )),
            patch.object(module.os, "stat",
                         side_effect=PermissionError),
            patch.object(module.os, "close"),
        ):
            self.assertEqual(module.decide(), Decision.UNTRUSTED)

    def test_no_docker_authorization(self):
        self.assertFalse(hasattr(module, "provision"))
        self.assertFalse(hasattr(module, "start"))
        self.assertFalse(hasattr(module, "initialize"))

    def test_fixed_path(self):
        self.assertEqual(
            module.DEFAULT_DIRECTORY,
            "/var/lib/daia/searxng",
        )


if __name__ == "__main__":
    unittest.main()
