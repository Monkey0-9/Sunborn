"""
Asynchronous Sunborn Client for asyncio workflows.
"""

import asyncio
from typing import Any

from .client import SunbornClient


class AsyncSunbornClient:
    """Asynchronous client for Sunborn Gateway supporting asyncio execution and async context managers."""

    def __init__(
        self,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "default-app",
        timeout: float = 30.0,
        max_retries: int = 2,
        backoff_factor: float = 0.5,
    ):
        self._sync_client = SunbornClient(
            base_url=base_url,
            api_key=api_key,
            tenant_id=tenant_id,
            application_id=application_id,
            timeout=timeout,
            max_retries=max_retries,
            backoff_factor=backoff_factor,
        )

    async def __aenter__(self) -> "AsyncSunbornClient":
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        pass

    @property
    def base_url(self) -> str:
        return self._sync_client.base_url

    @property
    def tenant_id(self) -> str:
        return self._sync_client.tenant_id

    @property
    def application_id(self) -> str:
        return self._sync_client.application_id

    @property
    def api_key(self) -> str:
        return self._sync_client.api_key

    async def login(
        self, client_id: str, client_secret: str, tenant_id: str | None = None
    ) -> dict:
        """Asynchronously authenticates with the Sunborn Gateway login endpoint."""
        return await asyncio.to_thread(
            self._sync_client.login, client_id, client_secret, tenant_id
        )

    async def chat_completion(self, *args, **kwargs) -> dict:
        """Asynchronously invokes the OpenAI-compatible /chat/completions endpoint."""
        return await asyncio.to_thread(
            self._sync_client.chat_completion, *args, **kwargs
        )

    async def create_message(self, *args, **kwargs) -> dict:
        """Asynchronously invokes the Anthropic-compatible /messages endpoint."""
        return await asyncio.to_thread(
            self._sync_client.create_message, *args, **kwargs
        )

    messages = create_message

    async def get_finops_report(self) -> dict:
        """Asynchronously retrieves the live FinOps cost and token savings report."""
        return await asyncio.to_thread(self._sync_client.get_finops_report)

    async def plan_execution_dag(self, plan_nodes: list[dict]) -> dict:
        """Asynchronously evaluates an execution DAG for tool memoization."""
        return await asyncio.to_thread(self._sync_client.plan_execution_dag, plan_nodes)

    async def record_tool_result(self, *args, **kwargs) -> dict:
        """Asynchronously records a tool execution result in the Sunborn cache."""
        return await asyncio.to_thread(
            self._sync_client.record_tool_result, *args, **kwargs
        )
