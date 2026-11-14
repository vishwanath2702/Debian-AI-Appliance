"""Fail-closed privileged Python import trust contracts."""

import os
import stat
import tempfile
import unittest
from pathlib import Path

RUNTIME = Path(__file__).resolve().parents[1]
VALIDATOR = RUNTIME / "openwebui_trusted_imports.py"

MODULES = (
    "openwebui_registration.py",
    "pgvector_registration.py",
    "pgvector_credentials.py",
)


class TrustedImportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name) / "daia"
        self.directory.mkdir(mode=0o755)

        fixtures = {
            "pgvector_credentials.py": (
                "VALUE = 'credentials'\n"
            ),
            "pgvector_registration.py": (
                "from pgvector_credentials import VALUE as CREDENTIALS\n"
                "VALUE = 'registration:' + CREDENTIALS\n"
            ),
            "openwebui_registration.py": (
                "from pgvector_registration import VALUE as REGISTRATION\n"
                "VALUE = 'openwebui:' + REGISTRATION\n"
            ),
        }

        for name in MODULES:
            (self.directory / name).write_text(fixtures[name])
            (self.directory / name).chmod(0o644)

    def load_validator(self):
        import importlib.util

        if not VALIDATOR.is_file() or VALIDATOR.is_symlink():
            self.fail("Trusted import validator is missing or unsafe")

        spec = importlib.util.spec_from_file_location(
            "openwebui_trusted_imports_under_test", VALIDATOR
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def validate(self):
        module = self.load_validator()
        return module.validate_trusted_runtime_imports(
            self.directory,
            expected_uid=os.geteuid(),
        )

    def test_accepts_trusted_directory_and_modules(self):
        self.assertIsNone(self.validate())

    def test_rejects_group_writable_ancestor(self):
        parent = self.directory.parent
        original_mode = stat.S_IMODE(parent.stat().st_mode)
        try:
            parent.chmod(0o777)
            with self.assertRaises((PermissionError, ValueError, OSError)):
                self.validate()
        finally:
            parent.chmod(original_mode)

    def test_rejects_symlinked_ancestor(self):
        alias = self.directory.parent / "alias"
        alias.symlink_to(self.directory, target_is_directory=True)

        module = self.load_validator()
        with self.assertRaises((PermissionError, ValueError, OSError)):
            module.validate_trusted_runtime_imports(
                alias,
                expected_uid=os.geteuid(),
            )

    def test_descriptor_loader_returns_required_modules(self):
        module = self.load_validator()
        self.assertTrue(
            callable(
                getattr(module, "load_trusted_runtime_modules", None)
            ),
            "Descriptor-bound loader is missing",
        )

        loaded = module.load_trusted_runtime_modules(
            self.directory,
            expected_uid=os.geteuid(),
        )

        self.assertEqual(
            set(loaded),
            {
                "pgvector_credentials",
                "pgvector_registration",
                "openwebui_registration",
            },
        )

    def test_descriptor_source_survives_path_replacement(self):
        module = self.load_validator()
        loader = getattr(module, "read_trusted_module_source", None)
        self.assertTrue(
            callable(loader),
            "Descriptor-backed source reader is missing",
        )

        target = self.directory / MODULES[0]
        target.write_text("VALUE = 'trusted'\\n")
        target.chmod(0o644)

        directory_fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
        )
        try:
            file_fd = os.open(
                MODULES[0],
                os.O_RDONLY | os.O_NOFOLLOW,
                dir_fd=directory_fd,
            )
            try:
                replacement = self.directory / "replacement.py"
                replacement.write_text("VALUE = 'untrusted'\\n")
                replacement.chmod(0o644)
                original = self.directory / 'original.py'
                os.rename(target, original)
                os.replace(replacement, target)

                content = loader(
                    file_fd,
                    expected_uid=os.geteuid(),
                )
                self.assertIn(b"VALUE = 'trusted'", content)
                self.assertNotIn(b"VALUE = 'untrusted'", content)
            finally:
                os.close(file_fd)
        finally:
            os.close(directory_fd)

    def test_descriptor_source_rejects_symlink(self):
        module = self.load_validator()
        loader = getattr(module, "read_trusted_module_source", None)
        self.assertTrue(
            callable(loader),
            "Descriptor-backed source reader is missing",
        )

        target = self.directory / MODULES[0]
        target.unlink()
        target.symlink_to(self.directory / MODULES[1])

        directory_fd = os.open(
            self.directory,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
        )
        try:
            with self.assertRaises(OSError):
                file_fd = os.open(
                    MODULES[0],
                    os.O_RDONLY | os.O_NOFOLLOW,
                    dir_fd=directory_fd,
                )
                os.close(file_fd)
        finally:
            os.close(directory_fd)

    def test_descriptor_loader_rejects_untrusted_module(self):
        module = self.load_validator()
        self.assertTrue(
            callable(
                getattr(module, "load_trusted_runtime_modules", None)
            ),
            "Descriptor-bound loader is missing",
        )

        (self.directory / MODULES[0]).chmod(0o666)

        with self.assertRaises((PermissionError, ValueError, OSError)):
            module.load_trusted_runtime_modules(
                self.directory,
                expected_uid=os.geteuid(),
            )

    def test_rejects_group_writable_module(self):
        (self.directory / MODULES[0]).chmod(0o664)
        with self.assertRaises((PermissionError, ValueError, OSError)):
            self.validate()

    def test_rejects_world_writable_module(self):
        (self.directory / MODULES[1]).chmod(0o666)
        with self.assertRaises((PermissionError, ValueError, OSError)):
            self.validate()

    def test_rejects_symlinked_module(self):
        target = self.directory / MODULES[0]
        target.unlink()
        target.symlink_to(self.directory / MODULES[1])
        with self.assertRaises((PermissionError, ValueError, OSError)):
            self.validate()

    def test_rejects_group_writable_directory(self):
        self.directory.chmod(0o775)
        with self.assertRaises((PermissionError, ValueError, OSError)):
            self.validate()

    def test_rejects_untrusted_module_owner(self):
        module = self.load_validator()
        with self.assertRaises((PermissionError, ValueError, OSError)):
            module.validate_trusted_runtime_imports(
                self.directory,
                expected_uid=os.geteuid() + 1,
            )


if __name__ == "__main__":
    unittest.main()
