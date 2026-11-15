import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
PROXY = ROOT / "runtime/daia-openwebui-localhost-proxy.py"
UNIT = ROOT / "runtime/services/daia-openwebui-localhost.service"

spec = importlib.util.spec_from_file_location(
    "daia_webui_proxy_under_test", PROXY
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ProxyTests(unittest.TestCase):
    def test_loopback_only(self):
        self.assertEqual(module.LISTEN, ("127.0.0.1", 3000))
        self.assertEqual(module.WEBUI_PORT, 3000)

    def test_missing_bridge_fails_closed(self):
        with patch.object(module, "BRIDGE", Path("/missing/bridge")):
            with self.assertRaises(PermissionError):
                module.trusted_webui()

    def verified_endpoint(self, health="healthy"):
        from unittest.mock import MagicMock

        fake_bridge = MagicMock()
        fake_bridge.trusted_endpoint.return_value = (
            "172.25.0.1", "172.25.0.4"
        )
        fake_bridge.WEBUI = "open-webui"
        fake_bridge.docker_json.return_value = {
            "State": {
                "Running": True,
                "Paused": False,
                "Restarting": False,
                "Health": {"Status": health},
            }
        }

        fake_spec = MagicMock()
        with (
            patch.object(module, "BRIDGE", PROXY),
            patch.object(
                module.importlib.util,
                "spec_from_file_location",
                return_value=fake_spec,
            ),
            patch.object(
                module.importlib.util,
                "module_from_spec",
                return_value=fake_bridge,
            ),
        ):
            return module.trusted_webui()

    def test_healthy_webui_resolves_verified_destination(self):
        self.assertEqual(
            self.verified_endpoint(),
            ("172.25.0.4", 3000),
        )

    def test_unhealthy_webui_fails_closed(self):
        with self.assertRaises(PermissionError):
            self.verified_endpoint("unhealthy")

    def test_missing_health_fails_closed(self):
        with self.assertRaises(PermissionError):
            self.verified_endpoint(None)

    def test_untrusted_webui_identity_fails_closed(self):
        from unittest.mock import MagicMock

        fake_bridge = MagicMock()
        fake_bridge.trusted_endpoint.side_effect = PermissionError(
            "Untrusted registration"
        )
        fake_spec = MagicMock()

        with (
            patch.object(module, "BRIDGE", PROXY),
            patch.object(
                module.importlib.util,
                "spec_from_file_location",
                return_value=fake_spec,
            ),
            patch.object(
                module.importlib.util,
                "module_from_spec",
                return_value=fake_bridge,
            ),
        ):
            with self.assertRaises(PermissionError):
                module.trusted_webui()

    def test_systemd_contract(self):
        text = UNIT.read_text()
        self.assertIn(
            "ExecStart=/usr/libexec/daia-openwebui-localhost-proxy.py",
            text,
        )
        self.assertIn("Restart=on-failure", text)
        self.assertIn("ProtectSystem=strict", text)
        self.assertIn(
            "RestrictAddressFamilies=AF_UNIX AF_INET",
            text,
        )
        self.assertNotIn("ExecStartPre=docker", text)

    def test_no_hardcoded_docker_subnet(self):
        source = PROXY.read_text()
        self.assertNotIn("172.18.", source)
        self.assertIn("bridge.trusted_endpoint()", source)
        self.assertIn('!= "healthy"', source)


if __name__ == "__main__":
    unittest.main()
