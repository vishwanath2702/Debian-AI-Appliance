import importlib.util
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-rag-entrypoint.py"
)
spec = importlib.util.spec_from_file_location(
    "daia_openwebui_secret_security", SOURCE
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class SecretSecurityTests(unittest.TestCase):
    def test_secure_creation_and_reuse(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                module.ensure_webui_secret_file()
                first = path.read_bytes()
                # Force the second call through the file-reuse path.
                os.environ.pop("WEBUI_SECRET_KEY", None)
                module.ensure_webui_secret_file()
                self.assertEqual(first, path.read_bytes())
                self.assertEqual(
                    os.environ["WEBUI_SECRET_KEY"],
                    first.decode("ascii"),
                )
                self.assertEqual(
                    stat.S_IMODE(path.stat().st_mode), 0o600
                )
                self.assertGreaterEqual(len(first), 32)

    def test_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target"
            target.write_text("do-not-change")
            link = Path(directory) / "secret"
            link.symlink_to(target)
            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(link)},
                clear=True,
            ):
                with self.assertRaises(OSError):
                    module.ensure_webui_secret_file()
            self.assertEqual(target.read_text(), "do-not-change")

    def test_unsafe_permissions_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            path.write_bytes(b"x" * 48)
            path.chmod(0o644)
            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                with self.assertRaises(PermissionError):
                    module.ensure_webui_secret_file()

    def test_explicit_environment_secret_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            with patch.dict(
                os.environ,
                {
                    "WEBUI_SECRET_KEY_FILE": str(path),
                    "WEBUI_SECRET_KEY": "existing-explicit-secret",
                },
                clear=True,
            ):
                module.ensure_webui_secret_file()
                self.assertFalse(path.exists())

    def test_relative_path_rejected(self):
        with patch.dict(
            os.environ,
            {"WEBUI_SECRET_KEY_FILE": "relative-secret"},
            clear=True,
        ):
            with self.assertRaises(ValueError):
                module.ensure_webui_secret_file()


    def test_secret_exported_to_upstream(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                module.ensure_webui_secret_file()
                self.assertEqual(
                    os.environ["WEBUI_SECRET_KEY"],
                    path.read_text(),
                )

    def test_existing_secret_exported_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            value = "a" * 48
            path.write_text(value)
            path.chmod(0o600)
            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                module.ensure_webui_secret_file()
                self.assertEqual(os.environ["WEBUI_SECRET_KEY"], value)
                self.assertEqual(path.read_text(), value)



    def test_concurrent_publication_preserves_one_secret(self):
        import concurrent.futures

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"

            def create_secret(_):
                with patch.dict(
                    os.environ,
                    {"WEBUI_SECRET_KEY_FILE": str(path)},
                    clear=True,
                ):
                    module.ensure_webui_secret_file()
                    return path.read_bytes()

            with concurrent.futures.ThreadPoolExecutor(
                max_workers=4
            ) as executor:
                results = list(executor.map(create_secret, range(8)))

            self.assertTrue(path.is_file())
            self.assertEqual(len(set(results)), 1)
            self.assertEqual(results[0], path.read_bytes())

    def test_atomic_publication_does_not_replace_existing_secret(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            original = b"x" * 48
            path.write_bytes(original)
            path.chmod(0o600)

            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                module.ensure_webui_secret_file()

            self.assertEqual(path.read_bytes(), original)

    def test_no_temporary_file_remains_after_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"

            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                module.ensure_webui_secret_file()

            self.assertEqual(
                sorted(p.name for p in Path(directory).iterdir()),
                ["secret"],
            )


    def test_group_writable_parent_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            original_mode = stat.S_IMODE(
                Path(directory).stat().st_mode
            )
            try:
                Path(directory).chmod(0o770)
                with patch.dict(
                    os.environ,
                    {"WEBUI_SECRET_KEY_FILE": str(path)},
                    clear=True,
                ):
                    with self.assertRaises(PermissionError):
                        module.ensure_webui_secret_file()

                self.assertFalse(path.exists())
            finally:
                Path(directory).chmod(original_mode)

    def test_other_writable_parent_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            original_mode = stat.S_IMODE(
                Path(directory).stat().st_mode
            )
            try:
                Path(directory).chmod(0o707)
                with patch.dict(
                    os.environ,
                    {"WEBUI_SECRET_KEY_FILE": str(path)},
                    clear=True,
                ):
                    with self.assertRaises(PermissionError):
                        module.ensure_webui_secret_file()

                self.assertFalse(path.exists())
            finally:
                Path(directory).chmod(original_mode)

    def test_non_writable_parent_mode_0755_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            original_mode = stat.S_IMODE(
                Path(directory).stat().st_mode
            )
            try:
                Path(directory).chmod(0o755)
                with patch.dict(
                    os.environ,
                    {"WEBUI_SECRET_KEY_FILE": str(path)},
                    clear=True,
                ):
                    module.ensure_webui_secret_file()

                self.assertTrue(path.is_file())
                self.assertEqual(
                    stat.S_IMODE(path.stat().st_mode),
                    0o600,
                )
            finally:
                Path(directory).chmod(original_mode)

    def test_existing_secret_not_modified_by_parent_rejection(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            original = b"p" * 48
            path.write_bytes(original)
            path.chmod(0o600)

            original_mode = stat.S_IMODE(
                Path(directory).stat().st_mode
            )
            try:
                Path(directory).chmod(0o777)
                with patch.dict(
                    os.environ,
                    {"WEBUI_SECRET_KEY_FILE": str(path)},
                    clear=True,
                ):
                    with self.assertRaises(PermissionError):
                        module.ensure_webui_secret_file()

                self.assertEqual(path.read_bytes(), original)
            finally:
                Path(directory).chmod(original_mode)


if __name__ == "__main__":
    unittest.main()
