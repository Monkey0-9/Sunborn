"""Threaded HTTP stub for SDK tests.

Using a real loopback server means the tests exercise the same transport code
as production (headers, retries, status mapping) instead of patching a
library-level function.
"""

from __future__ import annotations

import json
import threading
from collections.abc import Callable
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


@dataclass
class RecordedRequest:
    method: str
    path: str
    headers: dict[str, str]
    body: bytes

    def json(self) -> Any:
        return json.loads(self.body) if self.body else None


Response = tuple[int, Any] | tuple[int, Any, dict[str, str]]
Responder = Callable[[RecordedRequest], Response]


def json_response(payload: Any, status: int = 200) -> Response:
    return status, payload


class _Handler(BaseHTTPRequestHandler):
    server_version = "SunbornTestStub/1.0"

    def do_GET(self) -> None:  # noqa: N802 - http.server API
        self._handle("GET")

    def do_POST(self) -> None:  # noqa: N802 - http.server API
        self._handle("POST")

    def _handle(self, method: str) -> None:
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        request = RecordedRequest(method, self.path, dict(self.headers), body)

        server: Any = self.server
        server.requests.append(request)
        try:
            result = server.responder(request)
        except Exception as exc:  # pragma: no cover - test wiring failure
            result = (500, {"detail": f"stub responder failed: {exc}"})

        status, payload = result[0], result[1]
        extra_headers = result[2] if len(result) > 2 else {}
        body_bytes = b"" if payload is None else json.dumps(payload).encode("utf-8")

        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body_bytes)))
        for key, value in extra_headers.items():
            self.send_header(key, value)
        self.end_headers()
        if body_bytes:
            self.wfile.write(body_bytes)

    def log_message(self, *args: Any) -> None:  # noqa: D401 - silence test output
        return


class StubServer:
    """A loopback HTTP server whose responses are supplied by a callback."""

    def __init__(self, responder: Responder | None = None) -> None:
        self.requests: list[RecordedRequest] = []
        self.responder: Responder = responder or (lambda _req: (200, {}))
        self._server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
        self._server.daemon_threads = True
        self._server.requests = self.requests  # type: ignore[attr-defined]
        self._server.responder = self.responder  # type: ignore[attr-defined]
        self._thread = threading.Thread(
            target=self._server.serve_forever, name="sunborn-test-stub", daemon=True
        )

    @property
    def base_url(self) -> str:
        host, port = self._server.server_address[:2]
        return f"http://{host}:{port}"

    @property
    def last_request(self) -> RecordedRequest:
        if not self.requests:
            raise AssertionError("no request was recorded")
        return self.requests[-1]

    def __enter__(self) -> StubServer:
        self._thread.start()
        return self

    def __exit__(self, exc_type: object, exc_val: object, exc_tb: object) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=5)


def sequence(*responses: Response) -> Responder:
    """Return a responder that yields each response in order, then repeats the last."""
    remaining = list(responses)

    def responder(_request: RecordedRequest) -> Response:
        if len(remaining) > 1:
            return remaining.pop(0)
        return remaining[0]

    return responder
