import {
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
