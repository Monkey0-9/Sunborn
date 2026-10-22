/**
 * Read a fresh, well-formed gateway status response. Null means unavailable or untrusted.
 * The values remain gateway-reported and do not constitute independent verification.
 * @param {(input: string, init: RequestInit) => Promise<Response>} fetchImpl
 * @param {number} nowMs
 * @param {number} timeoutMs
 * @returns {Promise<{kmsProvider: string, clusterState: string} | null>}
 */
export async function fetchFreshGatewayStatus(fetchImpl, nowMs = Date.now(), timeoutMs = 1500) {
  try {
    const response = await fetchImpl("/v1/cluster/status", {
      cache: "no-store",
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!response.ok) return null;

    const value = await response.json();
    if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
    const status = /** @type {Record<string, unknown>} */ (value);
    if (
      typeof status.kms_provider !== "string" ||
      typeof status.cluster_state !== "string" ||
      typeof status.status_observed_at !== "string" ||
      !status.kms_provider.trim() ||
      !status.cluster_state.trim()
    ) {
      return null;
    }

    const observedAt = Date.parse(status.status_observed_at);
    const ageMs = nowMs - observedAt;
    if (!Number.isFinite(observedAt) || ageMs > 60_000 || ageMs < -30_000) return null;

    return { kmsProvider: status.kms_provider, clusterState: status.cluster_state };
  } catch {
    return null;
  }
}
