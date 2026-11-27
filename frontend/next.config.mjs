import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// The Content-Security-Policy is set per request in `src/middleware.ts`, because
// a nonce has to be generated per response. Keeping a second, nonce-less policy
// here would be enforced alongside it by the browser, and the intersection would
// block the very scripts the nonce authorises. The remaining headers below are
// static, so they stay in the config.

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  output: 'standalone',
  outputFileTracingRoot: __dirname,
  poweredByHeader: false,
  async headers() {
    return [
      {
        source: '/:path*',
        headers: [
          { key: 'X-Frame-Options', value: 'DENY' },
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
          { key: 'Permissions-Policy', value: 'camera=(), microphone=(), geolocation=()' },
          { key: 'Strict-Transport-Security', value: 'max-age=63072000; includeSubDomains; preload' },
          // The legacy XSS auditor is removed from modern browsers and its
          // filtering behaviour was itself exploitable; OWASP recommends an
          // explicit 0 rather than leaving it enabled.
          { key: 'X-XSS-Protection', value: '0' },
        ],
      },
    ];
  },
};

export default nextConfig;
