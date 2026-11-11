import { NextRequest, NextResponse } from 'next/server';

const GATEWAY_URL = process.env.SUNBORN_GATEWAY_URL || 'http://127.0.0.1:8080';

export async function GET(req: NextRequest) {
  const sessionToken = req.cookies.get('sunborn_session')?.value;
  if (!sessionToken) {
    return NextResponse.json({ authenticated: false }, { status: 200 });
  }

  try {
    const gatewayRes = await fetch(`${GATEWAY_URL}/v1/auth/session`, {
      method: 'GET',
      headers: {
        'Authorization': `Bearer ${sessionToken}`,
      },
    });

    if (!gatewayRes.ok) {
      return NextResponse.json({ authenticated: false }, { status: 200 });
    }

    const sessionData = await gatewayRes.json();
    return NextResponse.json({
      authenticated: true,
      session: {
        active: true,
        cookie_protected: true,
        tenant_id: sessionData.tenant_id,
        application_id: sessionData.application_id,
        is_admin: sessionData.is_admin,
      },
    });
  } catch {
    const isStrict = process.env.STRICT_GATEWAY === "true" || process.env.NODE_ENV === "production";
    if (!isStrict && sessionToken) {
      return NextResponse.json({
        authenticated: true,
        session: {
          active: true,
          cookie_protected: true,
          tenant_id: "dev-sandbox-tenant",
          application_id: "dev-console",
          is_admin: false,
          simulated: true,
        },
      }, {
        headers: { "X-Sunborn-Simulated": "true" }
      });
    }
    // Upstream gateway unreachable: fail-closed in production
    return NextResponse.json(
      { authenticated: false, error: 'Gateway authentication unavailable' },
      { status: 502 }
    );
  }
}

export async function POST(req: NextRequest) {
  // CSRF Protection for state-mutating browser requests
  const origin = req.headers.get('origin');
  const host = req.headers.get('host');
  if (origin && host) {
    const originHost = new URL(origin).host;
    if (originHost !== host) {
      return NextResponse.json(
        { error: 'CSRF origin mismatch rejected' },
        { status: 403 }
      );
    }
  }

  let body: any;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json(
      { error: 'Invalid JSON payload' },
      { status: 400 }
    );
  }

  const { action, client_id, client_secret, tenant_id } = body;

  if (action === 'logout') {
    const res = NextResponse.json({
      success: true,
      message: 'Session cleared successfully',
    });
    res.cookies.set('sunborn_session', '', {
      httpOnly: true,
      secure: process.env.NODE_ENV === 'production',
      sameSite: 'lax',
      path: '/',
      maxAge: 0,
      expires: new Date(0),
    });
    return res;
  }

  if (action === 'login' || !action) {
    if (!client_id || !client_secret) {
      return NextResponse.json(
        { error: 'client_id and client_secret are required' },
        { status: 400 }
      );
    }

    try {
      const gatewayRes = await fetch(`${GATEWAY_URL}/v1/auth/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          client_id,
          client_secret,
          tenant_id: tenant_id || undefined,
        }),
      });

      if (!gatewayRes.ok) {
        const errorData = await gatewayRes.json().catch(() => ({}));
        return NextResponse.json(
          {
            error: errorData.detail || 'Authentication failed',
            status: gatewayRes.status,
          },
          { status: gatewayRes.status }
        );
      }

      const data = await gatewayRes.json();
      const accessToken = data.access_token || client_secret;

      const res = NextResponse.json({
        success: true,
        tenant_id: data.tenant_id,
        application_id: data.application_id,
        is_admin: data.is_admin,
        expires_in: data.expires_in,
      });

      // Set HttpOnly, Secure, SameSite=Lax session cookie
      res.cookies.set('sunborn_session', accessToken, {
        httpOnly: true,
        secure: process.env.NODE_ENV === 'production',
        sameSite: 'lax',
        path: '/',
        maxAge: data.expires_in || 86400,
      });

      return res;
    } catch (err: any) {
      const isStrict = process.env.STRICT_GATEWAY === "true" || process.env.NODE_ENV === "production";
      if (!isStrict && client_id && client_secret) {
        // Offline development sandbox ONLY - never allowed in production
        const tid = tenant_id || "dev-sandbox-tenant";
        const app = "dev-console";
        const res = NextResponse.json({
          success: true,
          tenant_id: tid,
          application_id: app,
          is_admin: false,
          expires_in: 86400,
          simulated: true,
        }, {
          headers: { "X-Sunborn-Simulated": "true" }
        });
        res.cookies.set('sunborn_session', client_secret, {
          httpOnly: true,
          secure: process.env.NODE_ENV === 'production',
          sameSite: 'lax',
          path: '/',
          maxAge: 86400,
        });
        return res;
      }
      return NextResponse.json(
        { error: `Upstream gateway unreachable: ${err.message}` },
        { status: 502 }
      );
    }
  }

  return NextResponse.json(
    { error: `Unsupported auth action: '${action}'` },
    { status: 400 }
  );
}
