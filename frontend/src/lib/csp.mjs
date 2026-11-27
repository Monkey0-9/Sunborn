// Content-Security-Policy construction, kept in plain ESM so the nonce path is
// unit-testable without booting Next.js or a browser.
//
// Why a middleware nonce instead of the static header in `next.config.mjs`:
// `'unsafe-inline'` is ignored by a browser as soon as a nonce or hash is
// present, so keeping both would be a false comfort. Next.js reads the nonce
// from the request's `Content-Security-Policy` header and stamps its own
// bootstrap and chunk scripts with it, which lets the app run under
// `'strict-dynamic'` without a host allowlist.
//
// `style-src` keeps `'unsafe-inline'`: styles are not a script-execution vector,
// and React/Next emit inline `style` attributes for layout in several places.
// The script directive — the one that turns an injection into code execution —
// carries no `'unsafe-inline'` in production.

export const CSP_NONCE_HEADER = 'x-nonce';

/**
 * 128 bits of randomness, base64-encoded. `crypto.getRandomValues` exists in the
 * Edge runtime and in Node, so this does not depend on a Node-only `Buffer`
 * shim. A caller-supplied nonce would defeat the purpose, so none is accepted.
 */
export function generateNonce() {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

/**
 * Builds the policy for one response.
 *
 * @param {{ nonce: string, isDevelopment?: boolean }} options
 * @returns {string}
 */
export function buildContentSecurityPolicy({ nonce, isDevelopment = false }) {
  if (typeof nonce !== 'string' || nonce.length === 0) {
    throw new TypeError('buildContentSecurityPolicy requires a non-empty nonce');
  }

  // Development needs eval for React Refresh/HMR. Production must never have it:
  // it turns any markup injection into arbitrary code execution.
  const scriptSrc = ["'self'", `'nonce-${nonce}'`, "'strict-dynamic'"];
  if (isDevelopment) {
    scriptSrc.push("'unsafe-eval'");
  }

  return [
    "default-src 'self'",
    `script-src ${scriptSrc.join(' ')}`,
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: blob:",
    "font-src 'self' data:",
    "connect-src 'self'",
    "object-src 'none'",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
  ].join('; ');
}

/**
 * Applies the nonce and policy to a mutable header set. The caller passes the
 * request headers so Next.js can read the nonce from them, and then copies the
 * returned policy onto the response.
 *
 * @param {Headers} headers
 * @param {{ nonce: string, isDevelopment?: boolean }} options
 * @returns {string} the policy that was applied
 */
export function applyContentSecurityPolicy(headers, options) {
  const policy = buildContentSecurityPolicy(options);
  headers.set(CSP_NONCE_HEADER, options.nonce);
  headers.set('Content-Security-Policy', policy);
  return policy;
}
