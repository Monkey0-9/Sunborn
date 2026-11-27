import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const root = path.resolve(__dirname, "..");
const src = path.join(root, "src");

function sourceFiles(dir) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(full);
    return /\.(ts|tsx)$/.test(entry.name) ? [full] : [];
  });
}

const files = sourceFiles(src).map((file) => ({
  path: path.relative(root, file),
  text: fs.readFileSync(file, "utf-8"),
}));

test("no page asserts a risk bound the held-out evaluation does not support", () => {
  // research/results/heldout_generalization_run.json records
  // alpha_supported_by_999_upper_bound: false (99.9% CP upper bound 3.3949%).
  const forbidden = ["α ≤ 0.001", "α≤0.001", "proven 0.0% false-hit"];
  for (const { path: file, text } of files) {
    for (const claim of forbidden) {
      assert.ok(
        !text.includes(claim),
        `${file} still asserts "${claim}", which the results artifact contradicts`
      );
    }
  }
});

test("no page shows an invented risk value or gateway port", () => {
  for (const { path: file, text } of files) {
    assert.ok(
      !text.includes("CONFORMAL RISK: 0.000 (SAFE)"),
      `${file} shows a hardcoded zero risk as SAFE`
    );
    assert.ok(!text.includes("GATEWAY: HTTP 8080"), `${file} hardcodes the gateway port`);
  }
});

test("every cockpit route gets a plain-English explanation", () => {
  const header = fs.readFileSync(path.join(src, "components", "layout", "header.tsx"), "utf-8");
  const band = fs.readFileSync(
    path.join(src, "components", "layout", "plain-page-band.tsx"),
    "utf-8"
  );
  const layout = fs.readFileSync(path.join(src, "app", "layout.tsx"), "utf-8");

  assert.match(layout, /<PlainPageBand\s*\/>/, "the band must be rendered by the root layout");

  const navHrefs = [...header.matchAll(/href: "([^"]+)"/g)].map((m) => m[1]);
  const required = new Set(navHrefs.filter((href) => href !== "/" && href !== "/start"));
  required.add("/login");

  for (const href of required) {
    assert.ok(
      band.includes(`"${href}":`),
      `plain-page-band.tsx has no explanation for ${href}`
    );
  }
});
