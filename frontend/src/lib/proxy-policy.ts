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

/**
 * Normalizes request paths, collapsing duplicate slashes, resolving . and ..,
 * and preventing path traversal bypasses.
 */
export function normalizePath(rawPath: string): string {
  let decoded = rawPath;
  try {
    decoded = decodeURIComponent(rawPath);
  } catch {
    // Keep raw decoded if percent-decoding throws
  }
  const segments = decoded.replace(/\/+/g, "/").split("/");
  const resolved: string[] = [];
  for (const seg of segments) {
    if (!seg || seg === ".") continue;
    if (seg === "..") {
      resolved.pop();
    } else {
      resolved.push(seg);
    }
  }
  return resolved.join("/");
}

export function isPrivilegedRoute(path: string): boolean {
  const normalized = normalizePath(path);
  return PRIVILEGED_ADMIN_PREFIXES.some(
    (prefix) => normalized === prefix || normalized.startsWith(`${prefix}/`)
  );
}

export function isPublicRoute(path: string): boolean {
  const normalized = normalizePath(path);
  return PUBLIC_ROUTES.has(normalized);
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
  isCookieAuthenticated?: boolean;
}): AuthDecision {
  const {
    method,
    normalizedPath: rawPath,
    clientCredential,
    origin,
    host,
    telemetryKey,
    isCookieAuthenticated = false,
  } = params;

  const normalizedPath = normalizePath(rawPath);
  const isMutating = ["POST", "PUT", "PATCH", "DELETE"].includes(method.toUpperCase());

  // 1. CSRF validation for mutating requests
  if (isMutating) {
    // If request uses ambient cookie authentication, Origin and Host are MANDATORY
    if (isCookieAuthenticated) {
      if (!host) {
        return {
          allowed: false,
          status: 400,
          error: "MISSING_HOST_HEADER",
          detail: "Mutating cookie-authenticated requests require an explicit Host header.",
        };
      }
      if (!origin) {
        return {
          allowed: false,
          status: 403,
          error: "CSRF_MISSING_ORIGIN",
          detail: "Cross-site request forgery protection requires a valid Origin header for state-mutating requests with cookie authentication.",
        };
      }
    }

    if (origin) {
      if (!host) {
        return {
          allowed: false,
          status: 400,
          error: "MISSING_HOST_HEADER",
          detail: "Request specifying an Origin header must also include a Host header.",
        };
      }

      try {
        const originUrl = new URL(origin);

        // Disallow dangerous or unexpected protocols
        if (!["http:", "https:"].includes(originUrl.protocol)) {
          return {
            allowed: false,
            status: 403,
            error: "UNSAFE_ORIGIN_PROTOCOL",
            detail: `Unsafe origin protocol '${originUrl.protocol}'. Only HTTP/HTTPS origins are permitted.`,
          };
        }

        // Exact origin match against host
        const isExactHostMatch = originUrl.host === host;

        // Exact localhost / loopback matching (rejects evil-localhost.com, localhost.attacker.com)
        const hostNameOnly = host.split(":")[0];
        const isLocalHost =
          hostNameOnly === "localhost" ||
          hostNameOnly === "127.0.0.1" ||
          hostNameOnly === "[::1]";

        const isLocalOrigin =
          originUrl.hostname === "localhost" ||
          originUrl.hostname === "127.0.0.1" ||
          originUrl.hostname === "[::1]";

        const isPermittedLocalDev = isLocalHost && isLocalOrigin;

        if (!isExactHostMatch && !isPermittedLocalDev) {
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

  // 4. Deny-by-default for all non-public endpoints regardless of HTTP method
  if (!clientCredential) {
    return {
      allowed: false,
      status: 401,
      error: "AUTHENTICATION_REQUIRED",
      detail: `Access to protected endpoint '/${normalizedPath}' requires authenticated credentials or active session.`,
    };
  }

  return { allowed: true };
}
