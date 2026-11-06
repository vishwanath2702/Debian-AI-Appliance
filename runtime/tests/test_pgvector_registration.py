"""Security contract for DAIA pgvector container registration."""

import importlib.util
import sys
import unittest
from pathlib import Path


HELPER = (
    Path(__file__).resolve().parents[1]
    / "pgvector_registration.py"
)


def load_registration():
    if not HELPER.is_file() or HELPER.is_symlink():
        raise AssertionError(
            "DAIA pgvector registration module is not implemented"
        )

    spec = importlib.util.spec_from_file_location(
        "daia_pgvector_registration",
        HELPER,
    )
    module = importlib.util.module_from_spec(spec)

    runtime_directory = str(HELPER.parent)
    original_path = sys.path[:]

    try:
        sys.path.insert(0, runtime_directory)
        spec.loader.exec_module(module)
    finally:
        sys.path[:] = original_path

    return module


class PgvectorRegistrationValidationTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.registration = load_registration()
        globals()["registration"] = cls.registration

    def test_accepts_valid_container_id(self):
        container_id = "a" * 64
        self.assertEqual(
            self.registration.validate_container_id(container_id),
            container_id,
        )

    def test_rejects_short_container_id(self):
        with self.assertRaises(ValueError):
            self.registration.validate_container_id("a" * 63)

    def test_rejects_uppercase_container_id(self):
        with self.assertRaises(ValueError):
            self.registration.validate_container_id("A" * 64)

    def test_rejects_nonhex_container_id(self):
        with self.assertRaises(ValueError):
            self.registration.validate_container_id("g" * 64)

    def test_rejects_whitespace_container_id(self):
        with self.assertRaises(ValueError):
            self.registration.validate_container_id(" " + "a" * 64)

    def test_rejects_newline_in_container_id(self):
        with self.assertRaises(ValueError):
            self.registration.validate_container_id("a" * 64 + "\n")

    def test_accepts_registration_with_final_newline(self):
        container_id = "a" * 64
        self.assertEqual(
            self.registration.parse_registration(
                (container_id + "\n").encode("ascii")
            ),
            container_id,
        )

    def test_accepts_registration_without_newline(self):
        container_id = "a" * 64
        self.assertEqual(
            self.registration.parse_registration(
                container_id.encode("ascii")
            ),
            container_id,
        )

    def test_rejects_multiple_registration_lines(self):
        with self.assertRaises(ValueError):
            self.registration.parse_registration(
                (("a" * 64) + "\n" + ("b" * 64)).encode("ascii")
            )

    def test_rejects_empty_registration(self):
        with self.assertRaises(ValueError):
            self.registration.parse_registration(b"")

    def test_rejects_non_ascii_registration(self):
        with self.assertRaises(ValueError):
            self.registration.parse_registration(b"\\xff" * 64)

    def test_rejects_non_bytes_registration(self):
        with self.assertRaises(ValueError):
            self.registration.parse_registration("a" * 64)


class PgvectorRegistrationFileTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        globals()["registration"] = load_registration()

    def setUp(self):
        import os
        import tempfile

        if os.geteuid() != 0:
            self.skipTest(
                "Root-owned registration tests require isolated root execution"
            )

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-registration-"
        )
        self.addCleanup(self.temp.cleanup)

        self.directory = Path(self.temp.name)
        self.directory.chmod(0o700)

        self.fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC,
        )
        self.addCleanup(os.close, self.fd)

        self.container_id = "a" * 64
        self.registration_file = self.directory / "container-id"

    def write_registration(self, content=None, mode=0o600):
        if content is None:
            content = (self.container_id + "\n").encode("ascii")

        self.registration_file.write_bytes(content)
        self.registration_file.chmod(mode)

    def test_accepts_trusted_registration(self):
        self.write_registration()
        self.assertEqual(
            registration.read_registration(self.fd),
            self.container_id,
        )

    def test_rejects_missing_registration(self):
        with self.assertRaises(FileNotFoundError):
            registration.read_registration(self.fd)

    def test_rejects_world_readable_registration(self):
        self.write_registration(mode=0o644)
        with self.assertRaises(ValueError):
            registration.read_registration(self.fd)

    def test_rejects_symlink_registration(self):
        other = self.directory / "other"
        other.write_text(self.container_id + "\\n")
        self.registration_file.symlink_to(other)
        with self.assertRaises((ValueError, OSError)):
            registration.read_registration(self.fd)

    def test_rejects_hardlinked_registration(self):
        import os

        self.write_registration()
        os.link(
            self.registration_file,
            self.directory / "other-link",
        )

        with self.assertRaises(ValueError):
            registration.read_registration(self.fd)

    def test_rejects_directory_as_registration(self):
        self.registration_file.mkdir()
        with self.assertRaises((ValueError, OSError)):
            registration.read_registration(self.fd)

    def test_rejects_malformed_registration(self):
        self.write_registration(b"invalid\\n")
        with self.assertRaises(ValueError):
            registration.read_registration(self.fd)

    def test_rejects_oversized_registration(self):
        self.write_registration(b"a" * 4096)
        with self.assertRaises(ValueError):
            registration.read_registration(self.fd)

    def test_rejects_non_root_owned_registration(self):
        import os

        self.write_registration()
        try:
            os.chown(self.registration_file, 65534, 65534)
        except OSError as error:
            if error.errno in (1, 22):
                self.skipTest(
                    "Requested UID/GID unavailable in isolated namespace"
                )
            raise

        with self.assertRaises(ValueError):
            registration.read_registration(self.fd)


class PgvectorTrustedRegistrationTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.registration = load_registration()

    def setUp(self):
        from unittest import mock

        self.mock = mock
        self.container_id = "a" * 64

    def test_reads_registration_from_verified_directory(self):
        with self.mock.patch.object(
            self.registration,
            "open_existing_state_directory",
            create=True,
            return_value=42,
        ) as opener:
            with self.mock.patch.object(
                self.registration,
                "read_registration",
                return_value=self.container_id,
            ) as reader:
                with self.mock.patch.object(
                    self.registration.os,
                    "close",
                ) as closer:
                    result = self.registration.read_trusted_registration(
                        "/var/lib/daia/pgvector"
                    )

        self.assertEqual(result, self.container_id)
        opener.assert_called_once_with("/var/lib/daia/pgvector")
        reader.assert_called_once_with(42)
        closer.assert_called_once_with(42)

    def test_closes_directory_when_read_fails(self):
        with self.mock.patch.object(
            self.registration,
            "open_existing_state_directory",
            create=True,
            return_value=42,
        ):
            with self.mock.patch.object(
                self.registration,
                "read_registration",
                side_effect=ValueError("invalid registration"),
            ):
                with self.mock.patch.object(
                    self.registration.os,
                    "close",
                ) as closer:
                    with self.assertRaises(ValueError):
                        self.registration.read_trusted_registration(
                            "/var/lib/daia/pgvector"
                        )

        closer.assert_called_once_with(42)

    def test_rejects_untrusted_directory(self):
        with self.mock.patch.object(
            self.registration,
            "open_existing_state_directory",
            create=True,
            side_effect=ValueError("untrusted directory"),
        ):
            with self.mock.patch.object(
                self.registration,
                "read_registration",
            ) as reader:
                with self.assertRaises(ValueError):
                    self.registration.read_trusted_registration(
                        "/var/lib/daia/pgvector"
                    )

        reader.assert_not_called()


class PgvectorRegistrationPublicationTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        import os
        if os.geteuid() != 0:
            raise unittest.SkipTest("Root-owned publication tests require isolated root")
        cls.registration = load_registration()

    def setUp(self):
        import os
        import tempfile

        self.os = os
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name) / "state"
        self.directory.mkdir(mode=0o700)
        self.directory_fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC,
        )
        self.container_id = "a" * 64

    def tearDown(self):
        self.os.close(self.directory_fd)
        self.temporary.cleanup()

    def test_publishes_valid_registration(self):
        self.registration.publish_registration(
            self.directory_fd, self.container_id
        )
        self.assertEqual(
            self.registration.read_registration(self.directory_fd),
            self.container_id,
        )
        metadata = self.os.stat(
            "container-id",
            dir_fd=self.directory_fd,
            follow_symlinks=False,
        )
        self.assertEqual(metadata.st_uid, 0)
        self.assertEqual(metadata.st_gid, 0)
        self.assertEqual(metadata.st_mode & 0o777, 0o600)
        self.assertEqual(metadata.st_nlink, 1)

    def test_accepts_same_existing_identity(self):
        self.registration.publish_registration(
            self.directory_fd, self.container_id
        )
        self.registration.publish_registration(
            self.directory_fd, self.container_id
        )
        self.assertEqual(
            self.registration.read_registration(self.directory_fd),
            self.container_id,
        )

    def test_rejects_conflicting_existing_identity(self):
        self.registration.publish_registration(
            self.directory_fd, self.container_id
        )
        with self.assertRaises(ValueError):
            self.registration.publish_registration(
                self.directory_fd, "b" * 64
            )
        self.assertEqual(
            self.registration.read_registration(self.directory_fd),
            self.container_id,
        )

    def test_rejects_invalid_new_identity(self):
        with self.assertRaises(ValueError):
            self.registration.publish_registration(
                self.directory_fd, "not-a-container-id"
            )
        self.assertFalse((self.directory / "container-id").exists())

    def test_rejects_existing_symlink(self):
        (self.directory / "target").write_text(self.container_id)
        (self.directory / "container-id").symlink_to("target")
        with self.assertRaises((OSError, ValueError)):
            self.registration.publish_registration(
                self.directory_fd, self.container_id
            )

    def test_rejects_existing_world_readable_file(self):
        path = self.directory / "container-id"
        path.write_text(self.container_id)
        path.chmod(0o644)
        with self.assertRaises(ValueError):
            self.registration.publish_registration(
                self.directory_fd, self.container_id
            )

    def test_rejects_existing_hardlink(self):
        path = self.directory / "container-id"
        path.write_text(self.container_id)
        path.chmod(0o600)
        self.os.link(path, self.directory / "other-link")
        with self.assertRaises(ValueError):
            self.registration.publish_registration(
                self.directory_fd, self.container_id
            )

    def test_rejects_malformed_existing_registration(self):
        path = self.directory / "container-id"
        path.write_text("invalid")
        path.chmod(0o600)
        with self.assertRaises(ValueError):
            self.registration.publish_registration(
                self.directory_fd, self.container_id
            )

    def test_synchronizes_published_directory(self):
        from unittest import mock

        original_fsync = self.registration.os.fsync
        synchronized = []

        def record_fsync(fd):
            synchronized.append(fd)
            return original_fsync(fd)

        with mock.patch.object(
            self.registration.os,
            "fsync",
            side_effect=record_fsync,
        ):
            self.registration.publish_registration(
                self.directory_fd, self.container_id
            )

        self.assertIn(self.directory_fd, synchronized)

    def test_leaves_no_temporary_registration_files(self):
        self.registration.publish_registration(
            self.directory_fd, self.container_id
        )
        self.assertEqual(
            sorted(path.name for path in self.directory.iterdir()),
            ["container-id"],
        )


    def test_rejects_untrusted_directory_descriptor(self):
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "untrusted"
            directory.mkdir(mode=0o777)
            directory.chmod(0o777)

            fd = self.os.open(
                directory,
                self.os.O_RDONLY | self.os.O_DIRECTORY,
            )

            try:
                with self.assertRaises((OSError, ValueError)):
                    self.registration.publish_registration(
                        fd, self.container_id
                    )
            finally:
                self.os.close(fd)

            self.assertFalse((directory / "container-id").exists())

    def test_cleans_temporary_file_after_sync_failure(self):
        from unittest import mock

        original_fsync = self.registration.os.fsync
        calls = 0

        def fail_first_sync(fd):
            nonlocal calls
            calls += 1
            if calls == 1:
                raise OSError("injected file synchronization failure")
            return original_fsync(fd)

        with mock.patch.object(
            self.registration.os,
            "fsync",
            side_effect=fail_first_sync,
        ):
            with self.assertRaises(OSError):
                self.registration.publish_registration(
                    self.directory_fd, self.container_id
                )

        self.assertEqual(list(self.directory.iterdir()), [])

    def test_rejects_competing_registration(self):
        from unittest import mock

        def competing_link(src, dst, **kwargs):
            competitor = self.directory / "container-id"
            competitor.write_text("b" * 64)
            competitor.chmod(0o600)
            raise FileExistsError("simulated competing publication")

        with mock.patch.object(
            self.registration.os,
            "link",
            side_effect=competing_link,
        ):
            with self.assertRaises(ValueError):
                self.registration.publish_registration(
                    self.directory_fd, self.container_id
                )

        self.assertEqual(
            self.registration.read_registration(self.directory_fd),
            "b" * 64,
        )
        self.assertEqual(
            sorted(path.name for path in self.directory.iterdir()),
            ["container-id"],
        )



class PgvectorTrustedPublicationTests(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.registration = load_registration()

    def test_delegates_publication_through_trusted_opener(self):
        from unittest import mock

        identity = "a" * 64

        with (
            mock.patch.object(
                self.registration,
                "open_existing_state_directory",
                return_value=42,
            ) as opener,
            mock.patch.object(
                self.registration,
                "publish_registration",
            ) as publisher,
            mock.patch.object(
                self.registration.os,
                "close",
            ) as closer,
        ):
            self.registration.publish_trusted_registration(
                "/var/lib/daia/pgvector", identity
            )

        opener.assert_called_once_with("/var/lib/daia/pgvector")
        publisher.assert_called_once_with(42, identity)
        closer.assert_called_once_with(42)

    def test_rejects_invalid_identity_before_opening(self):
        from unittest import mock

        with mock.patch.object(
            self.registration,
            "open_existing_state_directory",
        ) as opener:
            with self.assertRaises(ValueError):
                self.registration.publish_trusted_registration(
                    "/var/lib/daia/pgvector", "invalid"
                )

        opener.assert_not_called()

    def test_closes_descriptor_after_publication_failure(self):
        from unittest import mock

        with (
            mock.patch.object(
                self.registration,
                "open_existing_state_directory",
                return_value=42,
            ),
            mock.patch.object(
                self.registration,
                "publish_registration",
                side_effect=ValueError("conflicting registration"),
            ),
            mock.patch.object(
                self.registration.os,
                "close",
            ) as closer,
        ):
            with self.assertRaises(ValueError):
                self.registration.publish_trusted_registration(
                    "/var/lib/daia/pgvector", "a" * 64
                )

        closer.assert_called_once_with(42)

    def test_rejects_untrusted_directory(self):
        from unittest import mock

        with (
            mock.patch.object(
                self.registration,
                "open_existing_state_directory",
                side_effect=ValueError("untrusted directory"),
            ),
            mock.patch.object(
                self.registration,
                "publish_registration",
            ) as publisher,
        ):
            with self.assertRaises(ValueError):
                self.registration.publish_trusted_registration(
                    "/tmp/untrusted", "a" * 64
                )

        publisher.assert_not_called()



class PgvectorRegistrationVerifierTests(unittest.TestCase):
    """Shutdown authorization must require matching trusted registration."""

    def test_verifier_rejects_missing_registration(self):
        from unittest.mock import patch
        registration = load_registration()
        verify_trusted_registration = registration.verify_trusted_registration

        with patch.object(
            registration, "read_trusted_registration",
            side_effect=FileNotFoundError,
        ):
            with self.assertRaises(FileNotFoundError):
                verify_trusted_registration(
                    "/var/lib/daia/pgvector", "a" * 64
                )

    def test_verifier_rejects_malformed_inspected_id(self):
        registration = load_registration()
        verify_trusted_registration = registration.verify_trusted_registration

        with self.assertRaises(ValueError):
            verify_trusted_registration(
                "/var/lib/daia/pgvector", "invalid"
            )

    def test_verifier_rejects_mismatched_registration(self):
        from unittest.mock import patch
        registration = load_registration()
        verify_trusted_registration = registration.verify_trusted_registration

        with patch.object(
            registration, "read_trusted_registration",
            return_value="a" * 64,
        ):
            with self.assertRaises(PermissionError):
                verify_trusted_registration(
                    "/var/lib/daia/pgvector", "b" * 64
                )

    def test_verifier_accepts_matching_registration(self):
        from unittest.mock import patch
        registration = load_registration()
        verify_trusted_registration = registration.verify_trusted_registration

        with patch.object(
            registration, "read_trusted_registration",
            return_value="a" * 64,
        ):
            self.assertIsNone(
                verify_trusted_registration(
                    "/var/lib/daia/pgvector", "a" * 64
                )
            )

if __name__ == "__main__":
    unittest.main()
