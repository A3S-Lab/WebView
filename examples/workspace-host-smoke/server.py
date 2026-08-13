#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import threading
from collections.abc import Callable
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from socket import socket
from socketserver import BaseServer
from urllib.parse import parse_qs, urlsplit


ALLOWED_STEPS = {
    "bridge-missing",
    "bridge-present",
    "calling-ready",
    "remote-cross-origin-navigation-blocked",
    "remote-loaded",
    "remote-ready",
    "remote-same-origin-navigation",
    "state-lost",
    "state-retained",
}

HandlerFactory = Callable[[socket, tuple[str, int], BaseServer], "SmokeHandler"]


class SmokeState:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._steps: list[str] = []
        self._forbidden_requests: list[str] = []

    def record_step(self, step: str) -> None:
        with self._lock:
            if step not in self._steps:
                self._steps.append(step)

    def record_forbidden_request(self, path: str) -> None:
        with self._lock:
            self._forbidden_requests.append(path)

    def snapshot(self) -> dict[str, object]:
        with self._lock:
            return {
                "steps": list(self._steps),
                "forbiddenRequests": list(self._forbidden_requests),
            }


class SmokeHandler(SimpleHTTPRequestHandler):
    state: SmokeState
    origin_name: str

    def do_GET(self) -> None:
        request = urlsplit(self.path)
        if request.path == "/health":
            self._json({"origin": self.origin_name, "status": "ready"})
            return
        if request.path == "/smoke-state":
            self._json(self.state.snapshot())
            return
        if request.path == "/smoke-log":
            step = parse_qs(request.query).get("step", [""])[0]
            if step not in ALLOWED_STEPS:
                self._json({"error": "unknown smoke step"}, HTTPStatus.BAD_REQUEST)
                return
            self.state.record_step(step)
            self._json({"recorded": step})
            return
        if request.path == "/forbidden.html":
            self.state.record_forbidden_request(self.path)
        super().do_GET()

    def log_message(self, format: str, *args: object) -> None:
        print(f"[{self.origin_name}] {format % args}", flush=True)

    def _json(
        self,
        value: object,
        status: HTTPStatus = HTTPStatus.OK,
    ) -> None:
        payload = json.dumps(value, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.end_headers()
        self.wfile.write(payload)


def handler_for(
    directory: Path,
    state: SmokeState,
    origin_name: str,
) -> HandlerFactory:
    class OriginHandler(SmokeHandler):
        def __init__(
            self,
            request: socket,
            client_address: tuple[str, int],
            server: BaseServer,
        ) -> None:
            super().__init__(
                request,
                client_address,
                server,
                directory=str(directory),
            )

    OriginHandler.state = state
    OriginHandler.origin_name = origin_name
    return OriginHandler


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Serve the two-origin A3S Workspace Host smoke fixture."
    )
    parser.add_argument("--shell-port", type=int, default=4318)
    parser.add_argument("--remote-port", type=int, default=4319)
    args = parser.parse_args()

    directory = Path(__file__).resolve().parent
    state = SmokeState()
    shell = ThreadingHTTPServer(
        ("127.0.0.1", args.shell_port),
        handler_for(directory, state, "shell"),
    )
    remote = ThreadingHTTPServer(
        ("127.0.0.1", args.remote_port),
        handler_for(directory, state, "remote"),
    )
    remote_thread = threading.Thread(target=remote.serve_forever, daemon=True)
    remote_thread.start()
    print(
        f"Workspace smoke origins: http://127.0.0.1:{args.shell_port} and "
        f"http://127.0.0.1:{args.remote_port}",
        flush=True,
    )
    try:
        shell.serve_forever()
    finally:
        shell.server_close()
        remote.shutdown()
        remote.server_close()
        remote_thread.join(timeout=2)


if __name__ == "__main__":
    main()
