"""Open WebUI RAG wrapper packaging contract."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "build/build-payload.sh"
WRAPPER = ROOT / "runtime/daia-openwebui-rag-entrypoint.py"


class OpenWebUIRAGPackagingTests(unittest.TestCase):
    def test_wrapper_exists_and_is_executable(self):
        self.assertTrue(WRAPPER.is_file())
        self.assertTrue(WRAPPER.stat().st_mode & 0o111)

    def test_registration_helper_uses_trusted_descriptor_loading(self):
        helper = (
            ROOT / "runtime/daia-openwebui-register-container"
        ).read_text()

        self.assertIn("def _bind_trusted_dependencies():", helper)
        self.assertIn("load_trusted_runtime_modules", helper)
        self.assertIn("os.O_NOFOLLOW", helper)
        self.assertIn("dir_fd=directory_fd", helper)
        self.assertIn("expected_uid=0", helper)
        self.assertNotIn('sys.path.insert(0, "/opt/daia")', helper)

    def test_registration_helper_loads_before_privileged_main(self):
        helper = (
            ROOT / "runtime/daia-openwebui-register-container"
        ).read_text()

        self.assertIn(
            "_bind_trusted_dependencies()\n"
            "        raise SystemExit(main())",
            helper,
        )

    def test_trusted_loader_is_staged_to_libexec(self):
        source = BUILD.read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/openwebui_trusted_imports.py"',
            source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/openwebui_trusted_imports.py"',
            source,
        )

    def test_registration_helper_exists_and_is_executable(self):
        helper = ROOT / "runtime/daia-openwebui-register-container"
        self.assertTrue(helper.is_file())
        self.assertTrue(helper.stat().st_mode & 0o111)

    def test_registration_helper_staged_to_libexec(self):
        source = BUILD.read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-openwebui-register-container"',
            source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-openwebui-register-container"',
            source,
        )

    def test_registration_module_staged_to_runtime_root(self):
        module = ROOT / "runtime/openwebui_registration.py"
        self.assertTrue(module.is_file())

        source = BUILD.read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/openwebui_registration.py"',
            source,
        )
        self.assertIn(
            '"$DAIA_ROOT_TARGET/openwebui_registration.py"',
            source,
        )

    def test_wrapper_is_staged_to_libexec(self):
        source = BUILD.read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-openwebui-rag-entrypoint.py"',
            source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-openwebui-rag-entrypoint.py"',
            source,
        )


if __name__ == "__main__":
    unittest.main()
