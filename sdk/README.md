# Sunborn Software Development Kits (`sdk/`)

Official multi-language client SDKs for integrating Sunborn Trustworthy Execution-Reuse Infrastructure into AI applications and agent frameworks.

## Available SDKs

- **[`sdk/python`](./python/)**: Python 3.10+ client with async/sync support, LangChain/LlamaIndex middleware, and OpenAI/Anthropic drop-in compatibility.
- **[`sdk/typescript`](./typescript/)**: Node.js & browser TypeScript SDK with zero-dependency HTTP client, automatic retries, and streaming support.

## Installation & Tests

```bash
# Python SDK
pip install -e sdk/python
pytest sdk/python

# TypeScript SDK
npm test --prefix sdk/typescript
npm run build --prefix sdk/typescript
```
