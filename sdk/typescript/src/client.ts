import {
  ChatCompletionChunk,
  ChatCompletionRequest,
  ChatCompletionResponse,
  GatewayHealth,
  GatewayReadiness,
  SunbornConfig,
} from "./types.js";

declare const process: any;

export class SunbornError extends Error {
  public readonly status: number;
  public readonly code: string;
  public readonly detail?: string;

  constructor(status: number, message: string, code: string = "SUNBORN_ERROR", detail?: string) {
    super(message);
    this.name = "SunbornError";
    this.status = status;
    this.code = code;
    this.detail = detail;
  }
}

export class SunbornClient {
  private readonly baseUrl: string;
  private readonly apiKey?: string;
  private readonly timeoutMs: number;
  private readonly maxRetries: number;
  private readonly backoffFactor: number;

  constructor(config: SunbornConfig = {}) {
    this.baseUrl = (config.baseUrl || process.env.SUNBORN_BASE_URL || "http://localhost:8080").replace(/\/$/, "");
    this.apiKey = config.apiKey || process.env.SUNBORN_API_KEY;
    this.timeoutMs = config.timeoutMs || 30_000;
    this.maxRetries = config.maxRetries !== undefined ? config.maxRetries : 3;
    this.backoffFactor = config.backoffFactor || 1.5;
  }

  private async request<T>(path: string, options: RequestInit = {}): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const headers: Record<string, string> = {
      "User-Agent": "Sunborn-TypeScript-SDK/1.0.0",
      ...(options.headers as Record<string, string>),
    };

    if (this.apiKey) {
      headers["x-api-key"] = this.apiKey;
    }

    let lastError: Error | null = null;
    let delay = 200;

    for (let attempt = 0; attempt <= this.maxRetries; attempt++) {
      try {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), this.timeoutMs);

        const response = await fetch(url, {
          ...options,
          headers,
          signal: controller.signal,
        });

        clearTimeout(timeout);

        if (response.ok) {
          return (await response.json()) as T;
        }

        const status = response.status;
        let errorMessage = `HTTP ${status}: ${response.statusText}`;
        let errorCode = `HTTP_${status}`;
        let errorDetail: string | undefined;

        try {
          const errorJson = await response.json();
          if (errorJson && typeof errorJson === "object") {
            errorMessage = errorJson.detail || errorJson.message || errorMessage;
            errorCode = errorJson.title || errorJson.code || errorCode;
            errorDetail = errorJson.detail;
          }
        } catch {
          // Response body was not JSON
        }

        // Retry on 429 Too Many Requests or 5xx Server Errors
        if ((status === 429 || status >= 500) && attempt < this.maxRetries) {
          const retryAfter = response.headers.get("retry-after");
          const sleepTime = retryAfter ? parseInt(retryAfter, 10) * 1000 : delay;
          await new Promise((resolve) => setTimeout(resolve, sleepTime));
          delay *= this.backoffFactor;
          continue;
        }

        throw new SunbornError(status, errorMessage, errorCode, errorDetail);
      } catch (err: any) {
        lastError = err;
        if (err.name === "AbortError") {
          lastError = new SunbornError(408, `Request timed out after ${this.timeoutMs}ms`, "TIMEOUT");
        }
        if (attempt < this.maxRetries && (err.name === "AbortError" || err.name === "FetchError")) {
          await new Promise((resolve) => setTimeout(resolve, delay));
          delay *= this.backoffFactor;
          continue;
        }
        break;
      }
    }

    throw lastError || new SunbornError(500, "Unknown request failure");
  }

  /**
   * Dispatches a chat completion request through the Sunborn multi-tier cache.
   * On cache HIT (C0/C1 exact or C2 semantic), returns immediately with zero origin token consumption.
   * On cache MISS or abstention, dispatches to origin LLM, attaches reuse certificate, and returns response.
   */
  public async chatCompletion(request: ChatCompletionRequest): Promise<ChatCompletionResponse> {
    return this.request<ChatCompletionResponse>("/v1/chat/completions", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(request),
    });
  }

  /**
   * Dispatches a streaming chat completion request (Server-Sent Events) through Sunborn.
   * Emits chunks token-by-token as they are streamed from origin or cached replay.
   */
  public async *chatCompletionStream(
    request: ChatCompletionRequest
  ): AsyncIterable<ChatCompletionChunk> {
    const url = `${this.baseUrl}/v1/chat/completions`;
    const headers: Record<string, string> = {
      "User-Agent": "Sunborn-TypeScript-SDK/1.0.0",
      "Content-Type": "application/json",
      Accept: "text/event-stream",
    };
    if (this.apiKey) {
      headers["x-api-key"] = this.apiKey;
    }

    const payload = { ...request, stream: true };
    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      const errText = await response.text();
      throw new SunbornError(response.status, `Streaming request failed: ${errText}`, `HTTP_${response.status}`);
    }

    if (!response.body) {
      throw new SunbornError(500, "Response body is null for streaming request", "EMPTY_STREAM");
    }

    const reader = (response.body as any).getReader();
    const decoder = new TextDecoder("utf-8");
    let buffer = "";

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed || trimmed.startsWith(":")) continue;
          if (trimmed.startsWith("data: ")) {
            const dataStr = trimmed.slice(6).trim();
            if (dataStr === "[DONE]") return;
            try {
              const chunk = JSON.parse(dataStr) as ChatCompletionChunk;
              yield chunk;
            } catch {
              // Ignore unparseable control frames
            }
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  }

  /**
   * Retrieves runtime service liveness status and version.
   */
  public async getHealth(): Promise<GatewayHealth> {
    return this.request<GatewayHealth>("/health", {
      method: "GET",
    });
  }

  /**
   * Checks multi-backend connectivity (Dragonfly, PostgreSQL, Qdrant).
   */
  public async getReady(): Promise<GatewayReadiness> {
    return this.request<GatewayReadiness>("/ready", {
      method: "GET",
    });
  }
}
