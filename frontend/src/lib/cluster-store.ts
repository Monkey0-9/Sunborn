import { useSyncExternalStore } from "react";

export type PersonaMode = "executive" | "developer";
export type CircuitBreakerState = "CLOSED" | "OPEN";

export interface ClusterState {
  persona: PersonaMode;
  circuitBreaker: CircuitBreakerState;
  byokApiKey: string | null;
  vectorEpoch: number;
  isSimulated: boolean;
}

const DEFAULT_STATE: ClusterState = {
  persona: "executive",
  circuitBreaker: "CLOSED",
  byokApiKey: null,
  vectorEpoch: 4192,
  isSimulated: false,
};

let currentState: ClusterState = { ...DEFAULT_STATE };
const listeners = new Set<(state: ClusterState) => void>();

function notify(): void {
  for (const listener of listeners) {
    try {
      listener(currentState);
    } catch {
      // Ignore listener error to protect store stability
    }
  }
}

export function getClusterState(): ClusterState {
  return currentState;
}

export function resetClusterState(): void {
  currentState = { ...DEFAULT_STATE };
  notify();
}

export function setPersona(mode: PersonaMode): void {
  if (currentState.persona !== mode) {
    currentState = { ...currentState, persona: mode };
    notify();
  }
}

export function setCircuitBreaker(state: CircuitBreakerState): void {
  if (currentState.circuitBreaker !== state) {
    currentState = { ...currentState, circuitBreaker: state };
    notify();
  }
}

export function setByokApiKey(key: string | null): void {
  const sanitized = key && key.trim().length > 0 ? key.trim() : null;
  if (currentState.byokApiKey !== sanitized) {
    currentState = { ...currentState, byokApiKey: sanitized };
    notify();
  }
}

export function setVectorEpoch(epoch: number): void {
  if (currentState.vectorEpoch !== epoch) {
    currentState = { ...currentState, vectorEpoch: epoch };
    notify();
  }
}

export function setIsSimulated(sim: boolean): void {
  if (currentState.isSimulated !== sim) {
    currentState = { ...currentState, isSimulated: sim };
    notify();
  }
}

export function subscribeClusterState(listener: (state: ClusterState) => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * React 19 hook providing reactive, zero-tearing access to global cluster & persona state.
 */
export function useClusterStore(): ClusterState {
  return useSyncExternalStore(
    subscribeClusterState,
    getClusterState,
    getClusterState
  );
}

/**
 * Safely extracts a numeric monotonic epoch from arbitrary vector clock representations.
 */
export function extractVectorClockEpoch(vc: unknown, fallback: number = 1): number {
  if (typeof vc === "number" && !Number.isNaN(vc)) {
    return vc;
  }
  if (!vc || typeof vc !== "object") {
    return fallback;
  }
  const obj = vc as Record<string, unknown>;
  if (typeof obj.epoch === "number" && !Number.isNaN(obj.epoch)) {
    return obj.epoch;
  }
  const targetMap =
    obj.clocks && typeof obj.clocks === "object"
      ? (obj.clocks as Record<string, unknown>)
      : obj;

  const numericValues = Object.values(targetMap).filter(
    (v): v is number => typeof v === "number" && !Number.isNaN(v)
  );
  if (numericValues.length > 0) {
    return Math.max(...numericValues);
  }
  return fallback;
}
