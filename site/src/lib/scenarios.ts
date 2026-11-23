export interface Scenario {
  id: string;
  label: string;
  prompt: string;
  expect: string;
}

/** A guided tour. Each step builds on the cache state left by the one before. */
export const SCENARIOS: Scenario[] = [
  { id: "ask", label: "Ask a question", prompt: "What is the capital of France?", expect: "Miss: goes to the model" },
  { id: "repeat", label: "Ask it again", prompt: "what is the capital of france", expect: "Exact hit" },
  { id: "paraphrase", label: "Paraphrase it", prompt: "Tell me France's capital city", expect: "Semantic hit" },
  { id: "num-a", label: "A conversion", prompt: "Convert a 5 km run into miles for my training log", expect: "Miss" },
  { id: "num-b", label: "Change the number", prompt: "Convert a 10 km run into miles for my training log", expect: "Refused: number changed" },
  { id: "neg-a", label: "A diet question", prompt: "Which foods should I eat before a long run?", expect: "Miss" },
  { id: "neg-b", label: "Flip it with “not”", prompt: "Which foods should I not eat before a long run?", expect: "Refused: negation" },
];

/** Canned origin answers so simulation mode reads naturally. */
export function simulatedAnswer(prompt: string): string {
  const p = prompt.toLowerCase();
  const km = p.match(/(\d+(?:\.\d+)?)\s*km/);
  if (km && p.includes("mile")) {
    const n = Number(km[1]);
    return `${n} km is about ${(n * 0.621371).toFixed(2)} miles.`;
  }
  if (p.includes("france") && p.includes("capital")) return "Paris is the capital of France.";
  if (p.includes("food") && /\bnot\b|n't|avoid/.test(p)) {
    return "Skip high-fat, high-fibre and very spicy foods, and large portions of anything new to you. They digest slowly and can upset your stomach mid-run.";
  }
  if (p.includes("food") || p.includes("eat")) {
    return "Easy carbohydrates 1–3 hours before: oatmeal, a banana, toast with honey or a plain bagel, plus some water.";
  }
  if (p.includes("stretch") && p.includes("after")) return "Static stretches held for 20–30 seconds work well after running.";
  if (p.includes("stretch")) return "Dynamic moves such as leg swings and walking lunges work best before running.";
  return "This is a simulated model answer. Switch to Live mode with a running gateway to get a real response from Gemini.";
}
