#!/usr/bin/env python3
"""Minimal loopback Git Smart HTTP server for integration tests."""

from __future__ import annotations

import argparse
import subprocess
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse


def packet_line(payload: bytes) -> bytes:
    return f"{len(payload) + 4:04x}".encode("ascii") + payload


class Handler(BaseHTTPRequestHandler):
    repository: Path

    def do_GET(self) -> None:
        request = urlparse(self.path)
        service = parse_qs(request.query).get("service", [None])[0]
        if request.path != "/repo.git/info/refs" or service not in {
            "git-upload-pack",
            "git-receive-pack",
        }:
            self.send_error(404)
            return

        advertised = subprocess.run(
            [
                "git",
                service.removeprefix("git-"),
                "--stateless-rpc",
                "--advertise-refs",
                self.repository,
            ],
            check=True,
            capture_output=True,
        ).stdout
        body = packet_line(f"# service={service}\n".encode()) + b"0000" + advertised
        self.send_response(200)
        self.send_header("Content-Type", f"application/x-{service}-advertisement")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:
        service = urlparse(self.path).path.removeprefix("/repo.git/")
        if service not in {"git-upload-pack", "git-receive-pack"}:
            self.send_error(404)
            return

        length = int(self.headers.get("Content-Length", "0"))
        result = subprocess.run(
            ["git", service.removeprefix("git-"), "--stateless-rpc", self.repository],
            input=self.rfile.read(length),
            capture_output=True,
        )
        if result.returncode != 0:
            self.send_error(500, result.stderr.decode("utf-8", errors="replace"))
            return
        self.send_response(200)
        self.send_header("Content-Type", f"application/x-{service}-result")
        self.send_header("Content-Length", str(len(result.stdout)))
        self.end_headers()
        self.wfile.write(result.stdout)

    def log_message(self, message: str, *args: object) -> None:
        print(f"smart-http: {message % args}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("repository", type=Path)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", default=0, type=int)
    parser.add_argument("--ready-file", required=True, type=Path)
    args = parser.parse_args()

    Handler.repository = args.repository.resolve()
    server = ThreadingHTTPServer((args.host, args.port), Handler)
    args.ready_file.write_text(str(server.server_address[1]), encoding="utf-8")
    print(
        f"serving {Handler.repository} at "
        f"http://{args.host}:{server.server_address[1]}/repo.git",
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
