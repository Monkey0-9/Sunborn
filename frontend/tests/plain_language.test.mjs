import test from "node:test";
import assert from "node:assert/strict";

const {
  explainDecision,
  gateChecks,
  plainHttpError,
  plainReason,
} = await import("../src/lib/plain-language.ts");

function gate(passed, reason_code = "G_OK") {
  return { passed, reason_code };
}

function evidence(overrides = {}) {
  return {
    g1_identity_equivalence: gate(true, "G1_CANONICAL_IDENTITY_VERIFIED"),
    g2_context_authorization: gate(true, "G2_CONTEXT_AUTHORIZED"),
    g3_dependency_freshness: gate(true, "G3_FRESHNESS_FENCE_VALID"),
    g4_execution_effects: gate(true, "G4_EFFECT_SAFE"),
    g5_evidence_policy: gate(true, "G5_CERTIFICATE_VERIFIED"),
    eligible_reuse: true,
    ...overrides,
  };
}

test("gateChecks reports every gate the gateway returned, in order", () => {
  const rows = gateChecks(evidence());
  assert.equal(rows.length, 5);
  assert.deepEqual(
    rows.map((row) => row.id),
    ["G1", "G2", "G3", "G4", "G5"]
  );
  assert.ok(rows.every((row) => row.passed));
});

test("a failed gate is never upgraded to a pass", () => {
  const rows = gateChecks(
    evidence({
      g2_context_authorization: gate(false, "G2_USER_SCOPE_MISMATCH"),
      eligible_reuse: false,
    })
  );
  const g2 = rows.find((row) => row.id === "G2");
  assert.equal(g2.passed, false);
  assert.equal(g2.statusLabel, "Not proven");
  assert.match(g2.explanation, /account|permission/i);
});

test("an exact hit with all gates is described as a reuse", () => {
  const outcome = explainDecision(
    {
      cache_path: "exact",
      reason_code: "EXACT_CANONICAL_MATCH",
      five_gate_evidence: evidence(),
    },
    { cached: true }
  );
  assert.equal(outcome.tone, "reused");
  assert.equal(outcome.headline, "Reused an earlier answer");
  assert.equal(outcome.evidenceReported, true);
  assert.doesNotMatch(outcome.summary, /asked the model/i);
});

test("a miss is described as fresh work, with a plain reason", () => {
  const outcome = explainDecision(
    {
      cache_path: "Miss",
      reason_code: "NO_CACHE_ENTRY_FOUND",
      five_gate_evidence: evidence(),
    },
    { cached: false }
  );
  assert.equal(outcome.tone, "fresh");
  assert.ok(outcome.why.some((reason) => /Nothing safe to reuse/i.test(reason)));
});

test("a refused reuse explains the failing check, not the raw code", () => {
  const outcome = explainDecision(
    {
      cache_path: "Miss",
      reason_code: "G4_SIDE_EFFECTING_TOOLS_PRESENT",
      five_gate_evidence: evidence({
        g4_execution_effects: gate(false, "G4_SIDE_EFFECTING_TOOLS_PRESENT"),
        eligible_reuse: false,
      }),
    },
    { cached: false }
  );
  assert.equal(outcome.tone, "fresh");
  assert.ok(outcome.why.some((reason) => /real action/i.test(reason)));
  assert.ok(outcome.why.every((reason) => !reason.includes("G4_")));
});

test("known reason codes become plain sentences", () => {
  for (const code of [
    "G2_TENANT_OR_APP_MISMATCH",
    "G3_STALE_FENCE_ABORTED_ADMISSION",
    "G5_CACHE_ADMISSION_SKIPPED",
    "EXACT_CANONICAL_MATCH",
  ]) {
    const text = plainReason(code);
    assert.ok(text.length > 20, `${code} should produce a sentence`);
    assert.ok(!text.includes(code), `${code} should not leak verbatim`);
  }
});

test("an unknown reason code falls back to the gate meaning", () => {
  const text = plainReason("G3_SOMETHING_NEW");
  assert.match(text, /changed|current/i);
  assert.ok(!text.includes("SOMETHING_NEW"));
});

test("missing evidence is reported as missing, never as a clean record", () => {
  assert.deepEqual(gateChecks(null), []);
  const outcome = explainDecision({ cache_path: "Miss" }, { cached: false });
  assert.equal(outcome.evidenceReported, false);
  assert.equal(outcome.gates.length, 0);

  const reusedWithoutEvidence = explainDecision(
    { cache_path: "exact" },
    { cached: true }
  );
  assert.equal(reusedWithoutEvidence.tone, "reused");
  assert.equal(reusedWithoutEvidence.evidenceReported, false);
  assert.match(reusedWithoutEvidence.summary, /not every check was reported/i);
});

test("the first request is not claimed to have reused anything", () => {
  const outcome = explainDecision(
    { cache_path: "Miss", reason_code: "NO_CACHE_ENTRY_FOUND", five_gate_evidence: evidence() },
    { cached: false }
  );
  assert.equal(outcome.tone, "fresh");
  assert.match(outcome.summary, /ask the same question again/i);
});

test("HTTP failures are explained without exposing internals", () => {
  assert.match(plainHttpError(401, null), /key|signed-in/i);
  assert.match(plainHttpError(503, null), /refused to answer/i);
  assert.match(plainHttpError(0, null), /127\.0\.0\.1:8080/);
  assert.match(plainHttpError(429, null), /limit/i);
  const longInternal = "x".repeat(400);
  assert.ok(!plainHttpError(500, { detail: longInternal }).includes(longInternal));
});
