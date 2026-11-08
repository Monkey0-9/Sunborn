import { NextRequest, NextResponse } from "next/server";
import {
  evaluateProxySecurity,
  isPrivilegedRoute,
  isPublicRoute,
} from "@/lib/proxy-policy";
import { handleSimulatedRoute } from "@/lib/standalone-simulator";

const GATEWAY_URL =
  process.env.SUNBORN_GATEWAY_URL ||
  process.env.API_URL ||
  "http://127.0.0.1:8080";

type RouteContext = {
  params: Promise<{ path?: string[] }>;
};

async function handleProxy(req: NextRequest, context: RouteContext) {
  const { path } = await context.params;
  const pathSegment = path ? path.join("/") : "";
  const normalizedPath = pathSegment.replace(/^\/+|\/+$/g, "");

  // Extract Client Credentials
  // Priority: explicit Authorization header -> explicit x-api-key -> session cookie
  const authHeader = req.headers.get("authorization");
  const apiKeyHeader = req.headers.get("x-api-key");
  const sessionCookie = req.cookies.get("sunborn_session")?.value;

  const clientCredential =
    authHeader ||
    (apiKeyHeader ? `Bearer ${apiKeyHeader}` : null) ||
    (sessionCookie ? `Bearer ${sessionCookie}` : null);

  const isCookieAuthenticated = !authHeader && !apiKeyHeader && !!sessionCookie;

  const securityDecision = evaluateProxySecurity({
    method: req.method,
    normalizedPath,
    clientCredential,
    origin: req.headers.get("origin"),
    host: req.headers.get("host"),
    telemetryKey: process.env.SUNBORN_READ_ONLY_TOKEN,
    isCookieAuthenticated,
  });

  if (!securityDecision.allowed) {
    return NextResponse.json(
      { error: securityDecision.error, detail: securityDecision.detail },
      { status: securityDecision.status }
    );
  }

  // 4. Construct Target Gateway URL & Headers
  const targetUrl = new URL(
    pathSegment,
    GATEWAY_URL.endsWith("/") ? GATEWAY_URL : `${GATEWAY_URL}/`
  );

  req.nextUrl.searchParams.forEach((value, key) => {
    targetUrl.searchParams.append(key, value);
  });

  const ALLOWED_REQUEST_HEADERS = new Set([
    "accept",
    "accept-encoding",
    "accept-language",
    "authorization",
    "content-type",
    "user-agent",
    "x-api-key",
    "x-request-id",
    "x-tenant-id",
    "x-application-id",
  ]);

  const headers = new Headers();
  req.headers.forEach((value, key) => {
    const lowerKey = key.toLowerCase();
    if (ALLOWED_REQUEST_HEADERS.has(lowerKey)) {
      headers.set(key, value);
    }
  });

  // Attach verified client credential
  if (clientCredential) {
    headers.set("Authorization", clientCredential);
  } else if (process.env.SUNBORN_READ_ONLY_TOKEN && req.method === "GET") {
    headers.set("Authorization", `Bearer ${process.env.SUNBORN_READ_ONLY_TOKEN}`);
  }

  // Enforce 10MB request payload size cap (BFF-001)
  const contentLength = Number(req.headers.get("content-length") || "0");
  if (contentLength > 10 * 1024 * 1024) {
    return NextResponse.json(
      { error: "PAYLOAD_TOO_LARGE", detail: "Request body exceeds maximum permitted 10MB limit." },
      { status: 413 }
    );
  }

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 60_000);
  let body: BodyInit | undefined = undefined;

  try {
    if (["POST", "PUT", "PATCH"].includes(req.method)) {
      body = await req.text();
    }

    const gatewayRes = await fetch(targetUrl.toString(), {
      method: req.method,
      headers,
      body,
      cache: "no-store",
      signal: controller.signal,
    });
    clearTimeout(timeoutId);

    const DISALLOWED_RESPONSE_HEADERS = new Set([
      "server",
      "x-powered-by",
      "x-aspnet-version",
    ]);

    const responseHeaders = new Headers();
    gatewayRes.headers.forEach((value, key) => {
      const lower = key.toLowerCase();
      if (
        !["content-encoding", "transfer-encoding"].includes(lower) &&
        !DISALLOWED_RESPONSE_HEADERS.has(lower) &&
        !lower.startsWith("internal-") &&
        !lower.startsWith("x-internal-")
      ) {
        responseHeaders.set(key, value);
      }
    });

    const contentType = gatewayRes.headers.get("content-type") || "";
    const isStream = contentType.includes("text/event-stream") || contentType.includes("application/octet-stream");

    if (isStream && gatewayRes.body) {
      return new NextResponse(gatewayRes.body as any, {
        status: gatewayRes.status,
        statusText: gatewayRes.statusText,
        headers: responseHeaders,
      });
    }

    const data = await gatewayRes.arrayBuffer();
    return new NextResponse(data, {
      status: gatewayRes.status,
      statusText: gatewayRes.statusText,
      headers: responseHeaders,
    });
  } catch (err: any) {
    clearTimeout(timeoutId);
    if (err.name === "AbortError") {
      return NextResponse.json(
        { error: "GATEWAY_TIMEOUT", detail: "Gateway request timed out after 60 seconds." },
        { status: 504 }
      );
    }

    // High-Fidelity Standalone Simulation Engine Fallback
    // When live gateway is offline, seamlessly serve mission-critical responses
    // so frontend dashboards, sandboxes, and inspectors function without friction.
    if (process.env.STRICT_GATEWAY !== "true") {
      const simulated = handleSimulatedRoute(req.method, normalizedPath, typeof body === "string" ? body : undefined);
      if (simulated) {
        if (typeof simulated.body === "string") {
          return new NextResponse(simulated.body, {
            status: simulated.status,
            headers: {
              ...simulated.headers,
              "x-sunborn-mode": "standalone-simulation",
            },
          });
        }
        return NextResponse.json(simulated.body, {
          status: simulated.status,
          headers: {
            ...simulated.headers,
            "x-sunborn-mode": "standalone-simulation",
          },
        });
      }
    }

    return NextResponse.json(
      {
        error: "GATEWAY_CONNECTION_FAILED",
        detail: `Next.js Server-Side BFF failed to connect to Sunborn Gateway at ${GATEWAY_URL}: ${err.message}`,
      },
      { status: 502 }
    );
  }
}

export const GET = handleProxy;
export const POST = handleProxy;
export const PUT = handleProxy;
export const DELETE = handleProxy;
