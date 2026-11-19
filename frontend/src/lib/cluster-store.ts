import { useSyncExternalStore } from "react";

export type PersonaMode = "executive" | "developer";
export type CircuitBreakerStatus = "CLOSED" | "OPEN";

export interface ClusterState {
  persona: PersonaMode;
  circuitBreaker: CircuitBreakerStatus;
  byokApiKey: string | null;
  activeReplicas: number;
  globalEpoch: number;
  simulationActive: boolean;
}

let currentState: ClusterState = {
  persona: "executive",
  circuitBreaker: "CLOSED",
  byokApiKey: null,
  activeReplicas: 6,
  globalEpoch: 42,
  simulationActive: false,
};

type Listener = () => void;
const listeners = new Set<Listener>();

function notify() {
  for (const listener of listeners) {
    listener();
  }
}

export function getClusterState(): ClusterState {
  return currentState;
}

export function setPersona(persona: PersonaMode): void {
  if (currentState.persona !== persona) {
    currentState = { ...currentState, persona };
    notify();
  }
}

export function setCircuitBreaker(circuitBreaker: CircuitBreakerStatus): void {
  if (currentState.circuitBreaker !== circuitBreaker) {
    currentState = { ...currentState, circuitBreaker };
    notify();
  }
}

export function setByokApiKey(byokApiKey: string | null): void {
  if (currentState.byokApiKey !== byokApiKey) {
    currentState = { ...currentState, byokApiKey };
    notify();
  }
}

export function setSimulationActive(simulationActive: boolean): void {
  if (currentState.simulationActive !== simulationActive) {
    currentState = { ...currentState, simulationActive };
    notify();
  }
}

export function subscribeToCluster(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * React Hook for tear-free, SSR-safe reactive cluster state.
 */
export function useClusterStore(): ClusterState {
  return useSyncExternalStore(
    subscribeToCluster,
    getClusterState,
    getClusterState
  );
}
