"""
Sunborn Enterprise AI Gateway & Execution Reuse Python SDK.
"""

from .accel import (
    fast_cosine_similarity,
    fast_dot_product,
    fast_euclidean_distance,
    fast_l2_norm,
    is_c_accelerator_available,
)
from .anthropic import (
    Anthropic,
    AsyncAnthropic,
    AsyncSunbornAnthropic,
    MessageResponse,
    MessageStreamEvent,
    SunbornAnthropic,
)
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
from .openai import (
    AsyncOpenAI,
    AsyncStream,
    AsyncSunbornOpenAI,
    ChatCompletion,
    ChatCompletionChunk,
    OpenAI,
    Stream,
    SunbornOpenAI,
)

__version__ = "1.0.0"

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
    "OpenAI",
    "SunbornOpenAI",
    "AsyncOpenAI",
    "AsyncSunbornOpenAI",
    "Stream",
    "AsyncStream",
    "ChatCompletion",
    "ChatCompletionChunk",
    "Anthropic",
    "SunbornAnthropic",
    "AsyncAnthropic",
    "AsyncSunbornAnthropic",
    "MessageResponse",
    "MessageStreamEvent",
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
    "fast_cosine_similarity",
    "fast_dot_product",
    "fast_l2_norm",
    "fast_euclidean_distance",
    "is_c_accelerator_available",
]
