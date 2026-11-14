import test from "node:test";
import assert from "node:assert/strict";
import {
  createSimulatedCache,
  guardrailViolation,
  normalize,
  similarity,
} from "../src/lib/sim-cache.mjs";

const fixedRng = () => 0.5;
const make = () => createSimulatedCache({ answerFor: (p) => `answer:${p}`, rng: fixedRng });

test("first request goes to the origin and costs money", () => {
  const cache = make();
  const r = cache.query("What is the capital of France?");
  assert.equal(r.outcome, "miss");
  assert.equal(r.tier, "origin");
  assert.ok(r.costUsd > 0);
  assert.equal(r.savedUsd, 0);
  assert.equal(cache.size(), 1);
});

test("same prompt with different casing and punctuation is an exact hit", () => {
  const cache = make();
  cache.query("What is the capital of France?");
  const r = cache.query("what is the capital of france");
  assert.equal(r.tier, "c1_exact");
  assert.equal(r.answer, "answer:What is the capital of France?");
  assert.equal(r.costUsd, 0);
  assert.ok(r.savedUsd > 0);
  assert.ok(r.latencyMs < 1);
});

test("a paraphrase is a semantic hit", () => {
  const cache = make();
  cache.query("What is the capital of France?");
  const r = cache.query("Tell me France's capital city");
  assert.equal(r.tier, "c2_semantic");
  assert.equal(r.guardrail, null);
});

test("a changed number is refused by the guardrail even though wording matches", () => {
  const cache = make();
  cache.query("Convert a 5 km run into miles for my training log");
  const r = cache.query("Convert a 10 km run into miles for my training log");
  assert.equal(r.outcome, "miss");
  assert.match(r.guardrail, /Numeric value changed \(5 → 10\)/);
  assert.equal(r.naiveWouldReuse, true);
});

test("a negation flip is refused", () => {
  const cache = make();
  cache.query("Which foods should I eat before a long run?");
  const r = cache.query("Which foods should I not eat before a long run?");
  assert.equal(r.outcome, "miss");
  assert.match(r.guardrail, /Negation/);
});

test("contractions count as negation", () => {
  assert.match(guardrailViolation("Should I stretch?", "Shouldn't I stretch?"), /Negation/);
});

test("a direction flip is refused", () => {
  const cache = make();
  cache.query("Should I stretch before running?");
  const r = cache.query("Should I stretch after running?");
  assert.equal(r.outcome, "miss");
  assert.match(r.guardrail, /Direction flipped/);
});

test("unrelated prompts are plain misses with no naive reuse", () => {
  const cache = make();
  cache.query("What is the capital of France?");
  const r = cache.query("How do I bake sourdough bread?");
  assert.equal(r.outcome, "miss");
  assert.equal(r.guardrail, null);
  assert.equal(r.naiveWouldReuse, false);
});

test("empty prompts are rejected and clear() resets state", () => {
  const cache = make();
  assert.throws(() => cache.query("   "), /empty/);
  cache.query("hello world example");
  cache.clear();
  assert.equal(cache.size(), 0);
  assert.equal(cache.query("hello world example").outcome, "miss");
});

test("helpers behave sensibly", () => {
  assert.equal(normalize("  Hello,   WORLD!! "), "hello world");
  assert.equal(normalize("pi is 3.14."), "pi is 3.14");
  assert.equal(similarity("capital of France", "France capital"), 1);
});
