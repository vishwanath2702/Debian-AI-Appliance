"""SearXNG firstboot activation contract."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "runtime/firstboot.sh").read_text()


class ActivationTests(unittest.TestCase):

    def test_runner_invoked(self):
        self.assertIn("searxng_firstboot_runner.run()", SOURCE)

    def test_isolated_python(self):
        self.assertIn("python3 -I -", SOURCE)

    def test_trusted_config_parser(self):
        self.assertIn("searxng_config.enabled()", SOURCE)

    def test_disabled_feature_noop(self):
        self.assertIn(
            "if (( status == 2 )); then\n        return 0",
            SOURCE,
        )

    def test_activation_precedes_completion(self):
        main = SOURCE.split("\nmain()\n", 1)[1]
        self.assertLess(
            main.index("provision_searxng_firstboot"),
            main.index("mark_firstboot_complete"),
        )

    def test_root_guard(self):
        self.assertIn("(( EUID == 0 )) || return 1", SOURCE)

    def test_entrypoint_permissions(self):
        self.assertIn('"0:0:755"', SOURCE)


if __name__ == "__main__":
    unittest.main()
