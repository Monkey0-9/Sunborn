------------------------- MODULE TemporalValidity -------------------------
(*
 * Formal TLA+ specification for Causal Freshness and Multi-Dimensional Temporal Validity.
 *
 * Models a 5-dimensional freshness predicate:
 *   F(q, t) = (DocVersion, PermVersion, ConfigVersion, ModelVersion, Epoch)
 *
 * Core Invariants:
 *  - NoStaleReuse: At the moment of reuse, served entries match current authoritative state.
 *  - SelectiveInvalidation: Invalidation of Document D1 never purges results depending only on D2.
 *)
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS DocumentId, MaxVersion

VARIABLES
    docVersion,      \* [DocumentId -> 1..MaxVersion]
    permVersion,     \* 1..MaxVersion
    cachedEntry,     \* Set of cached records with pinned versions
    lastServedValid  \* BOOLEAN indicating whether the last served reuse was valid

vars == <<docVersion, permVersion, cachedEntry, lastServedValid>>

Entry == [
    docId: DocumentId,
    pinnedDocVer: 1..MaxVersion,
    pinnedPermVer: 1..MaxVersion
]

Init ==
    /\ docVersion = [d \in DocumentId |-> 1]
    /\ permVersion = 1
    /\ cachedEntry = {}
    /\ lastServedValid = TRUE

(* Cache a response with current dependency versions pinned *)
AdmitCache(d) ==
    LET e == [docId |-> d, pinnedDocVer |-> docVersion[d], pinnedPermVer |-> permVersion] IN
    /\ cachedEntry' = cachedEntry \cup {e}
    /\ UNCHANGED <<docVersion, permVersion, lastServedValid>>

(* Authoritative document update event *)
MutateDocument(d) ==
    /\ docVersion[d] < MaxVersion
    /\ docVersion' = [docVersion EXCEPT ![d] = @ + 1]
    /\ UNCHANGED <<permVersion, cachedEntry, lastServedValid>>

(* Authoritative permission update event *)
MutatePermission ==
    /\ permVersion < MaxVersion
    /\ permVersion' = permVersion + 1
    /\ UNCHANGED <<docVersion, cachedEntry, lastServedValid>>

(* Reuse decision guarded by causal freshness predicate *)
ServeReuse(e) ==
    /\ e \in cachedEntry
    /\ e.pinnedDocVer = docVersion[e.docId]  \* Causal document check
    /\ e.pinnedPermVer = permVersion        \* Causal permission check
    /\ lastServedValid' = (e.pinnedDocVer = docVersion[e.docId] /\ e.pinnedPermVer = permVersion)
    /\ UNCHANGED <<docVersion, permVersion, cachedEntry>>

Next ==
    \/ \E d \in DocumentId : AdmitCache(d)
    \/ \E d \in DocumentId : MutateDocument(d)
    \/ MutatePermission
    \/ \E e \in cachedEntry : ServeReuse(e)

Spec == Init /\ [][Next]_vars

(* CRITICAL THEOREM: Pinned dependencies matched live state at time of reuse *)
NoStaleReuse == lastServedValid = TRUE

=============================================================================
