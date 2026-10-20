"""
Sunborn SDK Exception Hierarchy mapping RFC 7807 Problem Details.
"""

from typing import Any


class SunbornError(RuntimeError):
    """Base exception for all Sunborn SDK operations (inherits from RuntimeError for standard error handling)."""

    def __init__(
        self,
        message: str,
        status_code: int | None = None,
        problem_details: dict[str, Any] | None = None,
    ):
        super().__init__(message)
        self.status_code = status_code
        self.problem_details = problem_details or {}


class SunbornConnectionError(ConnectionError, SunbornError):
    """Raised when the client cannot establish a network connection to Sunborn Gateway (inherits from ConnectionError)."""
    pass


class SunbornAuthError(SunbornError):
    """Raised when credentials are missing, invalid, or expired (HTTP 401)."""
    pass


class SunbornForbiddenError(SunbornError):
    """Raised when an operation lacks administrative or tenant privileges (HTTP 403)."""
    pass


class SunbornNotFoundError(SunbornError):
    """Raised when a requested resource or cache entry does not exist (HTTP 404)."""
    pass


class SunbornRateLimitError(SunbornError):
    """Raised when client or tenant request rate limit is exceeded (HTTP 429)."""

    def __init__(
        self,
        message: str,
        retry_after: float | None = None,
        problem_details: dict[str, Any] | None = None,
    ):
        super().__init__(message, status_code=429, problem_details=problem_details)
        self.retry_after = retry_after


class SunbornValidationError(SunbornError):
    """Raised when request payload fails schema validation or boundary constraints (HTTP 400)."""
    pass


class SunbornServerError(SunbornError):
    """Raised when Sunborn Gateway encounters an internal error (HTTP 5xx)."""
    pass


# Ergonomic and standard exception aliases
AuthenticationError = SunbornAuthError
ForbiddenError = SunbornForbiddenError
NotFoundError = SunbornNotFoundError
RateLimitError = SunbornRateLimitError
BadRequestError = SunbornValidationError
ServerError = SunbornServerError
ConfigurationError = SunbornValidationError


def raise_for_status(status_code: int, response_text: str, problem_details: dict[str, Any] | None = None):
    """Maps HTTP status codes to typed Sunborn SDK exceptions."""
    details = problem_details or {}
    detail_msg = details.get("detail", response_text)
    msg = f"Sunborn Gateway error [{status_code}]: {detail_msg}"

    if status_code == 401:
        raise SunbornAuthError(msg, status_code=401, problem_details=details)
    elif status_code == 403:
        raise SunbornForbiddenError(msg, status_code=403, problem_details=details)
    elif status_code == 404:
        raise SunbornNotFoundError(msg, status_code=404, problem_details=details)
    elif status_code == 429:
        retry_after = float(details.get("retry_after", 1.0))
        raise SunbornRateLimitError(msg, retry_after=retry_after, problem_details=details)
    elif status_code == 400:
        raise SunbornValidationError(msg, status_code=400, problem_details=details)
    elif status_code >= 500:
        raise SunbornServerError(msg, status_code=status_code, problem_details=details)
    elif status_code >= 400:
        raise SunbornError(msg, status_code=status_code, problem_details=details)
