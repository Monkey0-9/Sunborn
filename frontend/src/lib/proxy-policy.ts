// Public endpoints that require no authentication
export const PUBLIC_ROUTES = new Set([
  "health",
  "ready",
  "metrics",
  "v1/auth/login",
]);

// Administrative endpoints that strictly require verified admin authentication
export const PRIVILEGED_ADMIN_PREFIXES = [
  "v1/tenants",
  "v1/governance/shred",
  "v1/learning/canary",
  "v1/learning/promote",
  "v1/learning/rollback",
  "v1/caches",
  "v1/invalidation",
  "v1/execution/evict",
  "v1/rebuild",
  "v1/chaos/trip",
  "v1/chaos/reset",
  "v1/lifecycle/sweep",
];

export function isPrivilegedRoute(normalizedPath: string): boolean {
  return PRIVILEGED_ADMIN_PREFIXES.some(
    (prefix) => normalizedPath === prefix || normalizedPath.startsWith(`${prefix}/`)
  );
}

export function isPublicRoute(normalizedPath: string): boolean {
  return PUBLIC_ROUTES.has(normalizedPath);
}

export type AuthDecision =
  | { allowed: true }
  | { allowed: false; status: number; error: string; detail: string };

export function evaluateProxySecurity(params: {
  method: string;
  normalizedPath: string;
  clientCredential?: string | null;
  origin?: string | null;
  host?: string | null;
  telemetryKey?: string | null;
}): AuthDecision {
  const { method, normalizedPath, clientCredential, origin, host, telemetryKey } = params;

  // 1. CSRF validation for mutating browser requests
  if (["POST", "PUT", "PATCH", "DELETE"].includes(method)) {
    if (origin && host) {
      try {
        const originUrl = new URL(origin);
        if (
          originUrl.host !== host &&
          !originUrl.host.includes("localhost") &&
          !originUrl.host.includes("127.0.0.1")
        ) {
          return {
            allowed: false,
            status: 403,
            error: "CSRF_ORIGIN_MISMATCH",
            detail: `Cross-site request forgery detected. Request origin '${origin}' does not match host '${host}'.`,
          };
        }
      } catch {
        return {
          allowed: false,
          status: 400,
          error: "INVALID_ORIGIN_HEADER",
          detail: "Malformed origin header.",
        };
      }
    }
  }

  // 2. Public route exemption
  if (isPublicRoute(normalizedPath)) {
    return { allowed: true };
  }

  // 3. Privileged route protection: fail-closed without credentials
  if (isPrivilegedRoute(normalizedPath)) {
    if (!clientCredential) {
      return {
        allowed: false,
        status: 401,
        error: "UNAUTHORIZED",
        detail: `Administrative operation on '/${normalizedPath}' requires authenticated administrator credentials. Ambient proxy fallback is disabled.`,
      };
    }
    return { allowed: true };
  }

  // 4. Protected general routes
  if (!clientCredential) {
    if (!telemetryKey && method !== "GET") {
      return {
        allowed: false,
        status: 401,
        error: "AUTHENTICATION_REQUIRED",
        detail: "Missing authentication header or session for protected API endpoint.",
      };
    }
  }

  return { allowed: true };
}
