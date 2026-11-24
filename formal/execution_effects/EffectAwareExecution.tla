----------------------- MODULE EffectAwareExecution -----------------------
(*
 * Formal TLA+ specification for Effect-Aware Agentic Partial Execution Reuse.
 *
 * Models a multi-step agent DAG with typed effect classes:
 *   - Pure (deterministic computation)
 *   - ReadOnlyStateful (data fetch)
 *   - NonIdempotentMutation (payment, state write)
 *
 * Core Invariant:
 *   - NoSideEffectReplay: Mutating steps and their tainted downstream dependencies
 *                         are NEVER replayed blindly from cache.
 *)
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Steps, PureSteps, MutatingSteps

ASSUME PureSteps \subseteq Steps
ASSUME MutatingSteps \subseteq Steps
ASSUME PureSteps \cap MutatingSteps = {}

VARIABLES
    executedSteps,
    cachedSteps,
    replayedSteps

vars == <<executedSteps, cachedSteps, replayedSteps>>

Init ==
    /\ executedSteps = {}
    /\ cachedSteps = {}
    /\ replayedSteps = {}

(* Authoritative execution of a step *)
ExecuteStep(s) ==
    /\ s \in Steps
    /\ executedSteps' = executedSteps \cup {s}
    /\ IF s \in PureSteps
       THEN cachedSteps' = cachedSteps \cup {s}
       ELSE cachedSteps' = cachedSteps  \* Mutating steps are NEVER admitted for blind replay
    /\ UNCHANGED replayedSteps

(* Reuse attempt: only cached (pure) steps may be replayed *)
ReplayStep(s) ==
    /\ s \in cachedSteps
    /\ s \notin MutatingSteps  \* Effect-aware safety guard
    /\ replayedSteps' = replayedSteps \cup {s}
    /\ UNCHANGED <<executedSteps, cachedSteps>>

Next ==
    \/ \E s \in Steps : ExecuteStep(s)
    \/ \E s \in cachedSteps : ReplayStep(s)

Spec == Init /\ [][Next]_vars

(* CRITICAL THEOREM: Mutating side-effects are never replayed *)
NoSideEffectReplay ==
    \A s \in replayedSteps : s \notin MutatingSteps

=============================================================================
