import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"
RUNTIME = ROOT / "runtime/runtime.sh"


class ActivationTests(unittest.TestCase):
    def test_activation_after_verification(self):
        text = RUNTIME.read_text()
        start = text.index("runtime_module_execute()")
        end = text.index("\n}", start)
        section = text[start:end]
        self.assertLess(
            section.index('"$lifecycle_operation" || return $?'),
            section.index("openwebui_activate_proxies || return 1"),
        )
        self.assertIn(
            'if [[ "$module_name" == "openwebui" ]]',
            section,
        )

    def test_both_services_activated(self):
        text = MODULE.read_text()
        start = text.index("openwebui_activate_proxies()")
        end = text.index("\n}", start)
        section = text[start:end]
        for name in (
            "daia-ollama-bridge.service",
            "daia-openwebui-localhost.service",
        ):
            self.assertIn(name, section)
        self.assertIn('systemctl enable "$service"', section)
        self.assertNotIn('systemctl enable --now', section)
        self.assertNotIn('systemctl is-active --quiet', section)
        self.assertIn("systemctl daemon-reload", section)

    def test_shell_syntax(self):
        for path in (MODULE, RUNTIME):
            with self.subTest(path=path.name):
                result = subprocess.run(
                    ["bash", "-n", str(path)],
                    capture_output=True, text=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
