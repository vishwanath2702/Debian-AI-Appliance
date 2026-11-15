#!/usr/bin/env python3
"""Loopback-only proxy to the independently verified Open WebUI."""
import importlib.util
import ipaddress
import socket
import socketserver
import threading
from pathlib import Path

BRIDGE = Path("/usr/libexec/daia-ollama-bridge.py")
LISTEN = ("127.0.0.1", 3000)
WEBUI_PORT = 3000


def trusted_webui():
    if not BRIDGE.is_file() or BRIDGE.is_symlink():
        raise PermissionError("Trusted bridge helper is unavailable")

    spec = importlib.util.spec_from_file_location(
        "daia_trusted_ollama_bridge", BRIDGE
    )
    if spec is None or spec.loader is None:
        raise PermissionError("Cannot load trusted bridge verifier")

    bridge = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bridge)

    _, address = bridge.trusted_endpoint()
    ip = ipaddress.ip_address(address)
    if ip.version != 4 or ip.is_loopback or ip.is_unspecified:
        raise PermissionError("Invalid Open WebUI destination")

    webui = bridge.docker_json(
        ["container", "inspect", bridge.WEBUI]
    )
    state = webui.get("State") or {}
    if (
        state.get("Running") is not True
        or state.get("Paused") is not False
        or state.get("Restarting") is not False
        or (state.get("Health") or {}).get("Status") != "healthy"
    ):
        raise PermissionError("Open WebUI is not healthy")

    return str(ip), WEBUI_PORT


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


class Relay(socketserver.BaseRequestHandler):
    def handle(self):
        upstream = None
        try:
            if self.client_address[0] != LISTEN[0]:
                return
            destination = trusted_webui()
            upstream = socket.create_connection(destination, timeout=10)
            self.request.settimeout(60)
            upstream.settimeout(60)
            worker = threading.Thread(
                target=forward,
                args=(self.request, upstream),
                daemon=True,
            )
            worker.start()
            forward(upstream, self.request)
            worker.join(timeout=5)
        except (OSError, ValueError, KeyError, TypeError,
                PermissionError):
            return
        finally:
            if upstream is not None:
                upstream.close()


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    # Bind only the host loopback interface.
    with Server(LISTEN, Relay) as server:
        server.serve_forever(poll_interval=0.5)


if __name__ == "__main__":
    main()
