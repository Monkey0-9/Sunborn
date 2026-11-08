import { chromium } from "playwright";

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("http://localhost:3000", { waitUntil: "networkidle" });

  const headerVisible = await page.isVisible("header");
  const mainVisible = await page.isVisible("main");
  const h1Visible = await page.isVisible("h1");
  const textContent = await page.textContent("body");

  console.log("HEADER VISIBLE:", headerVisible);
  console.log("MAIN VISIBLE:", mainVisible);
  console.log("H1 VISIBLE:", h1Visible);
  console.log("BODY LENGTH:", textContent.length);
  console.log("HAS Deterministic:", textContent.includes("Deterministic"));
  console.log("HAS GATEWAY:", textContent.includes("GATEWAY"));

  await page.screenshot({ path: "test_actual_view.png", fullPage: true });
  console.log("FULL PAGE SCREENSHOT SAVED to test_actual_view.png");

  await browser.close();
}

main().catch(console.error);
