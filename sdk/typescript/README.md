# Sunborn TypeScript SDK (`@sunborn/sdk`)

Enterprise-grade TypeScript and JavaScript SDK for the Sunborn Deterministic AI Inference & Invalidation Platform.

---

## Installation

```bash
npm install @sunborn/sdk
```

---

## Quickstart

### Chat Completion

```typescript
import { SunbornClient } from "@sunborn/sdk";

const client = new SunbornClient({
  baseUrl: process.env.SUNBORN_BASE_URL || "http://localhost:8080",
  apiKey: process.env.SUNBORN_API_KEY || "sb_live_your_api_key_here",
  timeoutMs: 30000,
  maxRetries: 3,
});

async function main() {
  const response = await client.chatCompletion({
    model: "gpt-4o",
    messages: [
      { role: "system", content: "You are a concise financial analyst." },
      { role: "user", content: "Summarize Q3 operating margins." },
    ],
    temperature: 0.0,
    max_tokens: 1000,
  });

  console.log(`Content: ${response.choices[0].message.content}`);
  console.log(`Cached: ${response.cached} (Tier: ${response.tier})`);
  console.log(`Decision: ${response.decision.action} [${response.decision.reason_code}]`);
  console.log(`Cost Avoided: $${response.usage.estimated_cost_avoided_usd?.toFixed(5) || "0.00"}`);
}

main().catch(console.error);
```

### Streaming SSE Responses

```typescript
async function streamExample() {
  for await (const chunk of client.chatCompletionStream({
    model: "gpt-4o",
    messages: [{ role: "user", content: "Explain Byzantine fault tolerance." }],
  })) {
    const text = chunk.choices[0]?.delta?.content || "";
    process.stdout.write(text);
  }
}
```

---

## Features

- **Multi-Tier Cache Transparency:** Full inspection of exact hash hits (`c0_exact`, `c1_exact`), conformal semantic hits (`c2_semantic`), and fail-safe abstentions.
- **Async Streaming Iterators:** Native `AsyncIterable` streaming support (`chatCompletionStream`) consuming Server-Sent Events (SSE) chunks without latency overhead.
- **Fail-Closed Reliability:** Native translation of RFC 7807 problem details into typed `SunbornError` instances.
- **Automatic Jittered Retries:** Built-in exponential backoff for rate limits (`429`) and transient errors with `Retry-After` header respect.
- **Zero-Dependency Core:** Lightweight, standard ESM using native `fetch` and `AbortController`.
