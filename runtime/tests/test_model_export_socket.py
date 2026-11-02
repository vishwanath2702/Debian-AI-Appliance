"""Security contract for the inactive DAIA Model Export socket."""

import configparser
from pathlib import Path
import unittest


SOCKET = (
    Path(__file__).resolve().parents[1]
    / "services/daia-model-export.socket"
)


class ExportSocketTests(unittest.TestCase):
    def setUp(self):
        self.config = configparser.ConfigParser(interpolation=None)
        self.assertTrue(self.config.read(SOCKET))
        self.socket = self.config["Socket"]

    def test_private_seqpacket_endpoint(self):
        self.assertEqual(
            self.socket.get("ListenSequentialPacket"),
            "/run/daia-model-export/worker.sock",
        )
        self.assertNotIn("ListenStream", self.socket)
        self.assertNotIn("ListenDatagram", self.socket)

    def test_root_owned_private_socket(self):
        self.assertEqual(self.socket.get("SocketUser"), "root")
        self.assertEqual(self.socket.get("SocketGroup"), "root")
        self.assertEqual(self.socket.get("SocketMode"), "0600")
        self.assertEqual(self.socket.get("DirectoryMode"), "0700")

    def test_single_supervised_worker(self):
        self.assertEqual(self.socket.get("Accept"), "no")
        self.assertEqual(
            self.socket.get("Service"),
            "daia-model-export-worker.service",
        )
        self.assertEqual(
            self.socket.get("FileDescriptorName"),
            "daia-model-export.socket",
        )

    def test_not_automatically_enabled(self):
        self.assertNotIn("Install", self.config)


if __name__ == "__main__":
    unittest.main()
