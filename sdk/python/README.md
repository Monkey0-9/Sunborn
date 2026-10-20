# Sunborn Python SDK (`sunborn`)

Official Python client library for the **Sunborn Enterprise AI Gateway & Autonomic Cache Platform**.

[![Python Version](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![Type Checked](https://img.shields.io/badge/mypy-checked-blue.svg)](http://mypy-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green.svg)](LICENSE)

---

## 1. Overview

The Sunborn Python SDK provides high-performance, strictly typed synchronous and asynchronous clients for interacting with the Sunborn Gateway. It enables transparent semantic caching, multi-tier execution reuse, conformal risk calibration, and GDPR-compliant cryptographic tenant governance for LLM and agentic workflows.

### Key Capabilities
- **Exact & Semantic AI Request Routing**: Execute prompts with automatic L1/L2 cache lookup and conformal confidence verification.
- **Hierarchical Execution Reuse**: Cache deterministic tool outputs and evaluate Merkle DAG execution plans.
- **Synchronous & Asynchronous Lifecycles**: First-class support for both standard and `asyncio` applications with automatic connection pooling and clean resource disposal.
- **Strict Exception Hierarchy**: Distinguish network failures, authentication errors, rate limiting, and cache faults programmatically.
- **PEP 561 Compliant**: Fully typed public interfaces with `py.typed` marker for static type checking in `mypy` and `pyright`.

---

## 2. Installation

Install via `pip`:

```bash
pip install sunborn
```

Or install from the built wheel:

```bash
pip install dist/sunborn-0.1.0rc1-py3-none-any.whl
```

Requirements: Python 3.10 or higher.

---

## 3. Quick Start

### Synchronous Client

```python
from sunborn import Sunborn
from sunborn.exceptions import SunbornError, AuthenticationError

# Initialize client with connection pooling
with Sunborn(
    base_url="http://127.0.0.1:8080",
    api_key="your-tenant-api-key",
    tenant_id="tenant-enterprise-alpha",
    application_id="app-customer-support",
    timeout=10.0,
) as client:
    try:
        response = client.execute_request(
            prompt="What are the repayment options for commercial loan 402?",
            model="gpt-4o",
            temperature=0.0,
        )
        print(f"Content: {response.content}")
        print(f"Cache Tier: {response.tier}")
        print(f"Tokens Avoided: {response.tokens_avoided}")
        print(f"Cost Savings (USD): ${response.cost_savings_usd:.4f}")
    except AuthenticationError as e:
        print(f"Authentication failed: {e}")
    except SunbornError as e:
        print(f"Gateway error: {e}")
```

### Asynchronous Client

```python
import asyncio
from sunborn import AsyncSunborn

async def main():
    async with AsyncSunborn(
        base_url="http://127.0.0.1:8080",
        api_key="your-tenant-api-key",
        tenant_id="tenant-enterprise-alpha",
        application_id="app-customer-support",
    ) as client:
        response = await client.execute_request(
            prompt="Analyze the quarterly performance indicators.",
            model="gpt-4o",
        )
        print(f"Tier: {response.tier} | Cached: {response.cached}")

asyncio.run(main())
```

---

## 4. Exception Hierarchy

All exceptions inherit from `sunborn.exceptions.SunbornError`:

```
SunbornError
├── ConfigurationError
├── ConnectionError
│   └── TimeoutError
├── APIError
│   ├── BadRequestError
│   ├── AuthenticationError
│   ├── ForbiddenError
│   ├── NotFoundError
│   ├── RateLimitError
│   └── ServerError
└── InvalidationError
```

---

## 5. API Compatibility & Stability Policy

Sunborn adheres to **Semantic Versioning 2.0.0** (`MAJOR.MINOR.PATCH`):
- **PATCH** releases (`0.1.x`) contain backward-compatible bug fixes and performance improvements.
- **MINOR** releases (`0.x.0`) introduce new backward-compatible functionality and SDK capabilities.
- **MAJOR** releases (`x.0.0`) introduce breaking API changes.
- **Deprecation Policy**: Any public interface marked for deprecation will emit a `DeprecationWarning` for at least two minor releases before formal removal.
- **Type Safety**: All public symbols are typed and guaranteed stable across minor versions.
