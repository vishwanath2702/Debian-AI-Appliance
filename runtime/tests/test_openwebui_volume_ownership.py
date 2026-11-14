"""Open WebUI persistent-volume ownership source contracts."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "runtime/modules/openwebui.sh"


class OpenWebUIVolumeOwnershipTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = MODULE.read_text()
        cls.install = cls.source.split(
            "openwebui_install() {", 1
        )[1].split("\nopenwebui_verify() {", 1)[0]

    def test_volume_ownership_inspection_exists(self):
        self.assertIn("docker volume inspect", self.source)
        self.assertIn('org.daia.component', self.source)
        self.assertIn('org.daia.managed', self.source)

    def test_new_volume_receives_ownership_labels(self):
        self.assertIn(
            "--label org.daia.component=openwebui", self.install
        )
        self.assertIn(
            "--label org.daia.managed=true", self.install
        )

    def test_existing_volume_requires_ownership_verification(self):
        self.assertIn("openwebui_ensure_volume || return 1", self.install)

        helper = self.source.split(
            "openwebui_ensure_volume() {", 1
        )[1].split("\nopenwebui_install() {", 1)[0]

        self.assertIn("docker volume ls", helper)
        self.assertIn("openwebui_volume_owned", helper)
        self.assertIn("docker volume create", helper)
        self.assertGreaterEqual(
            helper.count('openwebui_volume_owned "$volume_name"'),
            2,
        )

    def test_volume_verified_before_container_creation(self):
        verification = self.install.index("openwebui_ensure_volume || return 1")
        creation = self.install.index("docker container create")
        self.assertLess(verification, creation)

    def test_no_automatic_volume_removal(self):
        self.assertNotIn("docker volume rm", self.source)
        self.assertNotIn("docker volume prune", self.source)


if __name__ == "__main__":
    unittest.main()
