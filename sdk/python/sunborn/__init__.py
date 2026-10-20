"""
Sunborn Enterprise AI Gateway & Execution Reuse Python SDK.
"""

from .async_client import AsyncSunbornClient
from .client import SunbornAgentClient, SunbornClient, get_default_client, sunborn_tool
from .exceptions import (
    SunbornAuthError,
    SunbornConnectionError,
    SunbornError,
    SunbornForbiddenError,
    SunbornNotFoundError,
    SunbornRateLimitError,
    SunbornServerError,
    SunbornValidationError,
)
from .models import (
    CachePolicy,
    ExecutionTier,
    FallbackPolicy,
    ToolResultRecord,
    compute_context_digest,
)

__version__ = "0.1.0rc1"

# Canonical and ergonomic aliases
Sunborn = SunbornClient
AsyncSunborn = AsyncSunbornClient
AuthenticationError = SunbornAuthError
ForbiddenError = SunbornForbiddenError
NotFoundError = SunbornNotFoundError
RateLimitError = SunbornRateLimitError
BadRequestError = SunbornValidationError
ServerError = SunbornServerError

__all__ = [
    "Sunborn",
    "SunbornClient",
    "AsyncSunborn",
    "AsyncSunbornClient",
    "SunbornAgentClient",
    "get_default_client",
    "sunborn_tool",
    "CachePolicy",
    "FallbackPolicy",
    "ExecutionTier",
    "ToolResultRecord",
    "compute_context_digest",
    "SunbornError",
    "SunbornConnectionError",
    "SunbornAuthError",
    "AuthenticationError",
    "SunbornForbiddenError",
    "ForbiddenError",
    "SunbornNotFoundError",
    "NotFoundError",
    "SunbornRateLimitError",
    "RateLimitError",
    "SunbornValidationError",
    "BadRequestError",
    "SunbornServerError",
    "ServerError",
]
