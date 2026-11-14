"""Isolated Docker-mock tests for privileged Open WebUI volume verification."""

import importlib.util
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "daia-openwebui-register-container"

# Resolve the helper's local imports from the DAIA runtime directory.
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

# The privileged helper has no .py extension.
from importlib.machinery import SourceFileLoader

loader = SourceFileLoader(
    "daia_openwebui_volume_registration",
    str(HELPER),
)
spec = importlib.util.spec_from_loader(loader.name, loader)

if spec is None:
    raise RuntimeError("Cannot construct registration helper module spec")

registration = importlib.util.module_from_spec(spec)
loader.exec_module(registration)


class OpenWebUIVolumeOwnershipTests(unittest.TestCase):
    def inspect(self, labels, *, fail=False):
        commands = []

        def fake_run(args, **kwargs):
            commands.append(args)

            self.assertEqual(
                args[:3],
                ["docker", "volume", "inspect"],
            )
            self.assertEqual(args[-1], "open-webui")
            self.assertTrue(kwargs["check"])
            self.assertTrue(kwargs["text"])
            self.assertEqual(kwargs["timeout"], 15)

            if fail:
                raise subprocess.CalledProcessError(1, args)

            return subprocess.CompletedProcess(
                args,
                0,
                stdout=labels + "\n",
                stderr="",
            )

        with patch.object(registration.subprocess, "run", side_effect=fake_run):
            registration.inspect_volume_ownership()

        self.assertEqual(len(commands), 1)

    def test_accepts_exact_daia_ownership(self):
        self.inspect("openwebui\ttrue")

    def test_rejects_wrong_component(self):
        with self.assertRaises(PermissionError):
            self.inspect("pgvector\ttrue")

    def test_rejects_unmanaged_volume(self):
        with self.assertRaises(PermissionError):
            self.inspect("openwebui\tfalse")

    def test_rejects_missing_component(self):
        with self.assertRaises(PermissionError):
            self.inspect("\ttrue")

    def test_rejects_missing_managed_label(self):
        with self.assertRaises(PermissionError):
            self.inspect("openwebui\t")

    def test_rejects_extra_metadata(self):
        with self.assertRaises(PermissionError):
            self.inspect("openwebui\ttrue\textra")

    def test_rejects_multiple_lines(self):
        with self.assertRaises(PermissionError):
            self.inspect("openwebui\ttrue\nopenwebui\ttrue")

    def test_rejects_docker_inspection_failure(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inspect("openwebui\ttrue", fail=True)


if __name__ == "__main__":
    unittest.main()
