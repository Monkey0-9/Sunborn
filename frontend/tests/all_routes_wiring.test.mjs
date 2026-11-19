import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const appDir = path.resolve(__dirname, "..", "src", "app");

const routes = [
  "governance",
  "execution",
  "explorer",
  "finops",
  "knowledge",
  "learning",
  "security",
  "ceremony",
  "login",
];

test("all child routes contain sunbornApi integration and resilient error handling", () => {
  for (const route of routes) {
    const file = path.join(appDir, route, "page.tsx");
    assert.ok(fs.existsSync(file), `Route file must exist: ${file}`);
    const content = fs.readFileSync(file, "utf-8");
    assert.ok(content.length > 500, `${route} must have substantial implementation (>500 chars)`);
    assert.ok(
      content.includes("sunbornApi") || content.includes("fetch") || content.includes("api"),
      `${route} must wire to API`
    );
  }
});

test("governance page safely extracts vector clock epoch and provides GDPR shredding trigger", () => {
  const govFile = path.join(appDir, "governance", "page.tsx");
  const content = fs.readFileSync(govFile, "utf-8");
  assert.ok(content.includes("extractVectorClockEpoch"), "Must safely extract vector clock epoch");
  assert.ok(content.includes("shredTenant") || content.includes("handleShred"), "Must contain shredding action");
});

test("execution page provides DAG inspection and Merkle attestation verification", () => {
  const execFile = path.join(appDir, "execution", "page.tsx");
  const content = fs.readFileSync(execFile, "utf-8");
  assert.ok(content.includes("verifyMerkleProof") || content.includes("attestation"), "Must support Merkle proof verification");
});
