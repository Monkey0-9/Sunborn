import { NextRequest, NextResponse } from "next/server";

const GATEWAY_URL =
  process.env.SUNBORN_GATEWAY_URL ||
  process.env.API_URL ||
  "http://127.0.0.1:8080";

const ADMIN_API_KEY =
  process.env.SUNBORN_ADMIN_KEY ||
  process.env.SUNBORN_API_KEY ||
  "";

type RouteContext = {
  params: Promise<{ path?: string[] }>;
};

async function handleProxy(req: NextRequest, context: RouteContext) {
  const { path } = await context.params;
  const pathSegment = path ? path.join("/") : "";
  const targetUrl = new URL(
    pathSegment,
    GATEWAY_URL.endsWith("/") ? GATEWAY_URL : `${GATEWAY_URL}/`
  );

  req.nextUrl.searchParams.forEach((value, key) => {
    targetUrl.searchParams.append(key, value);
  });

  const headers = new Headers();
  req.headers.forEach((value, key) => {
    const lowerKey = key.toLowerCase();
    if (!["host", "connection", "content-length"].includes(lowerKey)) {
      headers.set(key, value);
    }
  });

  // Server-side authentication:
  // If client provided an explicit x-api-key or authorization, keep it;
  // otherwise, inject server-configured secure key without browser exposure.
  const clientKey =
    req.headers.get("x-api-key") || req.headers.get("authorization");
  if (!clientKey && ADMIN_API_KEY) {
    headers.set("Authorization", `Bearer ${ADMIN_API_KEY}`);
    headers.set("x-api-key", ADMIN_API_KEY);
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

    const responseHeaders = new Headers();
    gatewayRes.headers.forEach((value, key) => {
      const lower = key.toLowerCase();
      if (!["content-encoding", "transfer-encoding"].includes(lower)) {
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
