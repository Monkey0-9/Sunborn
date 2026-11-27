import { NextResponse } from 'next/server';
import type { NextRequest } from 'next/server';
import { applyContentSecurityPolicy, generateNonce } from './lib/csp.mjs';

/**
 * Per-request CSP nonce.
 *
 * The same nonce is written to the request headers (which Next.js reads and
 * stamps onto its own scripts) and to the response policy. Document routes only:
 * static assets under `/_next/static` and `/_next/image` are not documents, and
 * a prefetch request is not a navigation the browser will enforce a policy on.
 */
export function middleware(request: NextRequest) {
  const nonce = generateNonce();
  const requestHeaders = new Headers(request.headers);
  const policy = applyContentSecurityPolicy(requestHeaders, {
    nonce,
    isDevelopment: process.env.NODE_ENV !== 'production',
  });

  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set('Content-Security-Policy', policy);
  return response;
}

export const config = {
  matcher: [
    {
      source: '/((?!_next/static|_next/image|favicon.ico).*)',
      missing: [
        { type: 'header', key: 'next-router-prefetch' },
        { type: 'header', key: 'purpose', value: 'prefetch' },
      ],
    },
  ],
};
