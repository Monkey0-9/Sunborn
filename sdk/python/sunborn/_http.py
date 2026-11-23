"""Pooled HTTP transports with bounded retries and typed error mapping."""

from __future__ import annotations

import asyncio
import time
from collections.abc import Mapping
from typing import Any

import httpx

from ._json import dumps, loads
from .exceptions import SunbornConnectionError, raise_for_status

RETRY_STATUS = frozenset({429, 500, 502, 503, 504})


def _retry_delay(attempt: int, backoff: float, response: httpx.Response | None = None) -> float:
    if response is not None:
        header = response.headers.get("retry-after")
        if header:
            try:
                return max(0.0, float(header))
            except ValueError:
                pass
    return backoff * (2**attempt)


def _decode_body(response: httpx.Response) -> Any:
    if not response.content:
        return {}
    return loads(response.content)


def _raise_response(response: httpx.Response) -> None:
    text = response.text
    details: dict[str, Any] | None = None
    if text:
        try:
            parsed = loads(text)
        except ValueError:
            parsed = None
        if isinstance(parsed, dict):
            details = parsed
    raise_for_status(response.status_code, text, details)


class SyncTransport:
    """Synchronous pooled transport (one keep-alive connection pool per client)."""

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
        self._client = httpx.Client(timeout=timeout, headers=dict(headers))
        self._max_retries = max(0, max_retries)
        self._backoff_factor = max(0.0, backoff_factor)
        self._closed = False

    @property
    def is_closed(self) -> bool:
        return self._closed

    def close(self) -> None:
        if not self._closed:
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
                raise SunbornConnectionError(f"Failed to connect to Sunborn Gateway at {url}: {exc}") from exc

            if response.status_code in RETRY_STATUS and attempt < self._max_retries:
                time.sleep(_retry_delay(attempt, self._backoff_factor, response))
                continue
            if response.status_code >= 400:
                _raise_response(response)
            return _decode_body(response)

        raise SunbornConnectionError(
            f"Failed to connect to Sunborn Gateway at {url} after {self._max_retries} retries: {last_error}"
        )


class AsyncTransport:
    """Asynchronous pooled transport backed by ``httpx.AsyncClient``."""

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
        self._client = httpx.AsyncClient(timeout=timeout, headers=dict(headers))
        self._max_retries = max(0, max_retries)
        self._backoff_factor = max(0.0, backoff_factor)
        self._closed = False

    @property
    def is_closed(self) -> bool:
        return self._closed

    async def aclose(self) -> None:
        if not self._closed:
            await self._client.aclose()
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
                raise SunbornConnectionError(f"Failed to connect to Sunborn Gateway at {url}: {exc}") from exc

            if response.status_code in RETRY_STATUS and attempt < self._max_retries:
                await asyncio.sleep(_retry_delay(attempt, self._backoff_factor, response))
                continue
            if response.status_code >= 400:
                _raise_response(response)
            return _decode_body(response)

        raise SunbornConnectionError(
            f"Failed to connect to Sunborn Gateway at {url} after {self._max_retries} retries: {last_error}"
        )
