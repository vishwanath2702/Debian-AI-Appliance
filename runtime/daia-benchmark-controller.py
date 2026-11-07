#!/usr/bin/env python3
"""Single-active-process controller for DAIA benchmark JSONL events."""

import json
import subprocess
import threading
from collections import deque


EVENT_TYPES = frozenset(
    ("started", "hardware_sample", "completed", "failed")
)


def valid_event(event):
    return (
        isinstance(event, dict)
        and type(event.get("schema_version")) is int
        and event["schema_version"] == 1
        and event.get("event") in EVENT_TYPES
    )


class BenchmarkController:
    def __init__(self, command="/usr/bin/daia", max_events=256):
        if type(max_events) is not int or max_events < 1:
            raise ValueError("max_events must be positive")
        self.command = command
        self.events = deque(maxlen=max_events)
        self.lock = threading.Lock()
        self.process = None
        self.reader = None
        self.running = False
        self.state = "idle"
        self.exit_code = None
        self.generation = 0
        self.cancel_requested = False

    def start(self, model_name):
        if not isinstance(model_name, str) or not model_name.strip():
            raise ValueError("Model name is required")
        if len(model_name) > 512 or any(
            ord(char) < 32 or ord(char) == 127 for char in model_name
        ):
            raise ValueError("Invalid model name")

        with self.lock:
            if self.running or (
                self.reader is not None and self.reader.is_alive()
            ):
                raise RuntimeError("A benchmark is already running")

            process = subprocess.Popen(
                [self.command, "model", "benchmark",
                 model_name, "--events-jsonl"],
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                bufsize=1,
            )
            self.events.clear()
            self.generation += 1
            generation = self.generation
            self.process = process
            self.running = True
            self.state = "running"
            self.exit_code = None
            self.cancel_requested = False
            self.reader = threading.Thread(
                target=self._read_events,
                args=(process, generation),
                daemon=True,
            )
            self.reader.start()

    def _read_events(self, process, generation):
        terminal = None
        try:
            for line in process.stdout:
                try:
                    event = json.loads(line)
                except (ValueError, TypeError):
                    continue
                if not valid_event(event):
                    continue
                with self.lock:
                    if generation != self.generation:
                        continue
                    self.events.append(event)
                    if event["event"] in ("completed", "failed"):
                        terminal = event["event"]
        finally:
            exit_code = process.wait()
            with self.lock:
                if generation == self.generation:
                    self.exit_code = exit_code
                    if self.cancel_requested:
                        self.state = "cancelled"
                    elif exit_code == 0 and terminal == "completed":
                        self.state = "completed"
                    else:
                        self.state = "failed"
                    self.running = False

    def snapshot(self):
        with self.lock:
            return {
                "running": self.running,
                "state": self.state,
                "exit_code": self.exit_code,
                "events": list(self.events),
            }

    def cancel(self):
        with self.lock:
            process = self.process
            running = self.running
            if running:
                self.cancel_requested = True
        if not running or process is None:
            return False

        if process.poll() is None:
            try:
                process.terminate()
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=3)

        reader = self.reader
        if reader is not None:
            reader.join(timeout=3)
        return True

    def close(self):
        self.cancel()
