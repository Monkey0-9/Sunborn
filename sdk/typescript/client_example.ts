/**
 * Sunborn Semantic Execution Gateway — Enterprise TypeScript / Node Integration.
 *
 * Demonstrates type-safe request routing, decision inspection, and latency
 * telemetry tracking for enterprise TypeScript / Node.js services.
 */

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: str;
}

export interface ChatCompletionRequest {
  model: string;
  messages: ChatMessage[];
  temperature?: number;
  stream?: boolean;
}

export interface DecisionTelemetry {
  cache_path: "exact" | "semantic" | "none";
  confidence_score?: number;
  latency_saved_ms?: number;
  merkle_certificate?: string;
  source_documents?: string[];
}

export interface ChatCompletionResponse {
  id: string;
  object: "chat.completion";
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: ChatMessage;
    finish_reason: string;
  }>;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
  decision?: DecisionTelemetry;
}

export class SunbornGatewayClient {
  private readonly baseUrl: string;
  private readonly apiKey: string;
  private readonly tenantId: string;

  constructor(options: { baseUrl?: string; apiKey: string; tenantId?: string }) {
    this.baseUrl = (options.baseUrl || "http://localhost:8080").replace(/\/$/, "");
    this.apiKey = options.apiKey;
    this.tenantId = options.tenantId || "default-tenant";
  }

  /**
   * Dispatches a chat completion request through the Sunborn semantic gateway.
   */
  async createChatCompletion(request: ChatCompletionRequest): Promise<ChatCompletionResponse> {
    const url = `${this.baseUrl}/v1/chat/completions`;
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": `Bearer ${this.apiKey}`,
        "X-API-Key": this.apiKey,
        "X-Tenant-ID": this.tenantId,
      },
      body: JSON.stringify({
        model: request.model,
        messages: request.messages,
        temperature: request.temperature ?? 0.0,
      }),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`Sunborn Gateway Error [${response.status}]: ${errorText}`);
    }

    const payload: ChatCompletionResponse = await response.json();
    return payload;
  }
}

// ============================================================================
// Verification / Example Usage
// ============================================================================

export async function runExample(): Promise<void> {
  const client = new SunbornGatewayClient({
    baseUrl: process.env.SUNBORN_BASE_URL || "http://localhost:8080",
    apiKey: process.env.SUNBORN_API_KEY || "dev-key-alpha",
    tenantId: "financial-services-corp",
  });

  const prompt: ChatCompletionRequest = {
    model: "gpt-4o",
    messages: [
      { role: "system", content: "You are an enterprise compliance auditor." },
      { role: "user", content: "Explain the liquidity coverage ratio (LCR) requirement." },
    ],
    temperature: 0.0,
  };

  try {
    const result = await client.createChatCompletion(prompt);
    console.log(`Model Response: ${result.choices[0]?.message.content?.slice(0, 80)}...`);
    console.log(`Cache Path: ${result.decision?.cache_path || "none"}`);
    if (result.decision?.latency_saved_ms) {
      console.log(`Latency Saved: ${result.decision.latency_saved_ms} ms`);
    }
  } catch (err) {
    console.warn(`Gateway call skipped or failed offline: ${err}`);
  }
}

if (typeof require !== "undefined" && require.main === module) {
  runExample();
}
