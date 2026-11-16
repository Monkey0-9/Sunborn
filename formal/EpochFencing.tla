--------------------------- MODULE EpochFencing ---------------------------
(*
 * Abstract model of Sunborn's authoritative epoch protocol.
 *
 * The real implementation advances epochs in PostgreSQL with
 * GREATEST(current_epoch + 1, proposed) ... RETURNING and refuses to publish a
 * local fence until that write commits. This model checks the two properties
 * that matter:
 *
 *   PersistedAuthority : no replica ever publishes an epoch that the durable
 *                        store has not observed.
 *   MonotonicFences    : a replica's fence never moves backwards.
 *
 * A persistence failure freezes the replica instead of advancing it, which is
 * the fail-closed behavior required by the audit.
 *)
EXTENDS Naturals, TLC

CONSTANTS Replica, MaxEpoch

ASSUME MaxEpoch \in Nat /\ MaxEpoch > 0

VARIABLES persisted, fence, frozen

vars == <<persisted, fence, frozen>>

TypeOK ==
    /\ persisted \in 0..MaxEpoch
    /\ fence \in [Replica -> 0..MaxEpoch]
    /\ frozen \in [Replica -> BOOLEAN]

Init ==
    /\ persisted = 0
    /\ fence = [r \in Replica |-> 0]
    /\ frozen = [r \in Replica |-> FALSE]

(* Persist, then publish. Both happen in one atomic step here; the invariant
   PersistedAuthority would fail if publication were split out and allowed to
   happen first. *)
Advance(r) ==
    /\ ~frozen[r]
    /\ persisted < MaxEpoch
    /\ persisted' = persisted + 1
    /\ fence' = [fence EXCEPT ![r] = persisted + 1]
    /\ UNCHANGED frozen

(* A replica adopts a higher epoch observed from a peer. *)
AdoptPeer(r, s) ==
    /\ fence[r] < fence[s]
    /\ fence' = [fence EXCEPT ![r] = fence[s]]
    /\ UNCHANGED <<persisted, frozen>>

(* Durable write fails: the replica freezes and cannot advance. *)
FailPersist(r) ==
    /\ ~frozen[r]
    /\ frozen' = [frozen EXCEPT ![r] = TRUE]
    /\ UNCHANGED <<persisted, fence>>

Next ==
    \/ \E r \in Replica : Advance(r)
    \/ \E r \in Replica : FailPersist(r)
    \/ \E r, s \in Replica : AdoptPeer(r, s)

Spec == Init /\ [][Next]_vars

PersistedAuthority == \A r \in Replica : fence[r] <= persisted

MonotonicFences == [][\A r \in Replica : fence'[r] >= fence[r]]_vars

(* A persistence failure is not silently cleared: the replica stays frozen.
   (It may still adopt a peer's already-persisted epoch, which cannot make it
   publish anything the durable store has not seen.) *)
FrozenStaysFrozen == [][\A r \in Replica : frozen[r] => frozen'[r]]_vars

=============================================================================
