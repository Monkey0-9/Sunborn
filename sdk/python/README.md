# Sunborn Python SDK (`sunborn`)

Official Python client library for the **Sunborn Enterprise AI Gateway & Autonomic Cache Platform**.

[![Python Version](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![Type Checked](https://img.shields.io/badge/mypy-checked-blue.svg)](http://mypy-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green.svg)](LICENSE)

---

## 1. Overview

The Sunborn Python SDK provides high-performance, strictly typed synchronous and asynchronous clients for interacting with the Sunborn Gateway. It enables transparent semantic caching, multi-tier execution reuse, conformal risk calibration, and tenant-scoped cryptographic attestation for LLM and agentic workflows. Compliance obligations remain deployment-specific; the SDK provides technical controls, not a compliance certification.

### Key Capabilities
- **Exact & Semantic AI Request Routing**: Execute prompts with automatic L1/L2 cache lookup and conformal confidence verification.
- **Hierarchical Execution Reuse**: Cache deterministic tool outputs and evaluate Merkle DAG execution plans.
- **Synchronous & Asynchronous Lifecycles**: First-class support for both standard and `asyncio` applications with automatic connection pooling and clean resource disposal.
- **Strict Exception Hierarchy**: Distinguish network failures, authentication errors, rate limiting, and cache faults programmatically.
- **PEP 561 Compliant**: Fully typed public interfaces with `py.typed` marker for static type checking in `mypy` and `pyright`.

---

## 2. Installation

The package is not published on PyPI yet. Install it from this repository:

```bash
# Editable install from the repository root
pip install -e sdk/python
```

Or build and install a wheel from this tree:

```bash
python -m pip install build
python -m build sdk/python
pip install sdk/python/dist/sunborn-1.0.0-py3-none-any.whl
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
        print(f"Cached: {response.cached}")
        print(f"Tokens Used: {response.usage.get('total_tokens', 'n/a')}")
        # FinOps savings are reported by the gateway, not inferred by the SDK:
        # savings = client.get_finops_report()
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

### Drop-in OpenAI & Anthropic Clients

Switch existing OpenAI and Anthropic workloads to Sunborn with zero code rewrites:

```python
# Drop-in OpenAI Client
from sunborn.openai import OpenAI

client = OpenAI()
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "What is semantic caching?"}],
)
print(response.choices[0].message.content)
print(f"Cached: {response.sunborn_cached}")

# Drop-in Anthropic Client
from sunborn.anthropic import Anthropic

ant_client = Anthropic()
msg = ant_client.messages.create(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    messages=[{"role": "user", "content": "Explain causal invalidation."}],
)
print(msg.content[0].text)
```

### Agent Frameworks: LangChain & LlamaIndex

```python
# LangChain Global Cache
from langchain.globals import set_llm_cache
from sunborn.integrations.langchain import SunbornLangChainCache

set_llm_cache(SunbornLangChainCache())

# LlamaIndex Cache
from sunborn.integrations.llamaindex import SunbornLlamaIndexCache

cache = SunbornLlamaIndexCache()
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
