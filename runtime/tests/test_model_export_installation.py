"""Installation contract for DAIA Model Exporter executables."""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
BROKER = "/usr/libexec/daia-model-export-broker"
WORKER = "/usr/libexec/daia-model-export-worker"


class ExportInstallationTests(unittest.TestCase):
    def test_polkit_broker_path_matches_contract(self):
        policy = (
            ROOT / "runtime/polkit-1/actions/org.daia.model-export.policy"
        ).read_text()

        self.assertIn(BROKER, policy)

    def test_worker_service_path_matches_contract(self):
        service = (
            ROOT / "runtime/services/daia-model-export-worker.service"
        ).read_text()

        self.assertIn("ExecStart=" + WORKER, service)

    def test_worker_has_private_managed_staging_directory(self):
        service = (
            ROOT / "runtime/services/daia-model-export-worker.service"
        ).read_text()

        self.assertIn("User=daia-export\n", service)
        self.assertIn("Group=daia-export\n", service)
        self.assertIn("ProtectSystem=strict\n", service)
        self.assertIn("ProtectHome=yes\n", service)
        self.assertIn("StateDirectory=daia-model-export\n", service)
        self.assertIn("StateDirectoryMode=0700\n", service)

    def test_both_executables_have_installation_rules(self):
        sources = [
            ROOT / "build/build-payload.sh",
            ROOT / "engine/crates/engine/src/workflow/iso.rs",
        ]

        combined = "\n".join(path.read_text() for path in sources)

        self.assertIn(
            "daia-model-export-broker",
            combined,
            "Broker executable has no verified build integration",
        )
        self.assertIn(
            "daia-model-export-worker",
            combined,
            "Worker executable has no verified build integration",
        )


    def test_exporter_configuration_has_installation_rules(self):
        payload = (ROOT / "build/build-payload.sh").read_text()
        installer = (ROOT / "runtime/modules/ai.sh").read_text()

        self.assertIn(
            '"$DAIA_RUNTIME_SOURCE/polkit-1"',
            payload,
        )
        self.assertIn(
            '"$DAIA_ROOT_TARGET/polkit-1"',
            payload,
        )

        for filename in (
            "org.daia.model-export.policy",
            "daia-model-export-worker.service",
            "daia-model-export.socket",
        ):
            with self.subTest(filename=filename):
                self.assertIn(filename, installer)

        self.assertIn(
            'install -o root -g root -m 0644',
            installer,
        )

    def test_exporter_units_are_not_activated_by_installer(self):
        installer = (ROOT / "runtime/modules/ai.sh").read_text()

        for command in (
            "systemctl enable daia-model-export",
            "systemctl start daia-model-export",
            "systemctl restart daia-model-export",
            "systemctl enable --now daia-model-export",
        ):
            with self.subTest(command=command):
                self.assertNotIn(command, installer)

        socket = (
            ROOT / "runtime/services/daia-model-export.socket"
        ).read_text()

        self.assertNotIn("[Install]", socket)


if __name__ == "__main__":
    unittest.main()
