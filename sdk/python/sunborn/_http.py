"""Pooled HTTP transports with bounded retries, connection pooling, and stdlib fallback."""

from __future__ import annotations

import asyncio
import time
import urllib.error
import urllib.request
from collections.abc import Mapping
from typing import Any

from ._json import dumps, loads
from .exceptions import SunbornConnectionError, raise_for_status

try:
    import httpx

    HAS_HTTPX = True
except ImportError:
    httpx = None  # type: ignore[assignment]
    HAS_HTTPX = False

RETRY_STATUS = frozenset({429, 500, 502, 503, 504})


def _retry_delay(attempt: int, backoff: float, retry_after: str | None = None) -> float:
    if retry_after:
        try:
            return max(0.0, float(retry_after))
        except ValueError:
            pass
    return backoff * (2**attempt)


class SyncTransport:
    """Synchronous transport (uses pooled ``httpx.Client`` if installed, falls back to stdlib ``urllib``)."""

    def __init__(
        self,
        base_url: str,
        headers: Mapping[str, str],
        *,
        timeout: float = 30.0,
        max_retries: int = 2,
        backoff_factor: float = 0.5,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._default_headers = dict(headers)
        self._timeout = timeout
        self._max_retries = max(0, max_retries)
        self._backoff_factor = max(0.0, backoff_factor)
        self._closed = False

        if HAS_HTTPX:
            self._client: Any = httpx.Client(timeout=timeout, headers=dict(headers))
        else:
            self._client = None

    @property
    def is_closed(self) -> bool:
        return self._closed

    def close(self) -> None:
        if not self._closed:
            if self._client is not None:
                self._client.close()
            self._closed = True

    def _ensure_open(self) -> None:
        if self._closed:
            raise RuntimeError("Cannot execute requests on a closed Sunborn client session.")

    def request(
        self,
        method: str,
        path: str,
        payload: Mapping[str, Any] | None = None,
        headers: Mapping[str, str] | None = None,
    ) -> Any:
        self._ensure_open()
        url = f"{self._base_url}/{path.lstrip('/')}"
        content = dumps(payload) if payload is not None else None

        if self._client is not None:
            return self._request_httpx(method, url, content, headers)
        return self._request_urllib(method, url, content, headers)

    def _request_httpx(
        self,
        method: str,
        url: str,
        content: bytes | None,
        headers: Mapping[str, str] | None,
    ) -> Any:
        last_error: Exception | None = None
        for attempt in range(self._max_retries + 1):
            try:
                response = self._client.request(
                    method, url, content=content, headers=dict(headers) if headers else None
                )
            except httpx.HTTPError as exc:
                last_error = exc
                if attempt < self._max_retries:
                    time.sleep(_retry_delay(attempt, self._backoff_factor))
                    continue
                raise SunbornConnectionError(
                    f"Failed to connect to Sunborn Gateway at {url}: {exc}"
                ) from exc

            if response.status_code in RETRY_STATUS and attempt < self._max_retries:
                time.sleep(_retry_delay(attempt, self._backoff_factor, response.headers.get("retry-after")))
                continue

            if response.status_code >= 400:
                text = response.text
                details = None
                if text:
                    try:
                        parsed = loads(text)
                        if isinstance(parsed, dict):
                            details = parsed
                    except Exception:
                        pass
                raise_for_status(response.status_code, text, details)

            if not response.content:
                return {}
            return loads(response.content)

        raise SunbornConnectionError(
            f"Failed to connect to Sunborn Gateway at {url} after {self._max_retries} retries: {last_error}"
        )

    def _request_urllib(
        self,
        method: str,
        url: str,
        content: bytes | None,
        headers: Mapping[str, str] | None,
    ) -> Any:
        merged_headers = dict(self._default_headers)
        if headers:
            merged_headers.update(headers)
        merged_headers.setdefault("Connection", "keep-alive")

        last_error: Exception | None = None
        for attempt in range(self._max_retries + 1):
            req = urllib.request.Request(
                url,
                data=content if method not in ("GET", "HEAD") else None,
                headers=merged_headers,
                method=method,
            )
            try:
                with urllib.request.urlopen(req, timeout=self._timeout) as resp:
                    body = resp.read()
                    return loads(body) if body else {}
            except urllib.error.HTTPError as exc:
                err_body = exc.read().decode("utf-8", errors="replace") if hasattr(exc, "read") else str(exc)
                details = None
                try:
                    parsed = loads(err_body)
                    if isinstance(parsed, dict):
                        details = parsed
                except Exception:
                    pass

                if exc.code in RETRY_STATUS and attempt < self._max_retries:
                    retry_after = exc.headers.get("retry-after") if hasattr(exc, "headers") else None
                    time.sleep(_retry_delay(attempt, self._backoff_factor, retry_after))
                    continue
                raise_for_status(exc.code, err_body, details)
            except urllib.error.URLError as exc:
                last_error = exc
                if attempt < self._max_retries:
                    time.sleep(_retry_delay(attempt, self._backoff_factor))
                    continue
                raise SunbornConnectionError(
                    f"Failed to connect to Sunborn Gateway at {url}: {exc}"
                ) from exc

        raise SunbornConnectionError(
            f"Failed to connect to Sunborn Gateway at {url} after {self._max_retries} retries: {last_error}"
        )


class AsyncTransport:
    """Asynchronous pooled transport (uses ``httpx.AsyncClient`` if installed, or runs sync transport in threadpool)."""

    def __init__(
        self,
        base_url: str,
        headers: Mapping[str, str],
        *,
        timeout: float = 30.0,
        max_retries: int = 2,
        backoff_factor: float = 0.5,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._max_retries = max(0, max_retries)
        self._backoff_factor = max(0.0, backoff_factor)
        self._closed = False

        if HAS_HTTPX:
            self._client: Any = httpx.AsyncClient(timeout=timeout, headers=dict(headers))
            self._sync_fallback = None
        else:
            self._client = None
            self._sync_fallback = SyncTransport(
                base_url,
                headers,
                timeout=timeout,
                max_retries=max_retries,
                backoff_factor=backoff_factor,
            )

    @property
    def is_closed(self) -> bool:
        return self._closed

    async def aclose(self) -> None:
        if not self._closed:
            if self._client is not None:
                await self._client.aclose()
            elif self._sync_fallback is not None:
                self._sync_fallback.close()
            self._closed = True

    def _ensure_open(self) -> None:
        if self._closed:
            raise RuntimeError("Cannot execute requests on a closed Sunborn client session.")

    async def request(
        self,
        method: str,
        path: str,
        payload: Mapping[str, Any] | None = None,
        headers: Mapping[str, str] | None = None,
    ) -> Any:
        self._ensure_open()
        if self._client is None and self._sync_fallback is not None:
            return await asyncio.to_thread(
                self._sync_fallback.request, method, path, payload, headers
            )

        url = f"{self._base_url}/{path.lstrip('/')}"
        content = dumps(payload) if payload is not None else None
        last_error: Exception | None = None

        for attempt in range(self._max_retries + 1):
            try:
                response = await self._client.request(
                    method, url, content=content, headers=dict(headers) if headers else None
                )
            except httpx.HTTPError as exc:
                last_error = exc
                if attempt < self._max_retries:
                    await asyncio.sleep(_retry_delay(attempt, self._backoff_factor))
                    continue
                raise SunbornConnectionError(
                    f"Failed to connect to Sunborn Gateway at {url}: {exc}"
                ) from exc

            if response.status_code in RETRY_STATUS and attempt < self._max_retries:
                retry_after = response.headers.get("retry-after")
                await asyncio.sleep(_retry_delay(attempt, self._backoff_factor, retry_after))
                continue

            if response.status_code >= 400:
                text = response.text
                details = None
                if text:
                    try:
                        parsed = loads(text)
                        if isinstance(parsed, dict):
                            details = parsed
                    except Exception:
                        pass
                raise_for_status(response.status_code, text, details)

            if not response.content:
                return {}
            return loads(response.content)

        raise SunbornConnectionError(
            f"Failed to connect to Sunborn Gateway at {url} after {self._max_retries} retries: {last_error}"
        )
