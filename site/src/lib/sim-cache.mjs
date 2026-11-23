// In-browser simulation of Sunborn's cache decision path, for the public
// playground. It is deliberately small and runs with no gateway or API key.
//
// It mirrors the order the real gateway uses:
//   1. Tier-1 exact match on a normalised prompt.
//   2. Semantic candidate lookup (here: content-word overlap instead of
//      embeddings).
//   3. Structural guardrails that refuse reuse when the meaning changed even
//      though the wording is close: numbers, negation, direction words.
//   4. Otherwise go to the origin model and store the answer.
//
// Every result is labelled as simulated in the UI. Latency and cost figures
// are illustrative, based on the provider prices configured in the gateway
// (crates/sunborn-gateway/src/state.rs, Gemini: $1.25 in / $5.00 out per 1M).

const STOPWORDS = new Set(
  (
    "a an the is are was were be been being am do does did of to in on at for by with from as " +
    "and or but if then so than that this these those it its i me my you your we our they them " +
    "their he she his her what which who whom how why when where can could would should will shall " +
    "may might must please tell explain give show about into over under up down out just also some " +
    "any all each there here very really"
  ).split(" ")
);

// Very small synonym map so obvious paraphrases land on the same tokens.
const SYNONYMS = {
  whats: "what",
  capital: "capital",
  city: "city",
  summarise: "summarize",
  summary: "summarize",
  summarize: "summarize",
  describe: "explain",
  definition: "define",
  meaning: "define",
  means: "define",
  mean: "define",
  big: "large",
  huge: "large",
  small: "little",
  tiny: "little",
  fast: "quick",
  quickly: "quick",
  speed: "quick",
  purchase: "buy",
  buying: "buy",
  bought: "buy",
  cost: "price",
  costs: "price",
  pricing: "price",
  priced: "price",
  difference: "differ",
  different: "differ",
  differences: "differ",
  vs: "versus",
  compare: "versus",
  comparison: "versus",
};

const NEGATIONS = new Set(["not", "no", "never", "without", "none", "nothing", "cannot", "neither", "nor"]);

// Pairs of words that flip meaning. If one prompt has the left word and the
// other has the right word, reuse is refused.
const DIRECTION_PAIRS = [
  ["increase", "decrease"],
  ["increases", "decreases"],
  ["before", "after"],
  ["above", "below"],
  ["more", "less"],
  ["most", "least"],
  ["higher", "lower"],
  ["max", "min"],
  ["maximum", "minimum"],
  ["buy", "sell"],
  ["add", "remove"],
  ["enable", "disable"],
  ["allow", "deny"],
  ["true", "false"],
  ["first", "last"],
  ["largest", "smallest"],
  ["oldest", "newest"],
];

const DIRECTION_WORDS = new Set(DIRECTION_PAIRS.flat());

export const SEMANTIC_THRESHOLD = 0.6;

const PRICE_IN_PER_TOKEN = 1.25 / 1_000_000;
const PRICE_OUT_PER_TOKEN = 5.0 / 1_000_000;

export function normalize(prompt) {
  return String(prompt)
    .toLowerCase()
    .replace(/n[’']t\b/g, " not")
    .replace(/[’']/g, "")
    .replace(/[^a-z0-9.\s]/g, " ")
    .replace(/\.(?!\d)/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

export function tokenize(prompt) {
  return normalize(prompt)
    .split(" ")
    .filter(Boolean);
}

export function contentWords(prompt) {
  const words = tokenize(prompt)
    // Negation and direction words are left out of the similarity score on
    // purpose: the guardrails judge them separately, which is what lets the
    // playground show "close wording, different meaning" being refused.
    .filter((w) => !STOPWORDS.has(w) && !NEGATIONS.has(w) && !DIRECTION_WORDS.has(w))
    .map((w) => SYNONYMS[w] || w)
    .map((w) => (w.length > 4 && w.endsWith("s") && !/\d/.test(w) ? w.slice(0, -1) : w));
  return new Set(words);
}

export function numbersIn(prompt) {
  return (normalize(prompt).match(/\d+(?:\.\d+)?/g) || []).map((n) => String(Number(n)));
}

function hasNegation(prompt) {
  return tokenize(prompt).some((w) => NEGATIONS.has(w));
}

/** Jaccard similarity of content words, 0..1. */
export function similarity(a, b) {
  const A = contentWords(a);
  const B = contentWords(b);
  if (A.size === 0 && B.size === 0) return 1;
  let inter = 0;
  for (const w of A) if (B.has(w)) inter += 1;
  return inter / (A.size + B.size - inter);
}

/**
 * Returns null when it is safe to reuse, or a short human-readable reason
 * when the guardrails refuse reuse.
 */
export function guardrailViolation(cachedPrompt, newPrompt) {
  const nA = numbersIn(cachedPrompt);
  const nB = numbersIn(newPrompt);
  if (nA.join(",") !== nB.join(",")) {
    const from = nA.length ? nA.join(", ") : "none";
    const to = nB.length ? nB.join(", ") : "none";
    return `Numeric value changed (${from} → ${to})`;
  }
  if (hasNegation(cachedPrompt) !== hasNegation(newPrompt)) {
    return "Negation flipped (one prompt says not/no/never)";
  }
  const tA = new Set(tokenize(cachedPrompt));
  const tB = new Set(tokenize(newPrompt));
  for (const [x, y] of DIRECTION_PAIRS) {
    if ((tA.has(x) && tB.has(y) && !tA.has(y)) || (tA.has(y) && tB.has(x) && !tA.has(x))) {
      return `Direction flipped ("${tA.has(x) ? x : y}" → "${tA.has(x) ? y : x}")`;
    }
  }
  return null;
}

export function estimateTokens(text) {
  return Math.max(1, Math.round(String(text).length / 4));
}

export function originCost(prompt, answer) {
  return estimateTokens(prompt) * PRICE_IN_PER_TOKEN + estimateTokens(answer) * PRICE_OUT_PER_TOKEN;
}

/**
 * Creates a simulated cache. `answerFor(prompt)` supplies the origin answer;
 * `rng` is injectable so tests are deterministic.
 */
export function createSimulatedCache({ answerFor, rng = Math.random } = {}) {
  /** @type {{prompt: string, key: string, answer: string}[]} */
  const entries = [];
  const exact = new Map();

  const between = (lo, hi) => lo + (hi - lo) * rng();

  function query(prompt) {
    const key = normalize(prompt);
    if (!key) throw new Error("Prompt is empty");

    // 1. Exact
    const hit = exact.get(key);
    if (hit) {
      return {
        outcome: "hit",
        tier: "c1_exact",
        answer: hit.answer,
        matchedPrompt: hit.prompt,
        similarity: 1,
        latencyMs: round(between(0.25, 0.6), 2),
        costUsd: 0,
        savedUsd: originCost(prompt, hit.answer),
        guardrail: null,
        naiveWouldReuse: true,
      };
    }

    // 2. Semantic candidate
    let best = null;
    let bestScore = 0;
    for (const e of entries) {
      const s = similarity(e.prompt, prompt);
      if (s > bestScore) {
        best = e;
        bestScore = s;
      }
    }

    if (best && bestScore >= SEMANTIC_THRESHOLD) {
      // 3. Guardrails
      const violation = guardrailViolation(best.prompt, prompt);
      if (!violation) {
        return {
          outcome: "hit",
          tier: "c2_semantic",
          answer: best.answer,
          matchedPrompt: best.prompt,
          similarity: round(bestScore, 2),
          latencyMs: round(between(1.8, 4.2), 2),
          costUsd: 0,
          savedUsd: originCost(prompt, best.answer),
          guardrail: null,
          naiveWouldReuse: true,
        };
      }
      return toOrigin(prompt, key, {
        matchedPrompt: best.prompt,
        similarity: round(bestScore, 2),
        guardrail: violation,
        naiveWouldReuse: true,
      });
    }

    return toOrigin(prompt, key, {
      matchedPrompt: best ? best.prompt : null,
      similarity: round(bestScore, 2),
      guardrail: null,
      naiveWouldReuse: false,
    });
  }

  function toOrigin(prompt, key, extra) {
    const answer = answerFor ? answerFor(prompt) : `Simulated answer for: ${prompt}`;
    const entry = { prompt, key, answer };
    entries.push(entry);
    exact.set(key, entry);
    return {
      outcome: "miss",
      tier: "origin",
      answer,
      latencyMs: round(between(650, 1400), 0),
      costUsd: originCost(prompt, answer),
      savedUsd: 0,
      ...extra,
    };
  }

  return {
    query,
    size: () => entries.length,
    clear: () => {
      entries.length = 0;
      exact.clear();
    },
  };
}

function round(n, digits) {
  const f = 10 ** digits;
  return Math.round(n * f) / f;
}
