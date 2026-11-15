import unittest
from pathlib import Path


class OllamaBridgeSystemdTests(unittest.TestCase):
    def test_service_contract(self):
        root = Path(__file__).resolve().parents[1]
        unit = (
            root / "services/daia-ollama-bridge.service"
        ).read_text()
        helper = root / "daia-ollama-bridge.py"
        self.assertTrue(helper.is_file())
        for required in (
            "Requires=docker.service ollama.service",
            "After=docker.service ollama.service",
            "ExecStart=/usr/libexec/daia-ollama-bridge.py",
            "Restart=on-failure",
            "ProtectSystem=strict",
            "RestrictAddressFamilies=AF_UNIX AF_INET",
            "WantedBy=multi-user.target",
        ):
            self.assertIn(required, unit)
        self.assertNotIn("ExecStartPre=/bin/sh", unit)
        self.assertNotIn("0.0.0.0", unit)


if __name__ == "__main__":
    unittest.main()
