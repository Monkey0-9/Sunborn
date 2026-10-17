import { NextRequest, NextResponse } from "next/server";
import {
  evaluateProxySecurity,
  isPrivilegedRoute,
  isPublicRoute,
} from "@/lib/proxy-policy";

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

  const securityDecision = evaluateProxySecurity({
    method: req.method,
    normalizedPath,
    clientCredential,
    origin: req.headers.get("origin"),
    host: req.headers.get("host"),
    telemetryKey: process.env.SUNBORN_READ_ONLY_TOKEN,
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

  try {
    let body: BodyInit | undefined = undefined;
    if (["POST", "PUT", "PATCH"].includes(req.method)) {
      body = await req.text();
    }

    const gatewayRes = await fetch(targetUrl.toString(), {
      method: req.method,
      headers,
      body,
      cache: "no-store",
    });

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

    const data = await gatewayRes.arrayBuffer();
    return new NextResponse(data, {
      status: gatewayRes.status,
      statusText: gatewayRes.statusText,
      headers: responseHeaders,
    });
  } catch (err: any) {
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
