import importlib.util
import json
import unittest
from pathlib import Path
from unittest.mock import patch

PATH = Path(__file__).resolve().parents[1] / "daia-ollama-bridge.py"
SPEC = importlib.util.spec_from_file_location("daia_ollama_bridge", PATH)
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)

PG = "a" * 64
TIKA = "c" * 64
WEBUI = "b" * 64
NETWORK_ID = "d" * 64

def fixture():
    network = {
        "Name": "daia-rag-internal",
        "Id": NETWORK_ID,
        "Driver": "bridge",
        "Internal": True,
        "Labels": {
            "org.daia.component": "rag",
            "org.daia.managed": "true",
        },
        "IPAM": {
            "Config": [{
                "Subnet": "172.25.0.0/24",
                "Gateway": "172.25.0.1",
            }]
        },
        "Containers": {
            PG: {"IPv4Address": "172.25.0.2/24"},
            TIKA: {"IPv4Address": "172.25.0.3/24"},
            WEBUI: {"IPv4Address": "172.25.0.4/24"},
        },
    }
    webui = {
        "Id": WEBUI,
        "NetworkSettings": {
            "Networks": {
                "daia-rag-internal": {
                    "NetworkID": NETWORK_ID,
                    "IPAddress": "172.25.0.4",
                }
            }
        },
    }
    return network, webui


def run_case(network=None, webui=None, tika=True, verify=True):
    baseline_network, baseline_webui = fixture()
    network = baseline_network if network is None else network
    webui = baseline_webui if webui is None else webui

    def docker_json(args):
        if args[:2] == ["network", "inspect"]:
            return network
        if args[:2] == ["container", "inspect"]:
            return webui
        raise AssertionError(args)

    def command(args, timeout=20):
        if args[:3] == ["docker", "container", "ls"]:
            names = ["daia-pgvector", "open-webui"]
            if tika:
                names.append("daia-tika")
            return chr(10).join(names) + chr(10)
        if args[0] == module.TIKA_VERIFY:
            return TIKA + chr(10)
        if args[0] == module.VERIFY:
            if not verify:
                raise PermissionError("untrusted")
            return WEBUI + chr(10)
        raise AssertionError(args)

    with patch.object(module.os, "geteuid", return_value=0), \
         patch.object(module, "docker_json", side_effect=docker_json), \
         patch.object(module, "command", side_effect=command):
        return module.trusted_endpoint()


class OllamaBridgeTests(unittest.TestCase):
    def test_dynamic_gateway_and_webui_ip(self):
        self.assertEqual(
            run_case(), ("172.25.0.1", "172.25.0.4")
        )

    def test_rejects_noninternal_network(self):
        network, webui = fixture()
        network["Internal"] = False
        with self.assertRaises(PermissionError):
            run_case(network, webui)

    def test_rejects_untrusted_network_label(self):
        network, webui = fixture()
        network["Labels"]["org.daia.managed"] = "false"
        with self.assertRaises(PermissionError):
            run_case(network, webui)

    def test_rejects_changed_network_identity(self):
        network, webui = fixture()
        webui["NetworkSettings"]["Networks"]["daia-rag-internal"][
            "NetworkID"
        ] = "e" * 64
        with self.assertRaises(PermissionError):
            run_case(network, webui)

    def test_rejects_mismatched_member_address(self):
        network, webui = fixture()
        network["Containers"][WEBUI]["IPv4Address"] = "172.25.0.99/24"
        with self.assertRaises(PermissionError):
            run_case(network, webui)

    def test_rejects_untrusted_registration(self):
        with self.assertRaises(PermissionError):
            run_case(verify=False)

    def test_rejects_unexpected_webui_network(self):
        network, webui = fixture()
        webui["NetworkSettings"]["Networks"]["other"] = {}
        with self.assertRaises(PermissionError):
            run_case(network, webui)

    def test_accepts_absent_tika_when_membership_is_valid(self):
        network, webui = fixture()
        network["Containers"].pop(TIKA)
        self.assertEqual(
            run_case(network, webui, tika=False),
            ("172.25.0.1", "172.25.0.4"),
        )


    def test_rejects_malformed_member_address(self):
        network, webui = fixture()
        network["Containers"][WEBUI]["IPv4Address"] = "invalid"
        with self.assertRaises(PermissionError):
            run_case(network, webui)

    def test_rejects_missing_member_address(self):
        network, webui = fixture()
        del network["Containers"][WEBUI]["IPv4Address"]
        with self.assertRaises(PermissionError):
            run_case(network, webui)

    def test_rejects_malformed_docker_json(self):
        with patch.object(
            module, "command", return_value="not-json"
        ):
            with self.assertRaises(PermissionError):
                module.docker_json(["network", "inspect", module.NETWORK])

    def test_rejects_nonobject_docker_response(self):
        with patch.object(
            module, "command", return_value="[null]"
        ):
            with self.assertRaises(PermissionError):
                module.docker_json(["network", "inspect", module.NETWORK])

    def test_unauthorized_peer_never_reaches_ollama(self):
        class DummyRequest:
            pass

        with patch.object(
            module, "trusted_endpoint",
            return_value=("172.25.0.1", "172.25.0.4")
        ), patch.object(
            module.socket, "create_connection"
        ) as connect:
            module.Relay(
                DummyRequest(),
                ("172.25.0.99", 12345),
                object(),
            )
            connect.assert_not_called()

    def test_failed_authorization_never_reaches_ollama(self):
        class DummyRequest:
            pass

        with patch.object(
            module, "trusted_endpoint",
            side_effect=PermissionError("untrusted")
        ), patch.object(
            module.socket, "create_connection"
        ) as connect:
            module.Relay(
                DummyRequest(),
                ("172.25.0.4", 12345),
                object(),
            )
            connect.assert_not_called()


    def test_gateway_discovery_without_webui(self):
        network, _ = fixture()

        def inspect(args):
            self.assertEqual(
                args, ["network", "inspect", module.NETWORK]
            )
            return network

        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "docker_json", side_effect=inspect),
        ):
            found, subnet, gateway = module.trusted_gateway()

        self.assertEqual(found["Id"], NETWORK_ID)
        self.assertEqual(str(subnet), "172.25.0.0/24")
        self.assertEqual(str(gateway), "172.25.0.1")

    def test_gateway_rejects_missing_ipam(self):
        network, _ = fixture()
        del network["IPAM"]
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "docker_json", return_value=network),
        ):
            with self.assertRaises(PermissionError):
                module.trusted_gateway()

    def test_gateway_rejects_public_network(self):
        network, _ = fixture()
        network["Internal"] = False
        with (
            patch.object(module.os, "geteuid", return_value=0),
            patch.object(module, "docker_json", return_value=network),
        ):
            with self.assertRaises(PermissionError):
                module.trusted_gateway()

    def test_gateway_requires_root(self):
        with patch.object(module.os, "geteuid", return_value=1000):
            with self.assertRaises(PermissionError):
                module.trusted_gateway()


if __name__ == "__main__":
    unittest.main()
