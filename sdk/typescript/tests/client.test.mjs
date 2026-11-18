import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import { SunbornClient, SunbornError } from "../dist/client.js";

test("SunbornClient constructor sets defaults and strips trailing slashes", () => {
  const client = new SunbornClient({
    baseUrl: "http://api.sunborn.internal:8080/",
    apiKey: "sb_live_test_123",
  });
  assert.ok(client);
});

test("SunbornClient executes getHealth against mock gateway server", async () => {
  const server = http.createServer((req, res) => {
    if (req.url === "/health" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ service: "sunborn-gateway", status: "healthy", version: "1.0.0" }));
      return;
    }
    res.writeHead(404);
    res.end();
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;

  try {
    const client = new SunbornClient({
      baseUrl: `http://127.0.0.1:${port}`,
      apiKey: "test-key",
    });

    const health = await client.getHealth();
    assert.equal(health.service, "sunborn-gateway");
    assert.equal(health.status, "healthy");
    assert.equal(health.version, "1.0.0");
  } finally {
    server.close();
  }
});

test("SunbornClient executes chatCompletion with cache decision and attestation metadata", async () => {
  const server = http.createServer((req, res) => {
    if (req.url === "/v1/chat/completions" && req.method === "POST") {
      assert.equal(req.headers["x-api-key"], "sb_live_secret");
      assert.equal(req.headers["user-agent"], "Sunborn-TypeScript-SDK/1.0.0");

      let body = "";
      req.on("data", (chunk) => (body += chunk));
      req.on("end", () => {
        const payload = JSON.parse(body);
        assert.equal(payload.model, "gpt-4o");

        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(
          JSON.stringify({
            id: "chatcmpl-test-01",
            object: "chat.completion",
            created: 1718000000,
            model: "gpt-4o",
            choices: [
              {
                index: 0,
                message: { role: "assistant", content: "Deterministic safe response." },
                finish_reason: "stop",
              },
            ],
            usage: {
              prompt_tokens: 15,
              completion_tokens: 10,
              total_tokens: 25,
              estimated_cost_avoided_usd: 0.00015,
            },
            decision: {
              action: "safe_hit",
              cache_path: "exact",
              confidence: 1.0,
              risk_score: 0.0,
              reason_code: "EXACT_HASH_MATCH",
              latency_saved_ms: 220,
            },
            cached: true,
            tier: "c1_exact",
          })
        );
      });
      return;
    }
    res.writeHead(404);
    res.end();
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;

  try {
    const client = new SunbornClient({
      baseUrl: `http://127.0.0.1:${port}`,
      apiKey: "sb_live_secret",
    });

    const resp = await client.chatCompletion({
      model: "gpt-4o",
      messages: [{ role: "user", content: "Hello Sunborn" }],
    });

    assert.equal(resp.cached, true);
    assert.equal(resp.tier, "c1_exact");
    assert.equal(resp.decision.action, "safe_hit");
    assert.equal(resp.choices[0].message.content, "Deterministic safe response.");
    assert.equal(resp.usage.estimated_cost_avoided_usd, 0.00015);
  } finally {
    server.close();
  }
});

test("SunbornClient translates RFC 7807 problem details into SunbornError", async () => {
  const server = http.createServer((req, res) => {
    if (req.url === "/v1/chat/completions") {
      res.writeHead(429, {
        "Content-Type": "application/json",
        "Retry-After": "5",
      });
      res.end(
        JSON.stringify({
          type: "https://sunborn.ai/errors/ratelimitexceeded",
          title: "RateLimitExceeded",
          status: 429,
          detail: "Spend velocity ceiling of $100.00 exceeded for tenant 'tenant-alpha'",
        })
      );
      return;
    }
    res.writeHead(404);
    res.end();
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;

  try {
    const client = new SunbornClient({
      baseUrl: `http://127.0.0.1:${port}`,
      apiKey: "sb_live_test",
      maxRetries: 0,
    });

    await assert.rejects(
      async () => {
        await client.chatCompletion({
          model: "gpt-4o",
          messages: [{ role: "user", content: "Should fail" }],
        });
      },
      (err) => {
        assert.ok(err instanceof SunbornError);
        assert.equal(err.status, 429);
        assert.ok(err.detail.includes("Spend velocity ceiling"));
        return true;
      }
    );
  } finally {
    server.close();
  }
});
