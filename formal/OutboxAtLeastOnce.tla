----------------------- MODULE OutboxAtLeastOnce -----------------------
(*
 * Abstract model of Sunborn's durable outbox worker.
 *
 * Real behavior modeled:
 *   Claim : FOR UPDATE SKIP LOCKED moves a PENDING (or lease-expired
 *           PROCESSING) row to PROCESSING and records the worker id.
 *   Purge : the purge side effects are applied (idempotent deletes).
 *   Ack   : PROCESSING -> PROCESSED, only after a successful purge.
 *   Crash : the worker dies; its PROCESSING rows become claimable again after
 *           the lease expires. Schema columns and side effects are retained.
 *
 * Properties checked:
 *   NoAckWithoutPurge : a row is never acknowledged before its purge ran.
 *   ProcessedStable   : once acknowledged, a row never returns to the queue.
 *   EventualCompletion: with a bounded number of crashes, every event is
 *                       eventually acknowledged (at-least-once delivery).
 *)
EXTENDS Naturals, TLC

CONSTANTS Event, Worker, MaxCrashes, NoOwner

ASSUME MaxCrashes \in Nat

VARIABLES status, owner, purged, crashes

vars == <<status, owner, purged, crashes>>

Statuses == {"Pending", "Processing", "Processed"}

TypeOK ==
    /\ status \in [Event -> Statuses]
    /\ owner \in [Event -> Worker \union {NoOwner}]
    /\ purged \in [Event -> BOOLEAN]
    /\ crashes \in 0..MaxCrashes

(* NoOwner is a model value distinct from every worker, supplied by the config. *)

Init ==
    /\ status = [e \in Event |-> "Pending"]
    /\ owner = [e \in Event |-> NoOwner]
    /\ purged = [e \in Event |-> FALSE]
    /\ crashes = 0

Claim(w, e) ==
    /\ status[e] = "Pending"
    /\ status' = [status EXCEPT ![e] = "Processing"]
    /\ owner' = [owner EXCEPT ![e] = w]
    /\ UNCHANGED <<purged, crashes>>

(* Purges are idempotent deletes, so re-purging is modeled as a stutter: the
   state only records whether the side effect has been applied at least once. *)
Purge(w, e) ==
    /\ status[e] = "Processing"
    /\ owner[e] = w
    /\ ~purged[e]
    /\ purged' = [purged EXCEPT ![e] = TRUE]
    /\ UNCHANGED <<status, owner, crashes>>

Ack(w, e) ==
    /\ status[e] = "Processing"
    /\ owner[e] = w
    /\ purged[e]
    /\ status' = [status EXCEPT ![e] = "Processed"]
    /\ owner' = [owner EXCEPT ![e] = NoOwner]
    /\ UNCHANGED <<purged, crashes>>

(* Process dies: in-flight rows return to Pending (the lease is expired).
   Side effects already applied are retained, so a re-purge is a repeat, not a
   loss. *)
Crash(w) ==
    /\ crashes < MaxCrashes
    /\ crashes' = crashes + 1
    /\ status' = [e \in Event |->
                     IF status[e] = "Processing" /\ owner[e] = w THEN "Pending" ELSE status[e]]
    /\ owner' = [e \in Event |-> IF owner[e] = w THEN NoOwner ELSE owner[e]]
    /\ UNCHANGED purged

NoCrash == UNCHANGED vars

Next ==
    \/ \E w \in Worker : \E e \in Event : Claim(w, e)
    \/ \E w \in Worker : \E e \in Event : Purge(w, e)
    \/ \E w \in Worker : \E e \in Event : Ack(w, e)
    \/ \E w \in Worker : Crash(w)
    \/ NoCrash

Spec == Init /\ [][Next]_vars
          /\ (\A w \in Worker : WF_vars(Crash(w)))
          /\ (\A w \in Worker : \A e \in Event :
                 WF_vars(Claim(w, e)) /\ WF_vars(Purge(w, e)) /\ WF_vars(Ack(w, e)))

NoAckWithoutPurge == \A e \in Event : status[e] = "Processed" => purged[e]

ProcessedStable == [][\A e \in Event : status[e] = "Processed" => status'[e] = "Processed"]_vars

EventualCompletion == (crashes = MaxCrashes) ~> (\A e \in Event : status[e] = "Processed")

=========================================================================
