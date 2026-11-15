import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "build/build-payload.sh"
PROXY = ROOT / "runtime/daia-openwebui-localhost-proxy.py"
UNIT = ROOT / "runtime/services/daia-openwebui-localhost.service"


class ProxyPackagingTests(unittest.TestCase):
    def test_proxy_executable_staged(self):
        source = BUILD.read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/daia-openwebui-localhost-proxy.py"',
            source,
        )
        self.assertIn(
            '"$DAIA_WORKSPACE/usr/libexec/daia-openwebui-localhost-proxy.py"',
            source,
        )
        self.assertIn(
            "Trusted Open WebUI localhost proxy executable staged.",
            source,
        )
        self.assertTrue(PROXY.is_file())
        self.assertTrue(PROXY.stat().st_mode & 0o111)

    def test_service_staged_without_activation(self):
        source = BUILD.read_text()
        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/services/daia-openwebui-localhost.service"',
            source,
        )
        self.assertIn(
            '"$DAIA_SYSTEMD_TARGET/daia-openwebui-localhost.service"',
            source,
        )
        self.assertIn(
            "Open WebUI localhost proxy systemd service staged without activation.",
            source,
        )
        self.assertTrue(UNIT.is_file())

    def test_payload_shell_syntax(self):
        result = subprocess.run(
            ["bash", "-n", str(BUILD)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
