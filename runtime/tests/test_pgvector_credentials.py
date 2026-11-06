"""Security regression tests for DAIA pgvector credentials."""

import importlib.util
from pathlib import Path
import unittest


HELPER = Path(__file__).resolve().parents[1] / "pgvector_credentials.py"

spec = importlib.util.spec_from_file_location(
    "daia_pgvector_credentials",
    HELPER,
)
credentials = importlib.util.module_from_spec(spec)
spec.loader.exec_module(credentials)


class PgvectorCredentialPathTests(unittest.TestCase):

    def test_accepts_normalized_absolute_path(self):
        self.assertEqual(
            credentials.validate_state_path(
                "/var/lib/daia/pgvector"
            ),
            ("var", "lib", "daia", "pgvector"),
        )

    def test_rejects_relative_path(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.validate_state_path("var/lib/daia")

    def test_rejects_filesystem_root(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.validate_state_path("/")

    def test_rejects_parent_traversal(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.validate_state_path(
                "/var/lib/../pgvector"
            )

    def test_rejects_current_directory_component(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.validate_state_path(
                "/var/./lib/pgvector"
            )

    def test_rejects_duplicate_separator(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.validate_state_path(
                "/var//lib/pgvector"
            )

    def test_rejects_trailing_separator(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.validate_state_path(
                "/var/lib/pgvector/"
            )

    def test_rejects_empty_path(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.validate_state_path("")


class PgvectorCredentialFileTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-test-"
        )
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY,
        )
        self.addCleanup(os.close, self.fd)

    def write_password(self, content, mode=0o600):
        path = self.directory / "postgres-password"
        path.write_bytes(content)
        path.chmod(mode)
        return path

    def read_with_root_metadata(self):
        from unittest.mock import patch
        import os

        original_fstat = os.fstat

        def root_metadata(fd):
            metadata = original_fstat(fd)

            class Metadata:
                pass

            result = Metadata()
            for name in (
                "st_mode", "st_size", "st_nlink",
                "st_uid", "st_gid",
            ):
                setattr(result, name, getattr(metadata, name))

            result.st_uid = 0
            result.st_gid = 0
            return result

        with patch.object(
            credentials.os,
            "fstat",
            side_effect=root_metadata,
        ):
            return credentials.read_existing_credential(self.fd)

    def test_accepts_secure_existing_credential(self):
        password = "a" * 64
        self.write_password((password + "\n").encode())
        self.assertEqual(
            self.read_with_root_metadata(),
            password,
        )

    def test_rejects_world_readable_credential(self):
        self.write_password(b"a" * 64, mode=0o644)
        with self.assertRaises(credentials.CredentialError):
            self.read_with_root_metadata()

    def test_rejects_symlink_credential(self):
        target = self.directory / "external"
        target.write_bytes(b"a" * 64)
        (self.directory / "postgres-password").symlink_to(target)

        with self.assertRaises(OSError):
            self.read_with_root_metadata()

    def test_rejects_additional_hard_link(self):
        import os

        path = self.write_password(b"a" * 64)
        os.link(path, self.directory / "second-link")

        with self.assertRaises(credentials.CredentialError):
            self.read_with_root_metadata()

    def test_rejects_invalid_password_contents(self):
        self.write_password(b"Z" * 64)

        with self.assertRaises(credentials.CredentialError):
            self.read_with_root_metadata()

    def test_rejects_invalid_password_length(self):
        self.write_password(b"a" * 63)

        with self.assertRaises(credentials.CredentialError):
            self.read_with_root_metadata()

    def test_pgpass_format_matches_database_contract(self):
        password = "b" * 64

        self.assertEqual(
            credentials.format_pgpass_entry(password),
            (
                "127.0.0.1:5432:daia_rag:daia_rag:"
                + password + "\n"
            ).encode("ascii"),
        )

    def test_pgpass_rejects_invalid_password(self):
        with self.assertRaises(credentials.CredentialError):
            credentials.format_pgpass_entry("invalid")


class PgvectorCredentialCreationTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgvector-create-"
        )
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)

        self.fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY,
        )
        self.addCleanup(os.close, self.fd)

    def provision(self):
        import os
        from unittest.mock import patch

        real_fstat = os.fstat

        def root_metadata(fd):
            metadata = real_fstat(fd)

            values = {
                name: getattr(metadata, name)
                for name in (
                    "st_mode", "st_size", "st_nlink",
                    "st_uid", "st_gid",
                )
            }
            values["st_uid"] = 0
            values["st_gid"] = 0

            return type("Metadata", (), values)()

        with patch.object(
            credentials.os,
            "fstat",
            side_effect=root_metadata,
        ):
            return credentials.create_or_reuse_credential(self.fd)

    def test_creates_secure_credential(self):
        import os

        password = self.provision()
        path = self.directory / "postgres-password"

        self.assertEqual(len(password), 64)
        self.assertTrue(path.is_file())
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(path.stat().st_nlink, 1)
        self.assertEqual(
            path.read_bytes(),
            (password + chr(10)).encode("ascii"),
        )

        self.assertEqual(
            sorted(path.name for path in self.directory.iterdir()),
            ["postgres-password"],
        )

    def test_reuses_existing_credential(self):
        first = self.provision()
        second = self.provision()

        self.assertEqual(first, second)
        self.assertEqual(
            len(list(self.directory.iterdir())),
            1,
        )

    def test_rejects_invalid_existing_credential(self):
        path = self.directory / "postgres-password"
        path.write_bytes(b"invalid")
        path.chmod(0o600)

        with self.assertRaises(credentials.CredentialError):
            self.provision()

        self.assertEqual(path.read_bytes(), b"invalid")


class PgvectorCredentialConcurrencyTests(unittest.TestCase):

    def test_concurrent_creation_preserves_one_credential(self):
        import concurrent.futures
        import os
        import tempfile
        from unittest.mock import patch

        with tempfile.TemporaryDirectory(
            prefix="daia-pgvector-concurrent-"
        ) as directory:
            root = Path(directory)
            real_fstat = os.fstat

            def root_metadata(fd):
                metadata = real_fstat(fd)

                values = {
                    name: getattr(metadata, name)
                    for name in (
                        "st_mode", "st_size", "st_nlink",
                        "st_uid", "st_gid",
                    )
                }

                values["st_uid"] = 0
                values["st_gid"] = 0
                return type("Metadata", (), values)()

            def provision(_):
                fd = os.open(
                    root,
                    os.O_RDONLY | os.O_DIRECTORY,
                )
                try:
                    return credentials.create_or_reuse_credential(fd)
                finally:
                    os.close(fd)

            with patch.object(
                credentials.os,
                "fstat",
                side_effect=root_metadata,
            ):
                with concurrent.futures.ThreadPoolExecutor(
                    max_workers=8
                ) as executor:
                    passwords = list(executor.map(
                        provision,
                        range(8),
                    ))

            self.assertEqual(len(passwords), 8)
            self.assertEqual(len(set(passwords)), 1)

            password = passwords[0]
            path = root / "postgres-password"

            self.assertTrue(path.is_file())
            self.assertEqual(
                path.read_bytes(),
                (password + chr(10)).encode("ascii"),
            )
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(path.stat().st_nlink, 1)

            self.assertEqual(
                sorted(item.name for item in root.iterdir()),
                ["postgres-password"],
            )


class PgvectorPgpassSecurityTests(unittest.TestCase):

    def setUp(self):
        import os
        import tempfile

        self.temp = tempfile.TemporaryDirectory(
            prefix="daia-pgpass-security-"
        )
        self.addCleanup(self.temp.cleanup)

        self.directory = Path(self.temp.name)
        self.fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY,
        )
        self.addCleanup(os.close, self.fd)

        self.password = "c" * 64
        password_file = self.directory / "postgres-password"
        password_file.write_bytes(
            (self.password + chr(10)).encode("ascii")
        )
        password_file.chmod(0o600)

    def publish(self):
        import os
        from unittest.mock import patch

        real_fstat = os.fstat

        def root_metadata(fd):
            metadata = real_fstat(fd)

            values = {
                name: getattr(metadata, name)
                for name in (
                    "st_mode", "st_size", "st_nlink",
                    "st_uid", "st_gid",
                )
            }

            values["st_uid"] = 0
            values["st_gid"] = 0

            return type("Metadata", (), values)()

        with patch.object(
            credentials.os,
            "fstat",
            side_effect=root_metadata,
        ):
            credentials.publish_pgpass_file(self.fd)

    def expected_content(self):
        return credentials.format_pgpass_entry(self.password)

    def test_creates_secure_pgpass(self):
        self.publish()

        path = self.directory / "pgpass"
        self.assertEqual(path.read_bytes(), self.expected_content())
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(path.stat().st_nlink, 1)

    def test_reuses_existing_pgpass(self):
        self.publish()

        path = self.directory / "pgpass"
        original_inode = path.stat().st_ino

        self.publish()

        self.assertEqual(path.stat().st_ino, original_inode)
        self.assertEqual(path.read_bytes(), self.expected_content())

    def test_rejects_mismatched_pgpass(self):
        path = self.directory / "pgpass"
        path.write_bytes(b"incorrect")
        path.chmod(0o600)

        with self.assertRaises(credentials.CredentialError):
            self.publish()

        self.assertEqual(path.read_bytes(), b"incorrect")

    def test_rejects_world_readable_pgpass(self):
        path = self.directory / "pgpass"
        path.write_bytes(self.expected_content())
        path.chmod(0o644)

        with self.assertRaises(credentials.CredentialError):
            self.publish()

    def test_rejects_symlink_pgpass(self):
        external = self.directory / "external"
        external.write_bytes(self.expected_content())

        (self.directory / "pgpass").symlink_to(external)

        with self.assertRaises(OSError):
            self.publish()

        self.assertEqual(external.read_bytes(), self.expected_content())

    def test_rejects_additional_hard_link(self):
        import os

        path = self.directory / "pgpass"
        path.write_bytes(self.expected_content())
        path.chmod(0o600)

        os.link(path, self.directory / "additional-link")

        with self.assertRaises(credentials.CredentialError):
            self.publish()


if __name__ == "__main__":
    unittest.main()
