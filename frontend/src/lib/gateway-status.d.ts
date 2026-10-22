export function fetchFreshGatewayStatus(
  fetchImpl: (input: string, init: RequestInit) => Promise<Response>,
  nowMs?: number,
  timeoutMs?: number,
): Promise<{ kmsProvider: string; clusterState: string } | null>;
