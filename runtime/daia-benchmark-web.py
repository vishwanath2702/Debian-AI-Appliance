#!/usr/bin/env python3
"""Loopback-only DAIA benchmark web interface foundation."""

import argparse
import importlib.util
import json
import html
import secrets
import socket
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>DAIA Model Benchmark</title>
<meta name="daia-csrf-token" content="__DAIA_CSRF_TOKEN__">
<style>
body { font-family: system-ui, sans-serif; max-width: 960px;
       margin: 3rem auto; padding: 0 1rem; line-height: 1.5; }
h1 { margin-bottom: .25rem; }
small { color: #666; }
section { border: 1px solid #aaa; border-radius: 8px;
          padding: 1.25rem; margin-top: 1.5rem; }
</style>
</head>
<body>
<h1>DAIA Model Benchmark</h1>
<small>Local appliance interface</small>
<section>
<h2>Benchmark runner</h2>
<label for="model-name">Installed model name</label>
<input id="model-name" type="text" maxlength="512"
       autocomplete="off" spellcheck="false">
<p>
<button id="start-button" type="button" disabled>Start benchmark</button>
<button id="cancel-button" type="button" disabled>Cancel benchmark</button>
</p>
<p id="benchmark-status" role="status">Interface initializing</p>
</section>
<section>
<h2>Hardware telemetry</h2>
<p id="cpu-usage">CPU: unavailable</p>
<p id="ram-usage">RAM: unavailable</p>
<div id="gpu-telemetry">GPU telemetry: unavailable</div>
</section>
<section>
<h2>Benchmark results</h2>
<div id="benchmark-results">No benchmark results yet.</div>
</section>
<script src="/benchmark.js" defer></script>
</body>
</html>
""".encode("utf-8")


def load_controller():
    source = Path(__file__).resolve().with_name(
        "daia-benchmark-controller.py"
    )
    spec = importlib.util.spec_from_file_location(
        "daia_benchmark_controller", source
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.BenchmarkController()


CONTROLLER = load_controller()


class BenchmarkHandler(BaseHTTPRequestHandler):
    def _valid_csrf_token(self):
        tokens = self.headers.get_all("X-DAIA-CSRF-Token", [])
        return (
            len(tokens) == 1
            and secrets.compare_digest(
                tokens[0], self.server.csrf_token
            )
        )

    def _valid_host(self):
        expected = "127.0.0.1:%d" % self.server.server_address[1]
        return self.headers.get_all("Host", []) == [expected]

    def _reject_invalid_host(self):
        if self._valid_host():
            return False
        self._respond(
            403, "text/plain; charset=utf-8", b"Forbidden"
        )
        return True

    def _valid_origin(self):
        origins = self.headers.get_all("Origin", [])
        if not origins:
            return True
        expected = "http://127.0.0.1:%d" % self.server.server_address[1]
        return origins == [expected]

    def _reject_invalid_origin(self):
        if self._valid_origin():
            return False
        self._respond(
            403, "text/plain; charset=utf-8", b"Forbidden"
        )
        return True

    def _respond(self, status, content_type, body):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Content-Security-Policy",
                         "default-src 'none'; script-src 'self'; "
                         "connect-src 'self'; style-src 'unsafe-inline'")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self._reject_invalid_host():
            return
        if self._reject_invalid_origin():
            return
        if self.path == "/":
            token = html.escape(
                self.server.csrf_token, quote=True
            ).encode("utf-8")
            page = PAGE.replace(b"__DAIA_CSRF_TOKEN__", token)
            self._respond(200, "text/html; charset=utf-8", page)
        elif self.path == "/benchmark.js":
            source = Path(__file__).resolve().with_name("benchmark.js")
            try:
                if source.is_symlink() or not source.is_file():
                    raise OSError("Benchmark JavaScript unavailable")
                content = source.read_bytes()
            except OSError:
                self._respond(
                    503, "text/plain; charset=utf-8",
                    b"Benchmark JavaScript unavailable"
                )
                return
            self._respond(
                200, "text/javascript; charset=utf-8", content
            )
        elif self.path == "/health":
            self._respond(200, "application/json; charset=utf-8",
                          b'{"status":"ok"}')
        elif self.path == "/api/benchmark/status":
            body = json.dumps(CONTROLLER.snapshot()).encode("utf-8")
            self._respond(200, "application/json; charset=utf-8", body)
        else:
            self._respond(404, "text/plain; charset=utf-8", b"Not found")

    def do_POST(self):
        if self._reject_invalid_host():
            return
        if self._reject_invalid_origin():
            return

        if self.path not in (
            "/api/benchmark/start",
            "/api/benchmark/cancel",
        ):
            self._respond(
                405, "text/plain; charset=utf-8",
                b"Method not allowed"
            )
            return

        if not self._valid_csrf_token():
            self._respond(
                403, "text/plain; charset=utf-8", b"Forbidden"
            )
            return

        if self.path == "/api/benchmark/cancel":
            if self.headers.get_all("Transfer-Encoding", []):
                self.close_connection = True
                self._respond(
                    400, "text/plain; charset=utf-8",
                    b"Transfer encoding not supported"
                )
                return

            lengths = self.headers.get_all("Content-Length", [])
            if lengths not in ([], ["0"]):
                self.close_connection = True
                self._respond(
                    400, "text/plain; charset=utf-8",
                    b"Cancel request must have no body"
                )
                return

            if not CONTROLLER.cancel():
                self._respond(
                    409, "text/plain; charset=utf-8",
                    b"No benchmark running"
                )
                return

            self._respond(
                200, "application/json; charset=utf-8",
                b'{"status":"cancelled"}'
            )
            return

        if self.headers.get_all("Content-Type", []) != [
            "application/json"
        ]:
            self._respond(
                415, "text/plain; charset=utf-8",
                b"Unsupported media type"
            )
            return

        if self.headers.get_all("Transfer-Encoding", []):
            self._respond(
                400, "text/plain; charset=utf-8",
                b"Transfer encoding not supported"
            )
            self.close_connection = True
            return

        lengths = self.headers.get_all("Content-Length", [])
        if len(lengths) != 1 or not lengths[0].isdigit():
            self._respond(
                400, "text/plain; charset=utf-8",
                b"Invalid content length"
            )
            return

        length = int(lengths[0])
        if not 1 <= length <= 2048:
            self._respond(
                413, "text/plain; charset=utf-8",
                b"Invalid request size"
            )
            return

        try:
            self.connection.settimeout(3.0)
            body = self.rfile.read(length)
            if len(body) != length:
                self._respond(
                    400, "text/plain; charset=utf-8",
                    b"Incomplete request body"
                )
                self.close_connection = True
                return
            payload = json.loads(body)
        except (socket.timeout, TimeoutError, OSError):
            self.close_connection = True
            return
        except (ValueError, UnicodeDecodeError):
            self._respond(
                400, "text/plain; charset=utf-8",
                b"Invalid JSON"
            )
            return

        if not isinstance(payload, dict) or set(payload) != {
            "model_name"
        }:
            self._respond(
                400, "text/plain; charset=utf-8",
                b"Invalid request fields"
            )
            return

        try:
            CONTROLLER.start(payload["model_name"])
        except ValueError:
            self._respond(
                400, "text/plain; charset=utf-8",
                b"Invalid model name"
            )
            return
        except RuntimeError:
            self._respond(
                409, "text/plain; charset=utf-8",
                b"Benchmark already running"
            )
            return
        except OSError:
            self._respond(
                503, "text/plain; charset=utf-8",
                b"Benchmark unavailable"
            )
            return

        self._respond(
            202, "application/json; charset=utf-8",
            b'{"status":"started"}'
        )

    def log_message(self, format, *args):
        pass


def create_server(port=0):
    if not isinstance(port, int) or not 0 <= port <= 65535:
        raise ValueError("Invalid port")
    server = ThreadingHTTPServer(
        ("127.0.0.1", port), BenchmarkHandler
    )
    server.csrf_token = secrets.token_urlsafe(32)
    return server


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=0)
    args = parser.parse_args()
    server = create_server(args.port)
    print("DAIA benchmark server: http://127.0.0.1:%d/" %
          server.server_address[1], flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
