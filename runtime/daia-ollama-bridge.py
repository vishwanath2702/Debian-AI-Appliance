#!/usr/bin/env python3
"""Private, fail-closed Open WebUI to host Ollama TCP bridge."""

import ipaddress
import json
import os
import re
import socket
import socketserver
import subprocess
import sys
import threading

NETWORK = "daia-rag-internal"
WEBUI = "open-webui"
VERIFY = "/usr/libexec/daia-openwebui-register-container"
TIKA_VERIFY = "/usr/libexec/daia-tika-register-container"
OLLAMA = ("127.0.0.1", 11434)
IDENTITY = re.compile(r"[0-9a-f]{64}")


def command(args, timeout=20):
    result = subprocess.run(
        args, capture_output=True, text=True,
        check=False, timeout=timeout,
    )
    if result.returncode != 0:
        raise PermissionError("Required RAG verification failed")
    return result.stdout


def docker_json(args):
    try:
        data = json.loads(command(["docker", *args]))
    except (ValueError, TypeError) as exc:
        raise PermissionError("Invalid Docker inspection JSON") from exc
    if (
        not isinstance(data, list)
        or len(data) != 1
        or not isinstance(data[0], dict)
    ):
        raise PermissionError("Unexpected Docker inspection result")
    return data[0]


def trusted_gateway():
    """Validate the internal RAG network before WebUI exists."""
    if os.geteuid() != 0:
        raise PermissionError("Ollama bridge requires root")

    network = docker_json(["network", "inspect", NETWORK])
    labels = network.get("Labels") or {}
    if (
        network.get("Name") != NETWORK
        or network.get("Driver") != "bridge"
        or network.get("Internal") is not True
        or not isinstance(labels, dict)
        or labels.get("org.daia.component") != "rag"
        or labels.get("org.daia.managed") != "true"
    ):
        raise PermissionError("Untrusted RAG network")

    try:
        configs = network["IPAM"]["Config"]
        if not isinstance(configs, list) or len(configs) != 1:
            raise ValueError("Ambiguous bridge IPAM")
        subnet = ipaddress.ip_network(configs[0]["Subnet"], strict=True)
        gateway = ipaddress.ip_address(configs[0]["Gateway"])
    except (KeyError, TypeError, ValueError, AttributeError) as exc:
        raise PermissionError("Invalid private bridge IPAM") from exc

    if (
        subnet.version != 4
        or gateway.version != 4
        or gateway not in subnet
        or gateway in (subnet.network_address, subnet.broadcast_address)
    ):
        raise PermissionError("Invalid private bridge gateway")

    return network, subnet, gateway


def trusted_endpoint():
    network, subnet, gateway = trusted_gateway()

    webui = docker_json(["container", "inspect", WEBUI])
    cid = webui.get("Id")
    if not isinstance(cid, str) or not IDENTITY.fullmatch(cid):
        raise PermissionError("Invalid WebUI identity")

    networks = (webui.get("NetworkSettings") or {}).get("Networks") or {}
    if set(networks) != {NETWORK}:
        raise PermissionError("Unexpected WebUI network attachment")
    attachment = networks[NETWORK]
    if attachment.get("NetworkID") != network.get("Id"):
        raise PermissionError("RAG network identity mismatch")
    address = ipaddress.ip_address(attachment["IPAddress"])
    if address.version != 4 or address not in subnet or address == gateway:
        raise PermissionError("Invalid WebUI address")

    members = network.get("Containers") or {}
    if cid not in members:
        raise PermissionError("WebUI is not attached to RAG network")
    member = members[cid]
    if not isinstance(member, dict):
        raise PermissionError("Malformed WebUI network membership")
    try:
        member_address = ipaddress.ip_interface(member["IPv4Address"])
    except (KeyError, TypeError, ValueError) as exc:
        raise PermissionError("Invalid WebUI membership address") from exc
    if member_address.ip != address or member_address.network != subnet:
        raise PermissionError("WebUI membership address mismatch")

    if command([VERIFY, "--verify-identity", cid]) != cid + chr(10):
        raise PermissionError("WebUI registration mismatch")

    inventory = command(
        ["docker", "container", "ls", "-a", "--format", "{{.Names}}"]
    ).splitlines()
    if len(inventory) != len(set(inventory)):
        raise PermissionError("Ambiguous container inventory")

    tika_args = []
    if "daia-tika" in inventory:
        tika_id = command(
            [TIKA_VERIFY, "--resolve-registered"]
        ).strip()
        if not IDENTITY.fullmatch(tika_id):
            raise PermissionError("Invalid Tika registration")
        tika_args = ["--tika-id", tika_id]

    if command(
        [VERIFY, "--verify-running", cid, *tika_args], timeout=90
    ) != cid + chr(10):
        raise PermissionError("WebUI runtime verification failed")

    return str(gateway), str(address)


class Relay(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            # A peer must match the current Docker attachment before
            # it can reach the host's loopback-only Ollama listener.
            gateway, allowed_ip = trusted_endpoint()
            if self.client_address[0] != allowed_ip:
                return
            upstream = socket.create_connection(OLLAMA, timeout=10)
        except (OSError, ValueError, KeyError, TypeError,
                PermissionError, subprocess.SubprocessError):
            return
        sockets = (self.request, upstream)
        try:
            # The upstream connect timeout is intentionally short.
            # Established Ollama requests may legitimately remain idle
            # for more than 30 seconds while a model processes input.
            for sock in sockets:
                sock.settimeout(None)

            def forward(source, destination):
                try:
                    while True:
                        data = source.recv(65536)
                        if not data:
                            break
                        destination.sendall(data)
                except (OSError, TimeoutError):
                    pass
                finally:
                    try:
                        destination.shutdown(socket.SHUT_WR)
                    except OSError:
                        pass

            worker = threading.Thread(
                target=forward, args=(self.request, upstream),
                daemon=True,
            )
            worker.start()
            forward(upstream, self.request)
            worker.join(timeout=5)
        finally:
            upstream.close()


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    gateway, _ = trusted_endpoint()
    with Server((gateway, 11434), Relay) as server:
        server.serve_forever(poll_interval=0.5)


if __name__ == "__main__":
    if len(sys.argv) == 2 and sys.argv[1] == "--gateway":
        _, _, gateway = trusted_gateway()
        print(gateway)
    elif len(sys.argv) == 1:
        main()
    else:
        raise SystemExit("Usage: daia-ollama-bridge.py [--gateway]")
