/**
 * Plain-language explanations for Sunborn's reuse decisions.
 *
 * This module is deliberately free of React and network code so the wording can
 * be unit-tested. It never invents a decision: if the gateway does not return
 * per-check evidence, every gate is reported as "not reported" rather than as a
 * pass.
 */

export type GateId = "G1" | "G2" | "G3" | "G4" | "G5";

export interface RawGateCheck {
  passed: boolean;
  reason_code: string;
}

export interface RawFiveGateEvidence {
  g1_identity_equivalence: RawGateCheck;
  g2_context_authorization: RawGateCheck;
  g3_dependency_freshness: RawGateCheck;
  g4_execution_effects: RawGateCheck;
  g5_evidence_policy: RawGateCheck;
  eligible_reuse: boolean;
}

export interface RawDecision {
  action?: string;
  cache_path?: string;
  reason_code?: string;
  confidence?: number;
  risk_score?: number;
  five_gate_evidence?: RawFiveGateEvidence | null;
}

export interface PlainGate {
  id: GateId;
  /** The plain question this gate answers. */
  question: string;
  passed: boolean;
  statusLabel: string;
  explanation: string;
  reasonCode: string;
}

export interface PlainOutcome {
  tone: "reused" | "fresh";
  headline: string;
  summary: string;
  why: string[];
  gates: PlainGate[];
  evidenceReported: boolean;
}

/** The five questions, written for someone who has never read the paper. */
export const GATE_QUESTIONS: Record<GateId, string> = {
  G1: "Is it really the same question?",
  G2: "Is it the same person, account and permission?",
  G3: "Is the information still up to date?",
  G4: "Is it safe to skip or repeat a real action?",
  G5: "Can Sunborn prove the saved answer was not altered?",
};

const GATE_PASS_TEXT: Record<GateId, { label: string; explanation: string }> = {
  G1: {
    label: "Same meaning",
    explanation:
      "After tidying up the wording, this is the same task: same numbers, same names and same polarity (no added or removed “not”).",
  },
  G2: {
    label: "Same account and permission",
    explanation:
      "The earlier answer belongs to this tenant and application, the model is allowed for this key, and the user and conversation scope match.",
  },
  G3: {
    label: "Still current",
    explanation:
      "The source documents, permissions, configuration, model and policy versions behind the saved answer are still valid.",
  },
  G4: {
    label: "No action skipped",
    explanation:
      "Reusing this result does not quietly skip or repeat a real-world action such as a payment, an email or a database write.",
  },
  G5: {
    label: "Evidence checked",
    explanation:
      "A signed record ties the answer to this request. This proves the record was not altered; it does not prove the answer is true.",
  },
};

const GATE_FAIL_TEXT: Record<GateId, string> = {
  G1: "The earlier request was not the same task. A changed number, name, negation or parameter is enough to make it different.",
  G2: "The account, application, user, conversation or permission does not match the saved answer.",
  G3: "Something the answer depends on may have changed, so the saved copy is not proven current.",
  G4: "This request could change something outside the model (a payment, an email or a write). Reusing it could skip or repeat that real action, so reuse was refused.",
  G5: "The evidence for the saved answer could not be verified, so reuse was refused.",
};

const SPECIFIC_REASONS: Record<string, string> = {
  EXACT_CANONICAL_MATCH:
    "This is byte-for-byte the same request as one already stored, after normalisation.",
  NO_CACHE_ENTRY_FOUND:
    "Nothing safe to reuse was stored yet, so Sunborn asked the model.",
  G2_UNAUTHENTICATED: "Sunborn could not confirm who is asking, so nothing was reused.",
  G2_TENANT_OR_APP_MISMATCH:
    "The saved answer belongs to a different tenant or application.",
  G2_MODEL_UNAUTHORIZED: "This key is not allowed to use that model.",
  G2_SESSION_SCOPE_MISMATCH:
    "The saved answer belongs to a different conversation.",
  G2_USER_SCOPE_MISMATCH: "The saved answer belongs to a different user.",
  G4_SIDE_EFFECTING_TOOLS_PRESENT:
    "The request can perform a real action, so repeating or skipping it would be unsafe.",
  G4_EFFECT_UNCLASSIFIED:
    "Sunborn could not tell whether this request performs a real action, so it refused to reuse.",
  G4_EFFECT_PROHIBITS_REUSE:
    "This kind of action is not safe to reuse, so the request went to the model again.",
  G4_EXECUTION_DAG_CONTAINS_MUTATING_OR_TAINTED_NODES:
    "Part of this workflow writes or depends on a write, so its result cannot be replayed.",
  G4_MALFORMED_EXECUTION_PLAN_ABSTAIN:
    "The execution plan could not be read, so Sunborn refused to reuse.",
  G3_STALE_FENCE_ABORTED_ADMISSION:
    "The saved copy was older than the current data version, so it was not reused.",
  G5_CACHE_ADMISSION_SKIPPED:
    "The answer was returned, but it was not saved for reuse because its evidence could not be checked.",
  G5_CERTIFICATE_VERIFIED:
    "The signed record matched the request and was not altered.",
};

/**
 * Turns a raw reason code into one sentence a non-specialist can act on.
 * Unknown codes fall back to the gate's meaning, never to the raw code, so a
 * new server-side code cannot leak developer jargon into the page.
 */
export function plainReason(reasonCode: string | undefined | null): string {
  if (!reasonCode) {
    return "Sunborn did not return a reason for this decision.";
  }
  const base = reasonCode.split(":")[0].trim();
  if (SPECIFIC_REASONS[base]) {
    return SPECIFIC_REASONS[base];
  }
  const gate = base.slice(0, 2) as GateId;
  if (gate in GATE_FAIL_TEXT) {
    return GATE_FAIL_TEXT[gate];
  }
  return "Sunborn did not recognise this decision code, so treat the reuse as unproven.";
}

function readGate(
  id: GateId,
  check: RawGateCheck | undefined
): PlainGate | null {
  if (!check || typeof check.passed !== "boolean") {
    return null;
  }
  const pass = GATE_PASS_TEXT[id];
  const fail = GATE_FAIL_TEXT[id];
  return {
    id,
    question: GATE_QUESTIONS[id],
    passed: check.passed,
    statusLabel: check.passed ? pass.label : "Not proven",
    explanation: check.passed ? pass.explanation : fail,
    reasonCode: check.reason_code,
  };
}

/**
 * Returns one row per gate that the gateway actually reported.
 * An empty array means "no evidence was returned" — the caller must say so.
 */
export function gateChecks(
  evidence: RawFiveGateEvidence | null | undefined
): PlainGate[] {
  if (!evidence) {
    return [];
  }
  const rows = [
    readGate("G1", evidence.g1_identity_equivalence),
    readGate("G2", evidence.g2_context_authorization),
    readGate("G3", evidence.g3_dependency_freshness),
    readGate("G4", evidence.g4_execution_effects),
    readGate("G5", evidence.g5_evidence_policy),
  ];
  return rows.filter((row): row is PlainGate => row !== null);
}

function wasReused(decision: RawDecision | null | undefined, cached?: boolean): boolean {
  if (typeof cached === "boolean") {
    return cached;
  }
  const path = decision?.cache_path ?? "";
  return path === "exact" || path === "semantic" || path === "execution";
}

/**
 * Produces the headline, summary and reasons shown to the user.
 * The wording never upgrades a fresh answer into a safe-reuse claim.
 */
export function explainDecision(
  decision: RawDecision | null | undefined,
  options: { cached?: boolean } = {}
): PlainOutcome {
  const gates = gateChecks(decision?.five_gate_evidence);
  const reused = wasReused(decision, options.cached);
  const failed = gates.filter((gate) => !gate.passed);
  const allReportedPassed =
    decisionsReportAllPassed(gates);

  if (reused) {
    return {
      tone: "reused",
      headline: "Reused an earlier answer",
      summary: allReportedPassed
        ? "Sunborn did not call the model again. All five checks passed, so the stored answer was returned."
        : "Sunborn returned a stored answer, but not every check was reported. Read the reasons below before trusting it.",
      why:
        failed.length > 0
          ? failed.map((gate) => gate.explanation)
          : ["All five checks passed."],
      gates,
      evidenceReported: gates.length === 5,
    };
  }

  const why =
    failed.length > 0
      ? failed.map((gate) => gate.explanation)
      : [plainReason(decision?.reason_code)];

  return {
    tone: "fresh",
    headline: "Answered fresh",
    summary:
      failed.length > 0
        ? "At least one safety check did not pass, so Sunborn asked the model again instead of reusing. That is the safe direction."
        : allReportedPassed
          ? "Sunborn asked the model once and checked the result before saving it. Ask the same question again to see whether it is reused."
          : "Nothing safe to reuse was found, so Sunborn asked the model.",
    why: failed.length > 0 ? why : [plainReason(decision?.reason_code)],
    gates,
    evidenceReported: gates.length === 5,
  };
}

function decisionsReportAllPassed(gates: PlainGate[]): boolean {
  return gates.length === 5 && gates.every((gate) => gate.passed);
}

/**
 * Error text for the page. `detail` is shown only when it is short and does not
 * look like an internal stack trace.
 */
export function plainHttpError(
  status: number,
  body: { detail?: unknown; error?: unknown } | null
): string {
  const detail =
    typeof body?.detail === "string"
      ? body.detail
      : typeof body?.error === "string"
        ? body.error
        : "";

  switch (status) {
    case 0:
      return "Could not reach Sunborn. Check that the gateway is running on 127.0.0.1:8080.";
    case 401:
      return "Sunborn needs a valid key or a signed-in session before it can answer.";
    case 403:
      return "This key is not allowed to do that.";
    case 404:
      return "This Sunborn build does not expose the request endpoint.";
    case 413:
      return "The question is longer than the gateway accepts.";
    case 429:
      return "This account has reached its rate or spending limit.";
    case 503:
      return "A required part of Sunborn is unavailable. It refused to answer rather than serve an unverified result.";
    default:
      if (detail && detail.length <= 240) {
        return detail;
      }
      return `Sunborn returned HTTP ${status}.`;
  }
}
