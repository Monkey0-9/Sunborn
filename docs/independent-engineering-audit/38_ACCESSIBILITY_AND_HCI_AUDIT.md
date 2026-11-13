# 38 — Accessibility & HCI Audit

**Target claimed:** WCAG 2.2 AA/AAA. **Actual:** not conformance-level.

---

## 1. What passes

- `<html lang="en">` (`layout.tsx:24`).
- `role="banner"` (`header.tsx:160`), `role="navigation" aria-label` (`:243`).
- Global `*:focus-visible` outline (`globals.css:49-52`).
- Labelled login inputs (`login/page.tsx:191-235`).
- `role="alert"`/`aria-live` on login error.
- `aria-live` toast provider (though unused).
- No `<img>` tags → no missing-alt violations.

## 2. Failures (WCAG references)

| WCAG | Failure | Location |
|---|---|---|
| 1.3.1 / 3.3.2 | 8 unlabelled range sliders | `stress-simulator.tsx:330,352,376,398`; `finops/page.tsx:410,426`; `learning/page.tsx:353`; `explorer/page.tsx:253`; `security/page.tsx:509` |
| 2.1.1 / 4.1.2 | Clickable `<div>`s, no keyboard/role | `knowledge/page.tsx:628-641`; `execution/page.tsx:211-218` |
| 4.1.2 / 1.1.1 | Canvas with click handler, no role/label/tabIndex | `canvas-manifold.tsx:330` |
| 1.4.1 | Colour-only status in tables | `page.tsx:944-960,1620-1690`; `knowledge/page.tsx:640-660` |
| 2.4.6 | Text labels exist but some insufficient hierarchy | multiple |
| 4.1.3 | No status announcements for async updates | several pages |

## 3. HCI / product truth

- **Fake data undermines HCI trust:** the overview renders literals as "live telemetry"; the explorer's "10-D Request Envelope" fields are never sent.
- **Fake success:** purge optimistically removes rows even when DELETE fails (`explorer/page.tsx:99-108`); circuit-breaker toggle sets state on failure (`page.tsx:266-285`).
- **Dead controls:** tenant selector (`header.tsx:277-292`), `includeTools` checkbox (`explorer/page.tsx:175-184`), toast system.
- **No CSP** and no visible "simulation" banner on most pages.

## 4. Test adequacy

`frontend/tests/accessibility_wcag.test.mjs` greps source strings (`lang="en"`, `aria-label`). It passes while the above violations exist. It is not an accessibility test. Replace with rendered-DOM axe + Playwright keyboard tests.

## 5. Verdict

Accessibility: **FAIL** (not AA, not AAA). HCI: misleading due to fabricated telemetry. Accessibility score: **3/10**.
