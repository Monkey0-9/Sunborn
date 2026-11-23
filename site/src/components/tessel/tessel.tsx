"use client"

// Visual engine for the Sunborn showcase, adapted from "Tessel Agent Landing"
// by Kedhareswer Naidu (https://kedhar.pages.dev), MIT License:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions: The above copyright notice and this permission
//   notice shall be included in all copies or substantial portions of the
//   Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.
//
// The look is a living pixel mosaic drawn on canvas: it sharpens out of big
// blocks when the page opens, breaks back into blocks as you scroll past, and
// resolves into finer pixels under the pointer. Headlines are serif, labels
// are mono, and the page sits inside a hairline frame.
//
// What changed from the original: the page is split into a shell (nav, frame,
// footer, theme) and sections that Sunborn's pages compose; the hero prompt
// runs a caller-supplied planner instead of canned agent playbooks; the
// product stage renders caller-supplied cards; the call to action takes
// links instead of an email form; the photo, testimonial and pricing
// sections are gone.
import * as React from "react"
import { gsap } from "gsap"
import { ScrollTrigger } from "gsap/ScrollTrigger"

if (typeof window !== "undefined") gsap.registerPlugin(ScrollTrigger)

export type TslLink = { label: string; href?: string; external?: boolean }

export type TslMenuItem = { label: string; description?: string; href?: string }

export type TslNavItem = { label: string; href?: string; items?: TslMenuItem[]; current?: boolean }

export type TslLogoMark = "bolt" | "spark" | "chevron" | "orbit" | "bars" | "knot" | "leaf"

export type TslLogo = { name: string; mark?: TslLogoMark; style?: "bold" | "italic" | "light" | "caps" }

export type TslProblemIcon = "layers" | "steps" | "eye"

export type TslProblem = {
  kicker: string
  title: string
  items: { title: string; text: React.ReactNode; icon?: TslProblemIcon }[]
}

export type TslFeature = { kicker: string; title: string; text: string; points?: string[]; visual: string }

export type TslSolution = { kicker: string; title: string; features: TslFeature[] }

export type TslStat = { value: number; prefix?: string; suffix?: string; decimals?: number; label: string }

export type TslFooter = { tagline: string; columns: { title: string; links: TslLink[] }[]; note: React.ReactNode; status?: string }

export type TslPaletteName = "cobalt" | "ember" | "moss"

export type TslPaletteInput = { stops?: string[]; accent?: string }

// #region content
const PALETTES = {
  cobalt: { stops: ["#1b36f2", "#4f6bff", "#a3b2ff", "#eceeff"], accent: "#2f4dff" },
  ember: { stops: ["#c2410c", "#f9733a", "#ffc4a1", "#fff2e8"], accent: "#ea580c" },
  moss: { stops: ["#0c6247", "#2b9d74", "#a3dcc3", "#ecf8f2"], accent: "#13805b" },
}
// #endregion content

// #region logic
const clamp01 = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v)

/** Integer hash to [0, 1). */
function hash2(x: number, y: number, s: number) {
  let h = (Math.imul(x | 0, 374761393) + Math.imul(y | 0, 668265263) + Math.imul(s | 0, 1442695041)) | 0
  h = Math.imul(h ^ (h >>> 13), 1274126177)
  h ^= h >>> 16
  return (h >>> 0) / 4294967296
}

/** Smooth value noise in [0, 1]. */
function noise2(x: number, y: number, s: number) {
  const xi = Math.floor(x)
  const yi = Math.floor(y)
  const xf = x - xi
  const yf = y - yi
  const u = xf * xf * (3 - 2 * xf)
  const v = yf * yf * (3 - 2 * yf)
  const a = hash2(xi, yi, s)
  const b = hash2(xi + 1, yi, s)
  const c = hash2(xi, yi + 1, s)
  const d = hash2(xi + 1, yi + 1, s)
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/** Three octaves of value noise, normalised to [0, 1]. */
function fbm(x: number, y: number, s: number) {
  return (noise2(x, y, s) * 0.5 + noise2(x * 2, y * 2, s + 17) * 0.25 + noise2(x * 4, y * 4, s + 34) * 0.125) / 0.875
}

export type TslScene = "columns" | "silk" | "bloom"

/** Brightness of the mosaic at a point: 0 is the deepest stop, 1 the lightest. */
function fieldAt(scene: TslScene, x: number, y: number, t: number, w: number, h: number, shift: number) {
  if (scene === "silk") {
    const n = fbm(x / 240 + t * 0.035 + shift, y / 240 - shift * 0.4, 7)
    const s = Math.sin((x * 0.62 + y) / 85 + n * 5.5 + t * 0.32 + shift * 2)
    return clamp01(0.48 + 0.44 * s + (n - 0.5) * 0.6)
  }
  if (scene === "bloom") {
    const cx = x / w - 0.5
    const cy = y / h - 1.04
    const r = Math.sqrt(cx * cx * 1.5 + cy * cy)
    const n = fbm(x / 190 + t * 0.045, y / 190 - t * 0.035 + shift, 3)
    return clamp01(1.18 - r * 1.2 + (n - 0.5) * 0.6)
  }
  // columns: tall streaks, with a lit pool in the middle where the card sits
  const n = fbm(x / 160 + t * 0.045 + shift, y / 820 - t * 0.028, 11)
  const band = noise2(x / 58 + shift * 3, 0.5, 5)
  const cx = x / w - 0.5
  const cy = y / h - 0.5
  const glow = Math.exp(-(cx * cx * 3.4 + cy * cy * 4.2))
  const v = n * 0.78 + band * 0.24 + glow * 0.46 - 0.26
  return clamp01((v - 0.42) * 1.55 + 0.45)
}

function hexToRgb(hex: string) {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(String(hex).trim())
  if (!m) return [128, 128, 128]
  let h = m[1]
  if (h.length === 3) h = h.split("").map((c) => c + c).join("")
  const n = parseInt(h, 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

function mixHex(a: string, b: string, t: number) {
  const x = hexToRgb(a)
  const y = hexToRgb(b)
  const c = x.map((v, i) => Math.round(v + (y[i] - v) * t))
  return "#" + c.map((v) => v.toString(16).padStart(2, "0")).join("")
}

/** n colours along the stops, as rgb() strings. */
function buildRamp(stops: string[], n: number) {
  const rgb = stops.map(hexToRgb)
  const out: string[] = []
  for (let i = 0; i < n; i++) {
    const f = (n === 1 ? 0 : i / (n - 1)) * (rgb.length - 1)
    const k = Math.min(rgb.length - 2, Math.floor(f))
    const u = f - k
    const a = rgb[k]
    const b = rgb[k + 1]
    out.push("rgb(" + Math.round(a[0] + (b[0] - a[0]) * u) + "," + Math.round(a[1] + (b[1] - a[1]) * u) + "," + Math.round(a[2] + (b[2] - a[2]) * u) + ")")
  }
  return out
}

/** The same mosaic, lowered for a night page so the lightest pixels do not glare. */
function darkStops(stops: string[]) {
  return [mixHex(stops[0], "#000000", 0.55), stops[0], stops[1], mixHex(stops[2], stops[1], 0.3)]
}

function luminance(hex: string) {
  const [r, g, b] = hexToRgb(hex).map((v) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
  })
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** Readable text on a colour. */
function onColor(hex: string) {
  return luminance(hex) > 0.4 ? "#0b0b0c" : "#ffffff"
}

function resolvePalette(p: TslPaletteName | TslPaletteInput | undefined) {
  if (!p) return PALETTES.cobalt
  if (typeof p === "string") return PALETTES[p] || PALETTES.cobalt
  const base = PALETTES.cobalt
  const stops = p.stops && p.stops.length === 4 ? p.stops : base.stops
  return { stops, accent: p.accent || stops[0] }
}

/** "*muted*" runs in copy become muted spans. */
function twoTone(s: string) {
  const out: { text: string; muted: boolean }[] = []
  const parts = String(s).split("*")
  parts.forEach((text, i) => {
    if (text) out.push({ text, muted: i % 2 === 1 })
  })
  return out
}

/** Words of two-tone copy, each carrying its tone. */
function toneWords(s: string) {
  const out: { word: string; muted: boolean }[] = []
  for (const run of twoTone(s)) for (const word of run.text.split(/\s+/)) if (word) out.push({ word, muted: run.muted })
  return out
}

function strHash(s: string) {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
  return h >>> 0
}

export type TslRunStep = { tool: string; label: string; ms: number; time?: string; tone?: "ok" | "warn" | "hit" }

export type TslRunPlan = { kind: string; title: string; steps: TslRunStep[]; answer: string; verdict?: string; tone?: "hit" | "warn" | "miss" }

export type TslPlanner = (prompt: string, model: string) => TslRunPlan | Promise<TslRunPlan>

function compactCount(n: number) {
  if (n >= 1e6) return (Math.round(n / 1e5) / 10).toString().replace(/\.0$/, "") + "M"
  if (n >= 1e3) return (Math.round(n / 100) / 10).toString().replace(/\.0$/, "") + "k"
  return String(Math.round(n))
}

function formatStat(v: number, decimals = 0) {
  return v.toLocaleString("en-US", { minimumFractionDigits: decimals, maximumFractionDigits: decimals })
}

function isEmail(s: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(String(s).trim())
}

function nextIndex(i: number, key: string, n: number) {
  if (key === "ArrowRight" || key === "ArrowDown") return (i + 1) % n
  if (key === "ArrowLeft" || key === "ArrowUp") return (i - 1 + n) % n
  if (key === "Home") return 0
  if (key === "End") return n - 1
  return -1
}

/** A chip-sized version of a suggestion, cut at a word. */
function shortLabel(s: string, max = 32) {
  const t = String(s).trim()
  if (t.length <= max) return t
  const words = t.slice(0, max + 1).split(/\s+/)
  if (words.length > 1) words.pop()
  while (words.length > 2 && /^(and|or|the|a|an|of|to|from|for|with|against|in|on|by)$/i.test(words[words.length - 1])) words.pop()
  return words.join(" ").replace(/[,.;:]+$/, "")
}

/** Pixel sizes a portrait steps through as it resolves. */
function pixelSteps(from: number) {
  const out: number[] = []
  let p = Math.max(1, Math.round(from))
  while (p > 1) {
    out.push(p)
    p = Math.max(1, Math.floor(p * 0.66))
  }
  out.push(1)
  return out
}

const ISO_C = Math.cos(Math.PI / 6)

/** Isometric projection: x runs to the lower right, y to the lower left, z up. */
function isoPt(x: number, y: number, z: number) {
  return [(x - y) * ISO_C, (x + y) * 0.5 - z]
}

/** The three visible faces of a box, as SVG polygon points. */
function isoBox(x: number, y: number, z: number, w: number, d: number, h: number) {
  const p = (a: number, b: number, c: number) => isoPt(a, b, c).map((n) => Math.round(n * 100) / 100).join(",")
  return {
    top: [p(x, y, z + h), p(x + w, y, z + h), p(x + w, y + d, z + h), p(x, y + d, z + h)].join(" "),
    left: [p(x, y + d, z + h), p(x + w, y + d, z + h), p(x + w, y + d, z), p(x, y + d, z)].join(" "),
    right: [p(x + w, y, z + h), p(x + w, y + d, z + h), p(x + w, y + d, z), p(x + w, y, z)].join(" "),
  }
}
// #endregion logic
const TSL_CSS = `
.tsl-root{--tsl-paper:#ffffff;--tsl-panel:#f6f6f8;--tsl-card:#ffffff;--tsl-ink:#0a0a0b;--tsl-soft:#3b3c42;--tsl-muted:#6a6c74;--tsl-faint:#9a9ca4;--tsl-line:#ececf0;--tsl-line2:#dcdde3;--tsl-btn:#0a0a0b;--tsl-on-btn:#ffffff;--tsl-night:#000000;--tsl-night-line:rgba(255,255,255,.1);--tsl-shadow:0 1px 2px rgba(10,12,30,.06),0 18px 40px -18px rgba(18,28,90,.35);--tsl-serif:"Tiempos Headline","Charter","Bitstream Charter","Iowan Old Style","Source Serif 4",Georgia,Cambria,"Times New Roman",serif;--tsl-sans:"Inter","Geist",ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;--tsl-mono:"JetBrains Mono","Geist Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;--tsl-ease:cubic-bezier(.22,1,.36,1);--tsl-nav:60px;--tsl-gut:48px;position:relative;isolation:isolate;overflow-x:clip;background:var(--tsl-paper);color:var(--tsl-ink);font-family:var(--tsl-sans);font-size:15px;line-height:1.55;-webkit-font-smoothing:antialiased;text-rendering:optimizeLegibility;color-scheme:light}
.tsl-root[data-theme="dark"]{--tsl-paper:#0b0c0f;--tsl-panel:#111318;--tsl-card:#14161c;--tsl-ink:#f2f3f6;--tsl-soft:#c5c7cf;--tsl-muted:#8e919b;--tsl-faint:#62656e;--tsl-line:rgba(255,255,255,.08);--tsl-line2:rgba(255,255,255,.14);--tsl-btn:#f2f3f6;--tsl-on-btn:#0a0a0b;--tsl-night:#030304;--tsl-shadow:0 1px 2px rgba(0,0,0,.5),0 22px 50px -20px rgba(0,0,0,.9);color-scheme:dark}
.tsl-root :where(*,*::before,*::after){box-sizing:border-box}
.tsl-root :where(h1,h2,h3,h4,p,ul,ol,li,figure,blockquote,dl,dd){margin:0;padding:0}
.tsl-root :where(ul,ol){list-style:none}
.tsl-root :where(a){color:inherit;text-decoration:none}
.tsl-root :where(button,input,textarea,select){font:inherit;color:inherit;letter-spacing:inherit}
.tsl-root :where(button){background:none;border:0;padding:0;cursor:pointer}
.tsl-root :where(svg,img,canvas){display:block;max-width:none}
.tsl-root :where(a,button,input,textarea,[tabindex]):focus-visible{outline:2px solid var(--tsl-accent);outline-offset:2px}
.tsl-skip{position:absolute;left:12px;top:-60px;z-index:100;background:var(--tsl-btn);color:var(--tsl-on-btn);padding:10px 14px;font-size:13px;transition:top .2s}
.tsl-skip:focus{top:12px}
.tsl-sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
.tsl-frame{position:relative;max-width:1240px;margin:0 auto;border-left:1px solid var(--tsl-line);border-right:1px solid var(--tsl-line)}
.tsl-sec{position:relative;padding:112px var(--tsl-gut)}
.tsl-sec+.tsl-sec{border-top:1px solid var(--tsl-line)}
.tsl-cross{position:absolute;top:-6px;width:11px;height:11px;pointer-events:none;z-index:2}
.tsl-cross::before,.tsl-cross::after{content:"";position:absolute;background:var(--tsl-line2)}
.tsl-cross::before{left:5px;top:0;width:1px;height:11px}
.tsl-cross::after{top:5px;left:0;height:1px;width:11px}
.tsl-cross[data-side="l"]{left:-6px}
.tsl-cross[data-side="r"]{right:-6px}

.tsl-kicker{display:inline-flex;align-items:center;gap:8px;font-family:var(--tsl-mono);font-size:10.5px;font-weight:500;letter-spacing:.12em;text-transform:uppercase;color:var(--tsl-ink)}
.tsl-kicker svg{flex:none}
.tsl-night .tsl-kicker{color:#fff}
.tsl-h1{font-family:var(--tsl-serif);font-weight:400;font-size:clamp(36px,4.7vw,58px);line-height:1.07;letter-spacing:-.022em}
.tsl-h2{font-family:var(--tsl-serif);font-weight:400;font-size:clamp(28px,3.3vw,42px);line-height:1.14;letter-spacing:-.018em}
.tsl-h3{font-family:var(--tsl-serif);font-weight:400;font-size:clamp(20px,1.7vw,23px);line-height:1.25;letter-spacing:-.01em}
.tsl-line{display:block;overflow:hidden;padding-bottom:.08em;margin-bottom:-.08em}
.tsl-line>span{display:inline-block}
.tsl-muted{color:var(--tsl-muted)}
.tsl-lead{font-size:15px;color:var(--tsl-soft);max-width:440px}

.tsl-btn{position:relative;display:inline-flex;align-items:center;justify-content:center;gap:10px;height:40px;padding:0 18px;border-radius:2px;font-size:11.5px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;white-space:nowrap;isolation:isolate;overflow:hidden;transition:color .25s var(--tsl-ease),border-color .25s,background .25s}
.tsl-btn::before{content:"";position:absolute;inset:0;z-index:-1;background:var(--tsl-accent);clip-path:inset(0 100% 0 0);transition:clip-path .45s steps(6,end)}
.tsl-btn:hover::before{clip-path:inset(0 0 0 0)}
.tsl-btn svg{transition:transform .35s var(--tsl-ease)}
.tsl-btn:hover svg{transform:translateX(3px)}
.tsl-btn-solid{background:var(--tsl-btn);color:var(--tsl-on-btn)}
.tsl-btn-solid:hover{color:var(--tsl-on-accent)}
.tsl-btn-line{border:1px solid var(--tsl-line2);background:var(--tsl-card);color:var(--tsl-ink)}
.tsl-btn-line::before{background:var(--tsl-panel)}
.tsl-btn-line:hover{border-color:var(--tsl-ink)}
.tsl-btn-block{width:100%}
.tsl-link{display:inline-flex;align-items:center;gap:6px;font-size:13px;font-weight:500;border-bottom:1px solid transparent;transition:border-color .2s,gap .2s}
.tsl-link:hover{border-color:currentColor;gap:9px}

.tsl-announce{position:relative;z-index:31;background:#000;color:rgba(255,255,255,.72);font-size:12px;text-align:center}
.tsl-announce a{display:inline-flex;align-items:center;gap:8px;padding:8px 40px;transition:color .2s}
.tsl-announce a:hover{color:#fff}
.tsl-announce b{color:#fff;font-weight:500}
.tsl-announce-x{position:absolute;right:10px;top:50%;transform:translateY(-50%);width:26px;height:26px;display:grid;place-items:center;color:rgba(255,255,255,.55);border-radius:2px}
.tsl-announce-x:hover{color:#fff;background:rgba(255,255,255,.08)}

.tsl-nav{position:sticky;top:0;z-index:30;background:color-mix(in srgb,var(--tsl-paper) 96%,transparent);backdrop-filter:saturate(1.4) blur(10px);-webkit-backdrop-filter:saturate(1.4) blur(10px);border-bottom:1px solid transparent;transition:border-color .3s,background .3s}
.tsl-nav[data-scrolled]{border-bottom-color:var(--tsl-line)}
.tsl-nav-in{max-width:1240px;margin:0 auto;height:var(--tsl-nav);display:flex;align-items:center;gap:28px;padding:0 24px;border-left:1px solid var(--tsl-line);border-right:1px solid var(--tsl-line)}
.tsl-brand{display:inline-flex;align-items:center;gap:10px;font-family:var(--tsl-serif);font-size:20px;letter-spacing:-.02em}
.tsl-mark rect{transition:opacity .3s,transform .4s var(--tsl-ease);transform-box:fill-box;transform-origin:center}
.tsl-brand:hover .tsl-mark rect:nth-child(odd){transform:scale(.55);opacity:.5}
.tsl-brand:hover .tsl-mark rect:nth-child(5){transform:rotate(45deg) scale(.9)}
.tsl-menu{display:flex;align-items:center;gap:4px}
.tsl-menu>li{position:relative}
.tsl-menu-a{display:inline-flex;align-items:center;gap:5px;height:34px;padding:0 10px;font-size:13px;color:var(--tsl-soft);border-radius:2px;transition:color .2s,background .2s}
.tsl-menu-a:hover,.tsl-menu-a[aria-expanded="true"]{color:var(--tsl-ink);background:var(--tsl-panel)}
.tsl-menu-a svg{transition:transform .3s var(--tsl-ease)}
.tsl-menu-a[aria-expanded="true"] svg{transform:rotate(180deg)}
.tsl-drop{position:absolute;left:-8px;top:calc(100% + 10px);width:300px;padding:8px;background:var(--tsl-card);border:1px solid var(--tsl-line2);box-shadow:var(--tsl-shadow);border-radius:4px;opacity:0;transform:translateY(-6px);pointer-events:none;transition:opacity .2s,transform .3s var(--tsl-ease)}
.tsl-drop[data-open]{opacity:1;transform:none;pointer-events:auto}
.tsl-drop a{display:grid;grid-template-columns:28px 1fr;gap:2px 10px;padding:10px;border-radius:3px;transition:background .2s}
.tsl-drop a:hover,.tsl-drop a:focus-visible{background:var(--tsl-panel)}
.tsl-drop i{grid-row:span 2;width:28px;height:28px;display:grid;grid-template-columns:repeat(3,1fr);gap:2px;padding:5px;border:1px solid var(--tsl-line2);border-radius:2px}
.tsl-drop i span{background:var(--tsl-accent);opacity:.25}
.tsl-drop i span[data-on]{opacity:1}
.tsl-drop b{font-weight:550;font-size:13px}
.tsl-drop small{font-size:12px;color:var(--tsl-muted);line-height:1.4}
.tsl-nav-right{margin-left:auto;display:flex;align-items:center;gap:10px}
.tsl-signin{font-size:11.5px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;padding:0 10px;height:34px;display:inline-flex;align-items:center;border-radius:2px}
.tsl-signin:hover{background:var(--tsl-panel)}
.tsl-icon-btn{width:34px;height:34px;display:inline-grid;place-items:center;border-radius:2px;color:var(--tsl-soft);transition:background .2s,color .2s}
.tsl-icon-btn:hover{background:var(--tsl-panel);color:var(--tsl-ink)}
.tsl-burger{display:none}
.tsl-progress{position:absolute;left:0;right:0;bottom:-1px;height:2px;display:flex;gap:2px;pointer-events:none}
.tsl-progress i{flex:1;background:var(--tsl-accent);transform:scaleY(0);transform-origin:bottom;transition:transform .25s var(--tsl-ease)}
.tsl-progress i[data-on]{transform:scaleY(1)}
.tsl-sheet{position:fixed;inset:0;z-index:60;background:var(--tsl-paper);padding:16px 20px 28px;display:flex;flex-direction:column;overflow:auto;animation:tsl-fade .25s var(--tsl-ease)}
.tsl-sheet-top{display:flex;align-items:center;justify-content:space-between;height:44px;margin-bottom:18px}
.tsl-sheet-list>li{border-bottom:1px solid var(--tsl-line)}
.tsl-sheet-list a{display:flex;justify-content:space-between;align-items:center;padding:16px 2px;font-family:var(--tsl-serif);font-size:24px}
.tsl-sheet-sub{padding:0 0 14px;display:grid;gap:4px}
.tsl-sheet-sub a{font-family:var(--tsl-sans);font-size:14px;padding:6px 2px;color:var(--tsl-soft)}
.tsl-sheet-cta{margin-top:auto;display:grid;gap:10px;padding-top:24px}

.tsl-hero{padding:72px var(--tsl-gut) 0}
.tsl-badge{display:inline-flex;align-items:center;gap:8px;font-size:12px;color:var(--tsl-soft)}
.tsl-badge i{width:18px;height:18px;display:grid;place-items:center;background:#f26522;color:#fff;font-style:normal;font-size:11px;font-weight:600;border-radius:2px}
.tsl-hero .tsl-h1{margin-top:22px;max-width:760px}
.tsl-hero-text{margin-top:20px}
.tsl-hero-actions{margin-top:30px;display:flex;flex-wrap:wrap;gap:10px}
.tsl-hero-panel{position:relative;margin-top:56px;min-height:clamp(420px,calc(var(--tsl-h) * .62),640px);display:grid;place-items:center;padding:56px 20px 64px;overflow:hidden;isolation:isolate;background:var(--tsl-c1)}
.tsl-mosaic{position:absolute;inset:0;z-index:-1;overflow:hidden}
.tsl-mosaic canvas{position:absolute;left:0;top:0}
.tsl-mosaic[data-interactive]{cursor:crosshair}
.tsl-hero-ruler{position:absolute;left:0;right:0;bottom:0;height:22px;display:flex;justify-content:space-between;align-items:flex-end;padding:0 14px 6px;font-family:var(--tsl-mono);font-size:9.5px;color:rgba(255,255,255,.8);letter-spacing:.08em;pointer-events:none}
.tsl-prompt-wrap{width:min(760px,100%);display:grid;gap:14px;justify-items:center}
.tsl-prompt{width:min(560px,100%);background:var(--tsl-card);border-radius:8px;padding:6px;box-shadow:0 0 0 6px rgba(255,255,255,.28),0 30px 60px -24px rgba(12,20,80,.55);text-align:left}
.tsl-prompt-label{display:flex;align-items:center;justify-content:space-between;padding:6px 10px 8px;font-size:11.5px;color:var(--tsl-muted)}
.tsl-prompt-label span{display:inline-flex;align-items:center;gap:7px}
.tsl-prompt-label i{width:6px;height:6px;border-radius:1px;background:var(--tsl-accent)}
.tsl-prompt[data-phase="running"] .tsl-prompt-label i{animation:tsl-blink 1s steps(2,end) infinite}
.tsl-prompt-box{border:1px solid var(--tsl-line2);border-radius:6px;background:var(--tsl-card);transition:border-color .2s,box-shadow .2s}
.tsl-prompt-box:focus-within{border-color:var(--tsl-accent);box-shadow:0 0 0 3px color-mix(in srgb,var(--tsl-accent) 16%,transparent)}
.tsl-prompt-area{position:relative}
.tsl-prompt textarea:focus-visible{outline:0}
.tsl-prompt textarea{display:block;width:100%;min-height:76px;max-height:160px;resize:none;border:0;outline:0;background:transparent;padding:14px 14px 4px;font-size:14px;line-height:1.5}
.tsl-ph{position:absolute;left:14px;top:14px;right:14px;font-size:14px;color:var(--tsl-faint);pointer-events:none;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.tsl-ph::after{content:"";display:inline-block;width:1px;height:15px;margin-left:1px;vertical-align:-2px;background:currentColor;animation:tsl-blink 1s steps(2,end) infinite}
.tsl-prompt-bar{display:flex;align-items:center;justify-content:space-between;padding:8px 10px 10px}
.tsl-model{position:relative}
.tsl-model-btn{display:inline-flex;align-items:center;gap:6px;height:24px;padding:0 8px;font-size:11px;color:var(--tsl-soft);background:var(--tsl-panel);border:1px solid var(--tsl-line);border-radius:3px}
.tsl-model-btn:hover{border-color:var(--tsl-line2)}
.tsl-model-list{position:absolute;left:0;bottom:calc(100% + 6px);min-width:200px;padding:4px;background:var(--tsl-card);border:1px solid var(--tsl-line2);border-radius:4px;box-shadow:var(--tsl-shadow);z-index:5;animation:tsl-pop .2s var(--tsl-ease)}
.tsl-model-list li{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:8px 10px;font-size:12px;border-radius:3px;cursor:pointer}
.tsl-model-list li small{color:var(--tsl-muted);font-size:11px}
.tsl-model-list li[data-active]{background:var(--tsl-panel)}
.tsl-model-list li[aria-selected="true"] b::after{content:"";display:inline-block;width:5px;height:5px;margin-left:7px;vertical-align:2px;background:var(--tsl-accent)}
.tsl-model-list b{font-weight:550}
.tsl-send{width:28px;height:28px;display:grid;place-items:center;background:var(--tsl-btn);color:var(--tsl-on-btn);border-radius:3px;transition:transform .2s var(--tsl-ease),opacity .2s}
.tsl-send:hover:not(:disabled){transform:translateY(-1px)}
.tsl-send:disabled{opacity:.35;cursor:not-allowed}
.tsl-run{display:grid;grid-template-rows:0fr;transition:grid-template-rows .55s var(--tsl-ease)}
.tsl-run[data-open]{grid-template-rows:1fr}
.tsl-run>div{min-height:0;overflow:hidden}
.tsl-run-in{padding:8px 6px 4px}
.tsl-run-head{display:flex;align-items:center;justify-content:space-between;padding:8px 6px 10px;font-size:11.5px;color:var(--tsl-muted)}
.tsl-run-head b{font-weight:550;color:var(--tsl-ink)}
.tsl-run-head span{font-family:var(--tsl-mono);font-size:10.5px}
.tsl-steps{display:grid;gap:2px}
.tsl-step{display:grid;grid-template-columns:18px auto 1fr auto;align-items:center;gap:10px;padding:7px 6px;font-size:12.5px;border-radius:3px;opacity:.35;transition:opacity .3s,background .3s}
.tsl-step[data-state="active"],.tsl-step[data-state="done"]{opacity:1}
.tsl-step[data-state="active"]{background:var(--tsl-panel)}
.tsl-step code{font-family:var(--tsl-mono);font-size:10.5px;padding:2px 6px;border-radius:2px;background:color-mix(in srgb,var(--tsl-accent) 10%,transparent);color:var(--tsl-accent)}
.tsl-root[data-theme="dark"] .tsl-step code{color:color-mix(in srgb,var(--tsl-accent) 45%,#fff)}
.tsl-step em{font-style:normal;color:var(--tsl-soft);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.tsl-step time{font-family:var(--tsl-mono);font-size:10.5px;color:var(--tsl-faint)}
.tsl-spin{width:14px;height:14px;display:grid;grid-template-columns:1fr 1fr;gap:2px}
.tsl-spin span{background:var(--tsl-accent);animation:tsl-spin 1s steps(1,end) infinite}
.tsl-spin span:nth-child(2){animation-delay:.25s}
.tsl-spin span:nth-child(4){animation-delay:.5s}
.tsl-spin span:nth-child(3){animation-delay:.75s}
.tsl-dot{width:6px;height:6px;margin:4px;border-radius:1px;background:var(--tsl-line2)}
.tsl-check{color:var(--tsl-accent)}
.tsl-answer{margin:10px 6px 4px;padding:12px 14px;border-left:2px solid var(--tsl-accent);background:var(--tsl-panel);font-size:13px;line-height:1.6;color:var(--tsl-soft);min-height:48px}
.tsl-answer[data-error]{border-color:#e5484d;color:#e5484d}
.tsl-caret{display:inline-block;width:7px;height:13px;margin-left:2px;vertical-align:-2px;background:var(--tsl-accent);animation:tsl-blink 1s steps(2,end) infinite}
.tsl-run-foot{display:flex;justify-content:flex-end;gap:6px;padding:8px 6px 6px}
.tsl-mini{display:inline-flex;align-items:center;gap:6px;height:26px;padding:0 10px;font-size:11.5px;border:1px solid var(--tsl-line2);border-radius:3px;background:var(--tsl-card);transition:border-color .2s}
.tsl-mini:hover{border-color:var(--tsl-ink)}
.tsl-chips{display:flex;flex-wrap:wrap;justify-content:center;gap:8px}
.tsl-chip{height:28px;padding:0 12px;font-size:12px;color:#0a0a0b;background:rgba(255,255,255,.86);border:1px solid rgba(255,255,255,.95);border-radius:2px;transition:background .2s,transform .25s var(--tsl-ease)}
.tsl-chip:hover{background:#fff;transform:translateY(-1px)}
.tsl-chip:disabled{opacity:.5;cursor:default;transform:none}

.tsl-logos{padding:40px 0;overflow:hidden;position:relative}
.tsl-logos::before,.tsl-logos::after{content:"";position:absolute;top:0;bottom:0;width:120px;z-index:1;pointer-events:none}
.tsl-logos::before{left:0;background:linear-gradient(90deg,var(--tsl-paper),transparent)}
.tsl-logos::after{right:0;background:linear-gradient(270deg,var(--tsl-paper),transparent)}
.tsl-logos-track{display:flex;width:max-content;animation:tsl-marquee 38s linear infinite}
.tsl-logos:hover .tsl-logos-track{animation-play-state:paused}
.tsl-logo{display:inline-flex;align-items:center;gap:8px;padding:0 44px;font-size:19px;color:var(--tsl-ink);opacity:.82;transition:opacity .25s}
.tsl-logo:hover{opacity:1}
.tsl-logo[data-style="bold"]{font-weight:700;letter-spacing:-.03em}
.tsl-logo[data-style="italic"]{font-style:italic;font-weight:650;letter-spacing:-.02em}
.tsl-logo[data-style="light"]{font-weight:500;letter-spacing:-.01em}
.tsl-logo[data-style="caps"]{font-weight:600;font-size:15px;letter-spacing:.18em;text-transform:uppercase}

.tsl-night{background:var(--tsl-night);color:#fff;padding-top:120px;padding-bottom:120px}
.tsl-sec.tsl-night+.tsl-sec{border-top-color:transparent}
.tsl-night .tsl-cross::before,.tsl-night .tsl-cross::after{background:rgba(255,255,255,.22)}
.tsl-center{text-align:center;display:grid;justify-items:center}
.tsl-center .tsl-h2{margin-top:22px;max-width:820px}
.tsl-w{display:inline}
.tsl-night .tsl-w[data-muted]{color:rgba(255,255,255,.42)}
.tsl-w[data-muted]{color:var(--tsl-muted)}
.tsl-pcards{margin-top:64px;display:grid;grid-template-columns:repeat(3,1fr);border:1px solid var(--tsl-night-line)}
.tsl-pcard{position:relative;display:flex;flex-direction:column;justify-content:space-between;min-height:300px;padding:28px;text-align:left;color:#fff;overflow:hidden;cursor:pointer;transition:background .4s var(--tsl-ease)}
.tsl-pcard+.tsl-pcard{border-left:1px solid var(--tsl-night-line)}
.tsl-pcard:hover,.tsl-pcard[data-open]{background:#07080c}
.tsl-pcard-icon{width:120px;height:120px;margin:-8px 0 0 -10px}
.tsl-pcard-icon g{transition:transform .7s var(--tsl-ease)}
.tsl-pcard h3{font-family:var(--tsl-serif);font-weight:400;font-size:19px;letter-spacing:-.01em}
.tsl-pcard h3 button{text-align:left;font:inherit;color:inherit}
.tsl-pcard h3 button::after{content:"";position:absolute;inset:0;z-index:1}
.tsl-pcard h3 button:focus-visible{outline:0}
.tsl-pcard:has(:focus-visible){outline:2px solid var(--tsl-accent);outline-offset:-2px}
.tsl-pcard-body{display:grid;grid-template-rows:0fr;transition:grid-template-rows .55s var(--tsl-ease)}
.tsl-pcard-body>div{overflow:hidden;min-height:0}
.tsl-pcard-body p{padding-top:10px;font-size:13px;line-height:1.6;color:rgba(255,255,255,.62);max-width:300px}
.tsl-pcard:hover .tsl-pcard-body,.tsl-pcard[data-open] .tsl-pcard-body,.tsl-pcard:focus-within .tsl-pcard-body{grid-template-rows:1fr}
.tsl-pcard-px{position:absolute;right:16px;top:16px;display:grid;grid-template-columns:repeat(3,6px);gap:3px}
.tsl-pcard-px span{width:6px;height:6px;background:var(--tsl-accent);opacity:0;transform:scale(.3);transition:opacity .3s,transform .4s var(--tsl-ease)}
.tsl-pcard:hover .tsl-pcard-px span,.tsl-pcard[data-open] .tsl-pcard-px span{opacity:1;transform:none}
.tsl-pcard-num{position:absolute;right:20px;bottom:22px;font-family:var(--tsl-mono);font-size:10.5px;color:rgba(255,255,255,.35)}
.tsl-pcard:hover .lift1,.tsl-pcard[data-open] .lift1{transform:translateY(-8px)}
.tsl-pcard:hover .lift2,.tsl-pcard[data-open] .lift2{transform:translateY(-16px)}
.tsl-pcard:hover .lift3,.tsl-pcard[data-open] .lift3{transform:translateY(-24px)}
.tsl-pcard:hover .liftlid,.tsl-pcard[data-open] .liftlid{transform:translateY(-30px);opacity:.38}
.tsl-pcard .liftlid{transition:transform .7s var(--tsl-ease),opacity .5s}
.tsl-pcard:hover .rise,.tsl-pcard[data-open] .rise{transform:translateY(-10px)}
.tsl-pcard .rise:nth-of-type(2){transition-delay:.06s}
.tsl-pcard .rise:nth-of-type(3){transition-delay:.12s}
.tsl-pcard .rise:nth-of-type(4){transition-delay:.18s}
.tsl-pcard .rise:nth-of-type(5){transition-delay:.24s}

.tsl-feat{margin-top:88px;display:grid;grid-template-columns:minmax(0,.9fr) minmax(0,1.1fr);gap:64px;align-items:start}
.tsl-feat-steps{position:relative}
.tsl-feat-rail{position:absolute;left:0;top:0;bottom:0;width:1px;background:var(--tsl-line)}
.tsl-feat-rail i{position:absolute;left:-1px;top:0;width:3px;background:var(--tsl-accent);transition:transform .6s var(--tsl-ease),height .6s var(--tsl-ease)}
.tsl-step-item{min-height:clamp(380px,62vh,560px);display:flex;flex-direction:column;justify-content:center;padding-left:32px;opacity:.32;transition:opacity .5s var(--tsl-ease)}
.tsl-step-item[data-active]{opacity:1}
.tsl-step-item .tsl-h3{margin-top:16px;font-size:clamp(22px,2vw,28px)}
.tsl-step-item p{margin-top:14px;color:var(--tsl-soft);max-width:420px}
.tsl-points{margin-top:20px;display:grid;gap:9px}
.tsl-points li{display:flex;align-items:center;gap:10px;font-size:13.5px}
.tsl-points li::before{content:"";width:6px;height:6px;flex:none;background:var(--tsl-accent)}
.tsl-stage{position:sticky;top:calc(var(--tsl-nav) + 40px);height:clamp(460px,72vh,620px);display:grid;place-items:center;overflow:hidden;isolation:isolate;padding:28px;background:var(--tsl-c1)}
.tsl-stage-tag{position:absolute;left:14px;top:12px;font-family:var(--tsl-mono);font-size:10px;letter-spacing:.1em;color:rgba(255,255,255,.85);text-transform:uppercase}
.tsl-stage-card{width:min(420px,100%);background:var(--tsl-card);border-radius:8px;box-shadow:0 0 0 6px rgba(255,255,255,.25),0 30px 60px -24px rgba(12,20,80,.55);overflow:hidden}
.tsl-stage-card>div{padding:16px}
.tsl-feat-tabs{display:none}
.tsl-feat-mobile{display:none}
.tsl-card-head{display:flex;align-items:center;justify-content:space-between;padding-bottom:12px;margin-bottom:12px;border-bottom:1px solid var(--tsl-line)}
.tsl-card-head b{font-family:var(--tsl-mono);font-size:10.5px;letter-spacing:.1em;text-transform:uppercase;font-weight:500}
.tsl-pill{display:inline-flex;align-items:center;gap:6px;height:20px;padding:0 8px;font-size:10.5px;border-radius:10px;background:var(--tsl-panel);color:var(--tsl-soft)}
.tsl-pill i{width:6px;height:6px;border-radius:50%;background:#30a46c}
.tsl-field{margin-bottom:12px}
.tsl-field label,.tsl-field>span{display:block;font-size:11px;color:var(--tsl-muted);margin-bottom:6px}
.tsl-field-box{border:1px solid var(--tsl-line2);border-radius:5px;padding:10px 12px;font-size:12.5px;line-height:1.55;min-height:62px;background:var(--tsl-panel)}
.tsl-toggles{display:grid;gap:2px}
.tsl-toggle{display:flex;align-items:center;justify-content:space-between;gap:10px;padding:7px 2px;font-size:12.5px;width:100%;text-align:left}
.tsl-toggle small{color:var(--tsl-muted);font-size:11px;margin-left:6px}
.tsl-switch{position:relative;width:28px;height:16px;flex:none;border-radius:9px;background:var(--tsl-line2);transition:background .25s}
.tsl-switch::after{content:"";position:absolute;left:2px;top:2px;width:12px;height:12px;border-radius:50%;background:#fff;box-shadow:0 1px 2px rgba(0,0,0,.25);transition:transform .3s var(--tsl-ease)}
.tsl-toggle[aria-checked="true"] .tsl-switch{background:var(--tsl-accent)}
.tsl-toggle[aria-checked="true"] .tsl-switch::after{transform:translateX(12px)}
.tsl-card-foot{display:flex;align-items:center;justify-content:space-between;gap:10px;margin-top:12px;padding-top:12px;border-top:1px solid var(--tsl-line);font-size:11.5px;color:var(--tsl-muted)}
.tsl-graph{width:100%;height:auto}
.tsl-graph .edge{fill:none;stroke:var(--tsl-line2);stroke-width:1.2}
.tsl-graph .edge[data-on]{stroke:var(--tsl-accent);stroke-dasharray:4 4;animation:tsl-dash 1.2s linear infinite}
.tsl-graph .node{cursor:pointer}
.tsl-graph .node rect{fill:var(--tsl-card);stroke:var(--tsl-line2);transition:stroke .2s,fill .2s}
.tsl-graph .node[data-on] rect{stroke:var(--tsl-accent)}
.tsl-graph .node:hover rect,.tsl-graph .node:focus-visible rect{fill:var(--tsl-panel)}
.tsl-graph .node text{font-family:var(--tsl-mono);font-size:9.5px;fill:var(--tsl-soft)}
.tsl-graph .node[data-on] text{fill:var(--tsl-ink)}
.tsl-graph .core rect{fill:var(--tsl-btn)}
.tsl-graph .core text{fill:var(--tsl-on-btn);font-family:var(--tsl-sans);font-size:11px;font-weight:600}
.tsl-graph .node:focus{outline:none}
.tsl-wf{display:grid;gap:6px}
.tsl-wf-row{display:grid;grid-template-columns:98px 1fr;align-items:center;gap:10px;font-family:var(--tsl-mono);font-size:10.5px;color:var(--tsl-soft);padding:3px 4px;border-radius:3px;cursor:default;transition:background .2s}
.tsl-wf-row:hover,.tsl-wf-row:focus-visible{background:var(--tsl-panel)}
.tsl-wf-track{position:relative;height:12px}
.tsl-wf-bar{position:absolute;top:0;height:12px;border-radius:2px;background:var(--tsl-accent);transform-origin:left;transform:scaleX(0);transition:transform .7s var(--tsl-ease)}
.tsl-wf[data-play] .tsl-wf-bar{transform:scaleX(1)}
.tsl-wf-bar[data-kind="llm"]{background:color-mix(in srgb,var(--tsl-accent) 55%,#fff)}
.tsl-wf-bar[data-kind="tool"]{background:var(--tsl-ink)}
.tsl-wf-axis{display:flex;justify-content:space-between;font-family:var(--tsl-mono);font-size:9.5px;color:var(--tsl-faint);padding:4px 4px 0 112px}
.tsl-metrics{display:grid;grid-template-columns:1fr 1fr;gap:10px}
.tsl-metric{border:1px solid var(--tsl-line);border-radius:5px;padding:10px 12px}
.tsl-metric small{display:block;font-size:10.5px;color:var(--tsl-muted)}
.tsl-metric b{display:block;margin-top:4px;font-family:var(--tsl-serif);font-weight:400;font-size:24px;letter-spacing:-.02em;font-variant-numeric:tabular-nums}
.tsl-seg{display:inline-flex;padding:2px;border-radius:4px;background:var(--tsl-panel);border:1px solid var(--tsl-line)}
.tsl-seg button{height:24px;padding:0 10px;font-size:11.5px;border-radius:3px;color:var(--tsl-muted);transition:background .2s,color .2s}
.tsl-seg button[aria-pressed="true"],.tsl-seg button[aria-checked="true"]{background:var(--tsl-card);color:var(--tsl-ink);box-shadow:0 1px 2px rgba(0,0,0,.08)}
.tsl-range{width:100%;accent-color:var(--tsl-accent)}
.tsl-range-row{display:flex;justify-content:space-between;align-items:center;font-size:11.5px;color:var(--tsl-muted);margin:12px 0 4px}
.tsl-range-row b{color:var(--tsl-ink);font-weight:550;font-variant-numeric:tabular-nums}

.tsl-stats{display:grid;grid-template-columns:repeat(4,1fr);padding:0}
.tsl-stat{padding:48px 32px;display:flex;flex-direction:column;gap:14px}
.tsl-stat+.tsl-stat{border-left:1px solid var(--tsl-line)}
.tsl-stat b{font-family:var(--tsl-serif);font-weight:400;font-size:clamp(38px,4vw,54px);line-height:1;letter-spacing:-.03em;font-variant-numeric:tabular-nums}
.tsl-stat p{font-size:13px;color:var(--tsl-muted);max-width:220px}
.tsl-stat-px{display:flex;gap:3px}
.tsl-stat-px span{width:8px;height:8px;background:var(--tsl-line2);transition:background .3s}
.tsl-stat-px span[data-on]{background:var(--tsl-accent)}

.tsl-case{margin-top:48px;display:grid;grid-template-columns:minmax(0,.8fr) minmax(0,1.2fr);border:1px solid var(--tsl-line)}
.tsl-portrait{position:relative;aspect-ratio:5/6;overflow:hidden;background:var(--tsl-c1)}
.tsl-portrait img{position:absolute;inset:0;width:100%;height:100%;object-fit:cover;object-position:50% 30%;max-width:none}
.tsl-portrait canvas{position:absolute;left:0;top:0}
.tsl-portrait::after{content:"";position:absolute;inset:auto 0 0 0;height:40%;background:linear-gradient(0deg,color-mix(in srgb,var(--tsl-c0) 55%,transparent),transparent);pointer-events:none}
.tsl-portrait-tag{position:absolute;left:14px;bottom:14px;z-index:1;font-family:var(--tsl-mono);font-size:10px;letter-spacing:.1em;color:#fff;text-transform:uppercase}
.tsl-case-body{padding:44px;display:flex;flex-direction:column;justify-content:space-between;gap:36px}
.tsl-case-co{font-family:var(--tsl-mono);font-size:11px;letter-spacing:.12em;text-transform:uppercase;color:var(--tsl-muted)}
.tsl-case blockquote{font-family:var(--tsl-serif);font-size:clamp(21px,2.1vw,28px);line-height:1.35;letter-spacing:-.012em}
.tsl-case-who b{display:block;font-weight:550;font-size:14px}
.tsl-case-who span{font-size:13px;color:var(--tsl-muted)}
.tsl-case-metrics{display:grid;grid-template-columns:repeat(3,1fr);border-top:1px solid var(--tsl-line);padding-top:24px;gap:16px}
.tsl-case-metrics b{display:block;font-family:var(--tsl-serif);font-weight:400;font-size:30px;letter-spacing:-.02em}
.tsl-case-metrics span{font-size:12px;color:var(--tsl-muted)}
.tsl-quotes{display:grid;grid-template-columns:repeat(3,1fr);border:1px solid var(--tsl-line);border-top:0}
.tsl-quote{padding:28px;display:flex;flex-direction:column;justify-content:space-between;gap:24px;transition:background .3s}
.tsl-quote:hover{background:var(--tsl-panel)}
.tsl-quote+.tsl-quote{border-left:1px solid var(--tsl-line)}
.tsl-quote p{font-size:14.5px;line-height:1.6;color:var(--tsl-soft)}
.tsl-quote footer{display:flex;align-items:center;gap:12px}
.tsl-quote img,.tsl-quote .tsl-av{width:36px;height:36px;border-radius:2px;object-fit:cover;flex:none;filter:grayscale(1) contrast(1.05);transition:filter .4s}
.tsl-quote:hover img{filter:none}
.tsl-av{display:grid;place-items:center;background:var(--tsl-accent);color:var(--tsl-on-accent);font-size:12px;font-weight:600}
.tsl-quote b{display:block;font-size:13px;font-weight:550}
.tsl-quote span{font-size:12px;color:var(--tsl-muted)}

.tsl-price-top{display:flex;align-items:flex-end;justify-content:space-between;gap:24px;flex-wrap:wrap}
.tsl-price-top .tsl-h2{margin-top:18px;max-width:560px}
.tsl-plans{margin-top:48px;display:grid;grid-template-columns:repeat(3,1fr);border:1px solid var(--tsl-line)}
.tsl-plan{position:relative;display:flex;flex-direction:column;padding:28px;gap:20px}
.tsl-plan+.tsl-plan{border-left:1px solid var(--tsl-line)}
.tsl-plan[data-featured]{background:var(--tsl-panel)}
.tsl-plan-band{position:relative;height:84px;margin:-28px -28px 0;overflow:hidden;isolation:isolate;background:var(--tsl-c1);display:flex;align-items:flex-end;padding:10px 14px}
.tsl-plan-band span{font-family:var(--tsl-mono);font-size:10px;letter-spacing:.1em;text-transform:uppercase;color:#fff;background:rgba(0,0,0,.35);padding:3px 7px;border-radius:2px}
.tsl-plan h3{font-family:var(--tsl-mono);font-size:11px;letter-spacing:.12em;text-transform:uppercase;font-weight:500}
.tsl-plan-price{display:flex;align-items:baseline;gap:8px;min-height:52px}
.tsl-plan-price b{font-family:var(--tsl-serif);font-weight:400;font-size:46px;letter-spacing:-.03em;line-height:1;font-variant-numeric:tabular-nums}
.tsl-plan-price span{font-size:12px;color:var(--tsl-muted)}
.tsl-plan p{font-size:13.5px;color:var(--tsl-soft)}
.tsl-plan ul{display:grid;gap:10px;font-size:13.5px}
.tsl-plan li{display:flex;gap:10px;align-items:flex-start}
.tsl-plan li svg{flex:none;margin-top:3px;color:var(--tsl-accent)}
.tsl-plan .tsl-btn{margin-top:auto}
.tsl-est{border:1px dashed var(--tsl-line2);border-radius:4px;padding:12px 14px;background:var(--tsl-card)}
.tsl-est-row{display:flex;align-items:center;justify-content:space-between;font-size:12px;color:var(--tsl-muted)}
.tsl-est-total{margin-top:10px;display:flex;align-items:baseline;justify-content:space-between;font-size:12px;color:var(--tsl-muted)}
.tsl-est-total b{font-family:var(--tsl-serif);font-weight:400;font-size:22px;color:var(--tsl-ink);font-variant-numeric:tabular-nums}
.tsl-stepper{display:inline-flex;align-items:center;border:1px solid var(--tsl-line2);border-radius:3px}
.tsl-stepper button{width:24px;height:24px;display:grid;place-items:center}
.tsl-stepper button:hover{background:var(--tsl-panel)}
.tsl-stepper output{min-width:28px;text-align:center;font-size:12px;color:var(--tsl-ink);font-variant-numeric:tabular-nums}

.tsl-cta{padding:0}
.tsl-cta-panel{position:relative;min-height:clamp(420px,calc(var(--tsl-h) * .6),560px);display:grid;place-items:center;padding:64px 20px;overflow:hidden;isolation:isolate;background:var(--tsl-c1)}
.tsl-cta-card{width:min(560px,100%);background:var(--tsl-card);border-radius:8px;padding:36px;text-align:center;box-shadow:0 0 0 6px rgba(255,255,255,.28),0 30px 60px -24px rgba(12,20,80,.55)}
.tsl-cta-card .tsl-h2{font-size:clamp(26px,2.8vw,36px)}
.tsl-cta-card p{margin-top:12px;color:var(--tsl-soft);font-size:14px}
.tsl-form{margin-top:24px;display:flex;gap:8px}
.tsl-form input{flex:1;min-width:0;height:40px;padding:0 12px;border:1px solid var(--tsl-line2);border-radius:2px;background:var(--tsl-card);font-size:14px;outline:0;transition:border-color .2s,box-shadow .2s}
.tsl-form input:focus{border-color:var(--tsl-accent);box-shadow:0 0 0 3px color-mix(in srgb,var(--tsl-accent) 16%,transparent)}
.tsl-form input[aria-invalid="true"]{border-color:#e5484d}
.tsl-form-msg{margin-top:10px;margin-bottom:-8px;font-size:12.5px;min-height:19px;color:var(--tsl-muted)}
.tsl-form-msg[data-tone="error"]{color:#e5484d}
.tsl-form-msg[data-tone="ok"]{color:var(--tsl-accent)}

.tsl-foot{padding:72px var(--tsl-gut) 0}
.tsl-foot-top{display:grid;grid-template-columns:1.3fr repeat(4,1fr);gap:32px}
.tsl-foot-top p{margin-top:14px;font-size:13.5px;color:var(--tsl-muted);max-width:260px}
.tsl-foot h4{font-family:var(--tsl-mono);font-size:10.5px;letter-spacing:.12em;text-transform:uppercase;font-weight:500;color:var(--tsl-muted);margin-bottom:14px}
.tsl-foot li+li{margin-top:9px}
.tsl-foot li a{font-size:13.5px;color:var(--tsl-soft);transition:color .2s}
.tsl-foot li a:hover{color:var(--tsl-ink)}
.tsl-wordmark{position:relative;margin-top:64px;height:clamp(96px,17vw,240px);touch-action:pan-y}
.tsl-wordmark canvas{position:absolute;left:0;top:0}
.tsl-foot-bottom{display:flex;justify-content:space-between;align-items:center;gap:16px;flex-wrap:wrap;padding:20px 0 28px;border-top:1px solid var(--tsl-line);font-size:12px;color:var(--tsl-muted)}
.tsl-status{display:inline-flex;align-items:center;gap:8px}
.tsl-status i{width:7px;height:7px;border-radius:50%;background:#30a46c;box-shadow:0 0 0 3px color-mix(in srgb,#30a46c 22%,transparent)}

@keyframes tsl-blink{50%{opacity:0}}
@keyframes tsl-spin{0%{opacity:1}50%{opacity:.2}}
@keyframes tsl-marquee{to{transform:translateX(-50%)}}
@keyframes tsl-dash{to{stroke-dashoffset:-16}}
@keyframes tsl-fade{from{opacity:0}}
@keyframes tsl-pop{from{opacity:0;transform:translateY(4px)}}

.tsl-prompt-status{font-family:var(--tsl-mono);font-size:10.5px}
.tsl-prompt-status[data-tone="hit"]{color:#1f8a52}
.tsl-prompt-status[data-tone="warn"]{color:#b4540a}
.tsl-root[data-theme="dark"] .tsl-prompt-status[data-tone="hit"]{color:#5fd393}
.tsl-root[data-theme="dark"] .tsl-prompt-status[data-tone="warn"]{color:#ffb36b}
.tsl-step[data-tone="hit"] code,.tsl-step[data-tone="ok"] code{background:color-mix(in srgb,#30a46c 14%,transparent);color:#1f8a52}
.tsl-step[data-tone="warn"] code{background:color-mix(in srgb,#e5484d 12%,transparent);color:#c4272d}
.tsl-root[data-theme="dark"] .tsl-step[data-tone="hit"] code,.tsl-step[data-tone="ok"] code{color:#5fd393}
.tsl-root[data-theme="dark"] .tsl-step[data-tone="warn"] code{color:#ff8b8f}
.tsl-run-foot a.tsl-mini{margin-left:auto}
.tsl-cta-actions{margin-top:22px;display:flex;gap:10px;justify-content:center;flex-wrap:wrap}
.tsl-sec-lead{margin-top:18px;max-width:620px}
@media (max-width:640px){.tsl-cta-actions .tsl-btn{width:100%;justify-content:center}}
@media (max-width:1023px){
.tsl-root{--tsl-gut:28px}
.tsl-menu{display:none}
.tsl-burger{display:inline-grid}
.tsl-signin{display:none}
.tsl-feat{grid-template-columns:1fr;gap:28px;margin-top:56px}
.tsl-feat-steps{display:none}
.tsl-feat-tabs{display:flex;gap:6px;overflow-x:auto;scrollbar-width:none;padding-bottom:2px}
.tsl-feat-tabs button{flex:none;height:32px;padding:0 12px;font-family:var(--tsl-mono);font-size:10.5px;letter-spacing:.1em;text-transform:uppercase;border:1px solid var(--tsl-line2);border-radius:2px;color:var(--tsl-muted)}
.tsl-feat-tabs button[aria-selected="true"]{background:var(--tsl-btn);color:var(--tsl-on-btn);border-color:var(--tsl-btn)}
.tsl-feat-mobile{display:block}
.tsl-feat-mobile .tsl-h3{font-size:24px}
.tsl-feat-mobile p{margin-top:10px;color:var(--tsl-soft)}
.tsl-stage{position:relative;top:auto;height:auto;min-height:440px}
.tsl-stats{grid-template-columns:repeat(2,1fr)}
.tsl-stat:nth-child(3){border-left:0}
.tsl-stat:nth-child(n+3){border-top:1px solid var(--tsl-line)}
.tsl-case{grid-template-columns:1fr}
.tsl-portrait{aspect-ratio:16/10}
.tsl-quotes{grid-template-columns:1fr}
.tsl-quote+.tsl-quote{border-left:0;border-top:1px solid var(--tsl-line)}
.tsl-plans{grid-template-columns:1fr}
.tsl-plan+.tsl-plan{border-left:0;border-top:1px solid var(--tsl-line)}
.tsl-foot-top{grid-template-columns:repeat(2,1fr)}
.tsl-foot-top>div:first-child{grid-column:1/-1}
.tsl-pcards{grid-template-columns:1fr}
.tsl-pcard{min-height:220px}
.tsl-pcard+.tsl-pcard{border-left:0;border-top:1px solid var(--tsl-night-line)}
}
@media (max-width:640px){
.tsl-root{--tsl-gut:20px}
.tsl-sec{padding-top:80px;padding-bottom:80px}
.tsl-night{padding-top:88px;padding-bottom:88px}
.tsl-nav-in{padding:0 16px;gap:12px}
.tsl-nav-right .tsl-btn{display:none}
.tsl-hero{padding-top:48px}
.tsl-hero-actions .tsl-btn{flex:1}
.tsl-hero-panel{margin-top:40px;padding:40px 12px 52px;margin-left:calc(var(--tsl-gut) * -1);margin-right:calc(var(--tsl-gut) * -1)}
.tsl-chips .tsl-chip:nth-child(n+3){display:none}
.tsl-step{grid-template-columns:18px 1fr auto}
.tsl-step code{display:none}
.tsl-logo{padding:0 26px;font-size:17px}
.tsl-stats{grid-template-columns:1fr}
.tsl-stat{padding:32px 4px}
.tsl-stat+.tsl-stat{border-left:0;border-top:1px solid var(--tsl-line)}
.tsl-case-body{padding:28px 22px}
.tsl-case-metrics{grid-template-columns:1fr 1fr 1fr;gap:10px}
.tsl-case-metrics b{font-size:24px}
.tsl-stage{padding:16px 12px;margin-left:calc(var(--tsl-gut) * -1);margin-right:calc(var(--tsl-gut) * -1)}
.tsl-cta-card{padding:28px 20px}
.tsl-form{flex-direction:column}
.tsl-form input{flex:none}
.tsl-hero .tsl-line,.tsl-hero .tsl-line>span{display:inline;overflow:visible;padding:0;margin:0}
.tsl-foot-top{grid-template-columns:1fr 1fr}
.tsl-wf-row{grid-template-columns:80px 1fr}
.tsl-wf-axis{padding-left:94px}
}
@media (prefers-reduced-motion:reduce){
.tsl-root *,.tsl-root *::before,.tsl-root *::after{animation-duration:.001ms!important;animation-iteration-count:1!important;transition-duration:.001ms!important}
.tsl-logos-track{animation:none}
}
`

// ---------- hooks ----------

export type TslContextValue = { ramp: string[]; reduced: boolean; dark: boolean }

const TslContext = React.createContext({ ramp: [] as string[], reduced: false, dark: false } as TslContextValue)

/** The palette ramp, reduced-motion flag and theme of the surrounding shell. */
export function useTsl() {
  return React.useContext(TslContext)
}

export const useIsoLayoutEffect = typeof window !== "undefined" ? React.useLayoutEffect : React.useEffect

function useReducedMotion() {
  const [reduced, setReduced] = React.useState(false)
  React.useEffect(() => {
    if (typeof matchMedia !== "function") return
    const mq = matchMedia("(prefers-reduced-motion: reduce)")
    const on = () => setReduced(mq.matches)
    on()
    mq.addEventListener("change", on)
    return () => mq.removeEventListener("change", on)
  }, [])
  return reduced
}

function useTheme(pref: "system" | "light" | "dark") {
  const [dark, setDark] = React.useState(pref === "dark")
  React.useEffect(() => {
    if (pref !== "system") {
      setDark(pref === "dark")
      return
    }
    const mq = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null
    const read = () => {
      const el = document.documentElement
      if (el.classList.contains("dark")) return setDark(true)
      if (el.classList.contains("light")) return setDark(false)
      setDark(!!(mq && mq.matches))
    }
    read()
    const mo = new MutationObserver(read)
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] })
    if (mq) mq.addEventListener("change", read)
    return () => {
      mo.disconnect()
      if (mq) mq.removeEventListener("change", read)
    }
  }, [pref])
  return [dark, setDark] as const
}

/** Closes a popover on outside click or Escape. */
export function useDismiss(open: boolean, close: () => void, ref: { current: HTMLElement | null }) {
  React.useEffect(() => {
    if (!open) return
    const down = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) close()
    }
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") close()
    }
    document.addEventListener("pointerdown", down)
    document.addEventListener("keydown", key)
    return () => {
      document.removeEventListener("pointerdown", down)
      document.removeEventListener("keydown", key)
    }
  }, [open, close, ref])
}

/** Calls back once when the element first comes into view. */
export function useInView(ref: { current: Element | null }, margin = "0px 0px -15% 0px") {
  const [seen, setSeen] = React.useState(false)
  React.useEffect(() => {
    const el = ref.current
    if (!el || seen) return
    if (typeof IntersectionObserver !== "function") {
      setSeen(true)
      return
    }
    const io = new IntersectionObserver(
      (es) => {
        if (es.some((e) => e.isIntersecting)) {
          setSeen(true)
          io.disconnect()
        }
      },
      { rootMargin: margin }
    )
    io.observe(el)
    return () => io.disconnect()
  }, [ref, margin, seen])
  return seen
}

// ---------- small pieces ----------

const ICONS: { [k: string]: string } = {
  arrow: "M3 8h10M9 4l4 4-4 4",
  up: "M8 13V3M4 7l4-4 4 4",
  down: "M4 6l4 4 4-4",
  check: "M3 8.5l3 3 7-7",
  x: "M4 4l8 8M12 4l-8 8",
  menu: "M2 5h12M2 11h12",
  sun: "M8 2v1.5M8 12.5V14M2 8h1.5M12.5 8H14M3.8 3.8l1 1M11.2 11.2l1 1M3.8 12.2l1-1M11.2 4.8l1-1M8 5.5a2.5 2.5 0 100 5 2.5 2.5 0 000-5z",
  moon: "M13 9.5A5.5 5.5 0 016.5 3a5.5 5.5 0 106.5 6.5z",
  copy: "M5.5 5.5V3h7.5v7.5h-2.5M3 5.5h7.5V13H3z",
  plus: "M8 3v10M3 8h10",
  minus: "M3 8h10",
  replay: "M3 8a5 5 0 105-5H5M5 1v2.5H7.5",
}

export function Icon({ name, size = 14, stroke = 1.5 }: { name: string; size?: number; stroke?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={stroke} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={ICONS[name] || ""} />
    </svg>
  )
}

export function Mark({ size = 22 }: { size?: number }) {
  const cells = [0, 1, 2, 3, 4, 5, 6, 7, 8]
  return (
    <svg className="tsl-mark" width={size} height={size} viewBox="0 0 22 22" aria-hidden="true">
      {cells.map((i) => (
        <rect key={i} x={(i % 3) * 8} y={Math.floor(i / 3) * 8} width="6" height="6" fill={i === 4 ? "var(--tsl-accent)" : "currentColor"} />
      ))}
    </svg>
  )
}

export function KickerGlyph() {
  return (
    <svg width="18" height="8" viewBox="0 0 18 8" aria-hidden="true">
      <rect x="4" y="0" width="14" height="3" fill="var(--tsl-accent)" />
      <rect x="0" y="5" width="14" height="3" fill="var(--tsl-accent)" opacity=".55" />
    </svg>
  )
}

const GLYPHS = "{}~:Mb0xj1+&kZ/<>#%"

/** A label that decodes out of mono glyphs the first time it scrolls into view. */
export function Kicker({ text, reduced: forced }: { text: string; reduced?: boolean }) {
  const ctx = useTsl()
  const reduced = forced ?? ctx.reduced
  const ref = React.useRef(null as HTMLSpanElement | null)
  const seen = useInView(ref)
  const [out, setOut] = React.useState(text)
  React.useEffect(() => {
    if (!seen || reduced) {
      setOut(text)
      return
    }
    let raf = 0
    const t0 = performance.now()
    const dur = 650
    const tick = (now: number) => {
      const p = Math.min(1, (now - t0) / dur)
      let s = ""
      for (let i = 0; i < text.length; i++) {
        const ch = text[i]
        s += ch === " " || p > i / text.length + 0.15 ? ch : GLYPHS[(Math.random() * GLYPHS.length) | 0]
      }
      setOut(s)
      if (p < 1) raf = requestAnimationFrame(tick)
      else setOut(text)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [seen, reduced, text])
  return (
    <span className="tsl-kicker" ref={ref}>
      <KickerGlyph />
      <span className="tsl-sr">{text}</span>
      <span aria-hidden="true">{out}</span>
    </span>
  )
}

export function LogoMark({ mark }: { mark?: TslLogoMark }) {
  const d: { [k: string]: React.ReactNode } = {
    bolt: <path d="M11 1L3 11h5l-1 6 8-10h-5z" fill="currentColor" />,
    spark: <path d="M9 0l1.8 7.2L18 9l-7.2 1.8L9 18l-1.8-7.2L0 9l7.2-1.8z" fill="currentColor" />,
    chevron: <path d="M2 4l5 5-5 5M9 4l5 5-5 5" fill="none" stroke="currentColor" strokeWidth="2.4" />,
    orbit: (
      <g fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="9" cy="9" r="7" />
        <circle cx="15" cy="4" r="2.2" fill="currentColor" stroke="none" />
      </g>
    ),
    bars: (
      <g fill="currentColor">
        <rect x="1" y="7" width="3" height="10" />
        <rect x="6" y="3" width="3" height="14" />
        <rect x="11" y="9" width="3" height="8" />
        <rect x="16" y="5" width="2" height="12" />
      </g>
    ),
    knot: <path d="M4 9a5 5 0 0110 0 5 5 0 01-10 0zM9 4v10" fill="none" stroke="currentColor" strokeWidth="2.2" />,
    leaf: <path d="M2 16C2 7 8 2 16 2c0 8-5 14-14 14zM2 16l8-8" fill="none" stroke="currentColor" strokeWidth="2" />,
  }
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      {d[mark || "spark"]}
    </svg>
  )
}

// ---------- the mosaic ----------

export type TslMosaicState = { cell: number; shift: number }

/**
 * A canvas of quantized colour. The field is sampled once per cell and drawn
 * in colour buckets, so a frame is a few dozen fills. Near the pointer, cells
 * split into finer pixels and brighten, like a loupe. It runs only on screen
 * and never in a background tab; under reduced motion it draws still frames.
 */
export function Mosaic({
  scene,
  ramp,
  state,
  reduced,
  interactive = false,
  label,
}: {
  scene: TslScene
  ramp: string[]
  state: TslMosaicState
  reduced: boolean
  interactive?: boolean
  label?: string
}) {
  const wrapRef = React.useRef(null as HTMLDivElement | null)
  const canvasRef = React.useRef(null as HTMLCanvasElement | null)
  React.useEffect(() => {
    const wrap = wrapRef.current
    const canvas = canvasRef.current
    if (!wrap || !canvas) return
    const ctx = canvas.getContext("2d")
    if (!ctx) return
    let w = 0
    let h = 0
    let raf = 0
    let running = false
    let visible = false
    const ptr = { x: -9999, y: -9999, on: false, k: 0 }
    const t0 = performance.now()
    const n = ramp.length
    const buckets: number[][] = []
    for (let i = 0; i < n; i++) buckets.push([])

    const draw = (now: number) => {
      if (!w || !h) return
      const t = reduced ? 4 : (now - t0) / 1000
      const cell = Math.max(6, state.cell)
      const shift = state.shift
      ptr.k += ((ptr.on ? 1 : 0) - ptr.k) * (reduced ? 1 : 0.12)
      const R = 150 * ptr.k
      for (let i = 0; i < n; i++) buckets[i].length = 0
      const cols = Math.ceil(w / cell)
      const rows = Math.ceil(h / cell)
      const ox = (w - cols * cell) / 2
      const oy = (h - rows * cell) / 2
      for (let cy = 0; cy < rows; cy++) {
        for (let cx = 0; cx < cols; cx++) {
          const x0 = ox + cx * cell
          const y0 = oy + cy * cell
          const mx = x0 + cell / 2
          const my = y0 + cell / 2
          const dx = mx - ptr.x
          const dy = my - ptr.y
          const d = R > 1 ? Math.sqrt(dx * dx + dy * dy) : 1e9
          if (d < R) {
            // the loupe: finer pixels, a little more light
            const k = d < R * 0.45 ? 4 : 2
            const sub = cell / k
            const boost = 0.16 * (1 - d / R) * (1 - d / R)
            for (let sy = 0; sy < k; sy++) {
              for (let sx = 0; sx < k; sx++) {
                const px = x0 + sx * sub
                const py = y0 + sy * sub
                const v = fieldAt(scene, px + sub / 2, py + sub / 2, t, w, h, shift) + boost
                const b = Math.min(n - 1, Math.max(0, Math.round(v * (n - 1))))
                buckets[b].push(px, py, sub)
              }
            }
            continue
          }
          const grain = (hash2(cx, cy, 3) - 0.5) * 0.07
          const v = fieldAt(scene, mx, my, t, w, h, shift) + grain
          const b = Math.min(n - 1, Math.max(0, Math.round(v * (n - 1))))
          buckets[b].push(x0, y0, cell)
        }
      }
      for (let i = 0; i < n; i++) {
        const list = buckets[i]
        if (!list.length) continue
        ctx.fillStyle = ramp[i]
        ctx.beginPath()
        for (let j = 0; j < list.length; j += 3) ctx.rect(list[j], list[j + 1], list[j + 2] + 0.6, list[j + 2] + 0.6)
        ctx.fill()
      }
    }

    const loop = (now: number) => {
      draw(now)
      raf = requestAnimationFrame(loop)
    }
    const start = () => {
      if (running) return
      running = true
      raf = requestAnimationFrame(loop)
    }
    const stop = () => {
      running = false
      cancelAnimationFrame(raf)
    }
    const sync = () => {
      if (visible && !reduced && !document.hidden) start()
      else stop()
      if (reduced) draw(performance.now())
    }
    const resize = () => {
      const r = wrap.getBoundingClientRect()
      w = Math.max(1, Math.round(r.width))
      h = Math.max(1, Math.round(r.height))
      const dpr = Math.min(2, window.devicePixelRatio || 1)
      canvas.width = Math.round(w * dpr)
      canvas.height = Math.round(h * dpr)
      canvas.style.width = w + "px"
      canvas.style.height = h + "px"
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      draw(performance.now())
    }
    resize()
    const ro = new ResizeObserver(resize)
    ro.observe(wrap)
    const io = new IntersectionObserver((es) => {
      visible = es.some((e) => e.isIntersecting)
      sync()
    })
    io.observe(wrap)
    const vis = () => (document.hidden ? stop() : sync())
    document.addEventListener("visibilitychange", vis)

    // the panel's content sits above the canvas, so listen on the panel itself
    const host = interactive ? (wrap.parentElement as HTMLElement | null) : null
    const move = (e: PointerEvent) => {
      const r = wrap.getBoundingClientRect()
      ptr.x = e.clientX - r.left
      ptr.y = e.clientY - r.top
      ptr.on = e.pointerType === "mouse" || e.pointerType === "pen"
      if (reduced) draw(performance.now())
    }
    const leave = () => {
      ptr.on = false
      if (reduced) draw(performance.now())
    }
    if (host) {
      host.addEventListener("pointermove", move)
      host.addEventListener("pointerleave", leave)
    }
    return () => {
      stop()
      ro.disconnect()
      io.disconnect()
      document.removeEventListener("visibilitychange", vis)
      if (host) {
        host.removeEventListener("pointermove", move)
        host.removeEventListener("pointerleave", leave)
      }
    }
  }, [scene, ramp, state, reduced, interactive])
  return (
    <div className="tsl-mosaic" ref={wrapRef} data-interactive={interactive ? "" : undefined} role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
      <canvas ref={canvasRef} />
    </div>
  )
}

// ---------- nav ----------

function NavDrop({ item, onNavigate }: { item: TslNavItem; onNavigate: () => void }) {
  const [open, setOpen] = React.useState(false)
  const ref = React.useRef(null as HTMLLIElement | null)
  const close = React.useCallback(() => setOpen(false), [])
  useDismiss(open, close, ref)
  const timer = React.useRef(0)
  const items = item.items || []
  return (
    <li
      ref={ref}
      onPointerEnter={(e) => {
        if (e.pointerType !== "mouse") return
        clearTimeout(timer.current)
        setOpen(true)
      }}
      onPointerLeave={(e) => {
        if (e.pointerType !== "mouse") return
        timer.current = window.setTimeout(() => setOpen(false), 140)
      }}
    >
      <button type="button" className="tsl-menu-a" aria-expanded={open} aria-haspopup="true" onClick={() => setOpen((o) => !o)}>
        {item.label}
        <Icon name="down" size={12} />
      </button>
      <div className="tsl-drop" data-open={open ? "" : undefined}>
        <ul>
          {items.map((it, i) => (
            <li key={it.label}>
              <a
                href={it.href || "#"}
                tabIndex={open ? 0 : -1}
                onClick={() => {
                  setOpen(false)
                  onNavigate()
                }}
              >
                <i aria-hidden="true">
                  {[0, 1, 2, 3, 4, 5, 6, 7, 8].map((k) => (
                    <span key={k} data-on={(k + i * 2) % 3 === 0 ? "" : undefined} />
                  ))}
                </i>
                <b>{it.label}</b>
                {it.description ? <small>{it.description}</small> : null}
              </a>
            </li>
          ))}
        </ul>
      </div>
    </li>
  )
}

function Nav({
  brand,
  logo,
  home,
  nav,
  signIn,
  getStarted,
  dark,
  onTheme,
  themeToggle,
}: {
  brand: string
  logo?: React.ReactNode
  home: string
  nav: TslNavItem[]
  signIn: TslLink | null
  getStarted: TslLink
  dark: boolean
  onTheme: () => void
  themeToggle: boolean
}) {
  const [scrolled, setScrolled] = React.useState(false)
  const [progress, setProgress] = React.useState(0)
  const [sheet, setSheet] = React.useState(false)
  const sheetRef = React.useRef(null as HTMLDivElement | null)
  React.useEffect(() => {
    const on = () => {
      setScrolled(window.scrollY > 8)
      const max = document.documentElement.scrollHeight - window.innerHeight
      setProgress(max > 0 ? Math.min(1, window.scrollY / max) : 0)
    }
    on()
    window.addEventListener("scroll", on, { passive: true })
    return () => window.removeEventListener("scroll", on)
  }, [])
  React.useEffect(() => {
    if (!sheet) return
    const el = sheetRef.current
    const prev = document.activeElement as HTMLElement | null
    const overflow = document.body.style.overflow
    document.body.style.overflow = "hidden"
    const focusables = () => Array.from(el ? el.querySelectorAll("a,button") : []) as HTMLElement[]
    const f = focusables()
    if (f[0]) f[0].focus()
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSheet(false)
      if (e.key !== "Tab") return
      const list = focusables()
      if (!list.length) return
      const first = list[0]
      const last = list[list.length - 1]
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault()
        first.focus()
      }
    }
    document.addEventListener("keydown", key)
    return () => {
      document.removeEventListener("keydown", key)
      document.body.style.overflow = overflow
      if (prev) prev.focus()
    }
  }, [sheet])
  const segs = 24
  return (
    <header className="tsl-nav" data-scrolled={scrolled ? "" : undefined}>
      <div className="tsl-nav-in">
        <a href={home} className="tsl-brand" aria-label={brand + " home"}>
          {logo || <Mark />}
          <span>{brand}</span>
        </a>
        <nav aria-label="Main">
          <ul className="tsl-menu">
            {nav.map((item) =>
              item.items && item.items.length ? (
                <NavDrop key={item.label} item={item} onNavigate={() => {}} />
              ) : (
                <li key={item.label}>
                  <a className="tsl-menu-a" href={item.href || "#"} aria-current={item.current ? "page" : undefined}>
                    {item.label}
                  </a>
                </li>
              )
            )}
          </ul>
        </nav>
        <div className="tsl-nav-right">
          {themeToggle ? (
            <button type="button" className="tsl-icon-btn" onClick={onTheme} aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}>
              <Icon name={dark ? "sun" : "moon"} size={15} />
            </button>
          ) : null}
          {signIn ? (
            <a className="tsl-signin" href={signIn.href || "#"}>
              {signIn.label}
            </a>
          ) : null}
          <a className="tsl-btn tsl-btn-solid" href={getStarted.href || "#signup"}>
            {getStarted.label}
          </a>
          <button type="button" className="tsl-icon-btn tsl-burger" aria-label="Open menu" aria-expanded={sheet} onClick={() => setSheet(true)}>
            <Icon name="menu" size={16} />
          </button>
        </div>
      </div>
      <div className="tsl-progress" aria-hidden="true">
        {Array.from({ length: segs }, (_, i) => (
          <i key={i} data-on={progress > 0.002 && i / segs < progress ? "" : undefined} />
        ))}
      </div>
      {sheet ? (
        <div className="tsl-sheet" ref={sheetRef} role="dialog" aria-modal="true" aria-label="Menu">
          <div className="tsl-sheet-top">
            <span className="tsl-brand">
              {logo || <Mark />}
              <span>{brand}</span>
            </span>
            <button type="button" className="tsl-icon-btn" aria-label="Close menu" onClick={() => setSheet(false)}>
              <Icon name="x" size={16} />
            </button>
          </div>
          <ul className="tsl-sheet-list">
            {nav.map((item) => (
              <li key={item.label}>
                <a href={item.href || (item.items && item.items[0] ? item.items[0].href || "#" : "#")} onClick={() => setSheet(false)}>
                  {item.label}
                  <Icon name="arrow" size={16} />
                </a>
                {item.items ? (
                  <div className="tsl-sheet-sub">
                    {item.items.map((it) => (
                      <a key={it.label} href={it.href || "#"} onClick={() => setSheet(false)}>
                        {it.label}
                      </a>
                    ))}
                  </div>
                ) : null}
              </li>
            ))}
          </ul>
          <div className="tsl-sheet-cta">
            {signIn ? (
              <a className="tsl-btn tsl-btn-line" href={signIn.href || "#"} onClick={() => setSheet(false)}>
                {signIn.label}
              </a>
            ) : null}
            <a className="tsl-btn tsl-btn-solid" href={getStarted.href || "#signup"} onClick={() => setSheet(false)}>
              {getStarted.label}
              <Icon name="arrow" />
            </a>
          </div>
        </div>
      ) : null}
    </header>
  )
}

// ---------- hero prompt ----------

type RunState = { plan: TslRunPlan; prompt: string; model: string }

export function PromptCard({
  label,
  placeholder,
  suggestions,
  models,
  modelHints = [],
  reduced,
  planner,
  footer,
  request,
  disabled = false,
}: {
  label: string
  placeholder: string
  suggestions: string[]
  models: string[]
  modelHints?: string[]
  reduced: boolean
  planner: TslPlanner
  footer?: React.ReactNode
  /** Send this text as if typed and submitted; a new id sends it again. */
  request?: { text: string; id: number } | null
  /** Blocks sending, e.g. while settings are incomplete. */
  disabled?: boolean
}) {
  const uid = React.useId().replace(/:/g, "")
  const [text, setText] = React.useState("")
  const [focused, setFocused] = React.useState(false)
  const [model, setModel] = React.useState(0)
  const [menu, setMenu] = React.useState(false)
  const [hover, setHover] = React.useState(0)
  const [phase, setPhase] = React.useState("idle" as "idle" | "running" | "done" | "error")
  const [run, setRun] = React.useState(null as RunState | null)
  const [done, setDone] = React.useState(0)
  const [answer, setAnswer] = React.useState("")
  const [shown, setShown] = React.useState(0)
  const [elapsed, setElapsed] = React.useState(0)
  const [ph, setPh] = React.useState(placeholder)
  const [copied, setCopied] = React.useState(false)
  const token = React.useRef(0)
  const abort = React.useRef(null as AbortController | null)
  const menuRef = React.useRef(null as HTMLDivElement | null)
  const areaRef = React.useRef(null as HTMLTextAreaElement | null)
  const closeMenu = React.useCallback(() => setMenu(false), [])
  useDismiss(menu, closeMenu, menuRef)

  // the placeholder types out example tasks while the box is idle
  React.useEffect(() => {
    if (reduced || focused || text || phase !== "idle" || !suggestions.length) {
      setPh(placeholder)
      return
    }
    let alive = true
    let timer = 0
    const seq = [placeholder, ...suggestions]
    let i = 0
    let n = placeholder.length
    let dir = -1
    let wait = 2200
    const step = () => {
      if (!alive) return
      const s = seq[i]
      if (wait > 0) {
        const w = wait
        wait = 0
        timer = window.setTimeout(step, w)
        return
      }
      n += dir
      setPh(s.slice(0, Math.max(0, n)))
      if (dir < 0 && n <= 0) {
        i = (i + 1) % seq.length
        dir = 1
        n = 0
        timer = window.setTimeout(step, 260)
        return
      }
      if (dir > 0 && n >= seq[i].length) {
        dir = -1
        wait = 2000
      }
      timer = window.setTimeout(step, dir > 0 ? 34 : 16)
    }
    timer = window.setTimeout(step, 4200)
    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [reduced, focused, text, phase, placeholder, suggestions.join("\n")])

  const reset = () => {
    token.current++
    if (abort.current) abort.current.abort()
    setPhase("idle")
    setRun(null)
    setDone(0)
    setAnswer("")
    setShown(0)
    setElapsed(0)
    setCopied(false)
  }
  React.useEffect(() => () => {
    token.current++
    if (abort.current) abort.current.abort()
  }, [])

  const start = (value?: string) => {
    const prompt = (value ?? text).trim()
    if (!prompt || disabled) return
    if (value !== undefined) setText(value)
    const my = ++token.current
    const m = models[model] || models[0] || "Default"
    const ctrl = new AbortController()
    abort.current = ctrl
    setPhase("running")
    setDone(0)
    setAnswer("")
    setShown(0)
    setCopied(false)
    const t0 = performance.now()
    const sleep = (ms: number) => new Promise((r) => setTimeout(r, reduced ? 0 : ms))
    ;(async () => {
      let plan: TslRunPlan
      try {
        plan = await planner(prompt, m)
      } catch (err) {
        if (token.current !== my) return
        setRun({ plan: { kind: "error", title: "Request failed", steps: [], answer: "" }, prompt, model: m })
        setAnswer(err instanceof Error && err.message ? err.message : "The request could not finish.")
        setShown(9999)
        setPhase("error")
        return
      }
      if (token.current !== my) return
      setRun({ plan, prompt, model: m })
      for (let i = 0; i < plan.steps.length; i++) {
        await sleep(plan.steps[i].ms)
        if (token.current !== my) return
        setDone(i + 1)
        setElapsed(performance.now() - t0)
      }
      const final = plan.answer
      if (token.current !== my) return
      setElapsed(performance.now() - t0)
      setAnswer(final)
      if (reduced) {
        setShown(final.length)
        setPhase("done")
        return
      }
      let k = 0
      const tick = () => {
        if (token.current !== my) return
        k = Math.min(final.length, k + 3)
        setShown(k)
        if (k < final.length) window.setTimeout(tick, 18)
        else setPhase("done")
      }
      tick()
    })()
  }

  const lastRequest = React.useRef(0)
  React.useEffect(() => {
    if (!request || request.id === lastRequest.current) return
    lastRequest.current = request.id
    reset()
    start(request.text)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request])

  const stop = () => {
    token.current++
    if (abort.current) abort.current.abort()
    setPhase(run ? "done" : "idle")
    if (run && !answer) {
      setAnswer("Stopped. Nothing after the last finished step was run.")
      setShown(9999)
    }
  }

  const copy = () => {
    if (!answer) return
    try {
      navigator.clipboard.writeText(answer).then(() => setCopied(true), () => {})
    } catch {}
  }

  const busy = phase === "running"
  const open = !!run
  const pickModel = (i: number) => {
    setModel(i)
    setMenu(false)
  }
  return (
    <div className="tsl-prompt-wrap">
      <div className="tsl-prompt tsl-prompt-card" data-phase={phase}>
        <div className="tsl-prompt-label">
          <span>
            <i aria-hidden="true" />
            {label}
          </span>
          {open || busy ? (
            <span className="tsl-prompt-status" data-tone={!busy && run ? run.plan.tone : undefined} aria-live="polite">
              {busy ? (run ? "Step " + Math.min(done + 1, run.plan.steps.length) + "/" + run.plan.steps.length : "Sending") : phase === "error" ? "Failed" : run && run.plan.verdict ? run.plan.verdict : "Done in " + (elapsed / 1000).toFixed(1) + "s"}
            </span>
          ) : null}
        </div>
        <form
          className="tsl-prompt-box"
          onSubmit={(e) => {
            e.preventDefault()
            if (busy) stop()
            else start()
          }}
        >
          <div className="tsl-prompt-area">
            <label htmlFor={uid + "-ta"} className="tsl-sr">
              {label}
            </label>
            <textarea
              id={uid + "-ta"}
              ref={areaRef}
              rows={2}
              value={text}
              onChange={(e) => setText(e.target.value)}
              onFocus={() => setFocused(true)}
              onBlur={() => setFocused(false)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault()
                  if (!busy) start()
                }
              }}
              readOnly={busy}
            />
            {!text ? (
              <span className="tsl-ph" aria-hidden="true">
                {ph}
              </span>
            ) : null}
          </div>
          <div className="tsl-prompt-bar">
            <div className="tsl-model" ref={menuRef}>
              <button
                type="button"
                className="tsl-model-btn"
                aria-haspopup="listbox"
                aria-expanded={menu}
                aria-label={"Model: " + (models[model] || "")}
                onClick={() => {
                  setHover(model)
                  setMenu((o) => !o)
                }}
                onKeyDown={(e) => {
                  if (e.key === "ArrowUp" || e.key === "ArrowDown") {
                    e.preventDefault()
                    if (!menu) {
                      setHover(model)
                      setMenu(true)
                      return
                    }
                    const n = nextIndex(hover, e.key, models.length)
                    if (n > -1) setHover(n)
                  } else if (menu && (e.key === "Enter" || e.key === " ")) {
                    e.preventDefault()
                    pickModel(hover)
                  }
                }}
              >
                {models[model]}
                <Icon name="down" size={10} />
              </button>
              {menu ? (
                <ul className="tsl-model-list" role="listbox" aria-label="Model">
                  {models.map((m, i) => (
                    <li
                      key={m}
                      role="option"
                      aria-selected={i === model}
                      data-active={i === hover ? "" : undefined}
                      onPointerEnter={() => setHover(i)}
                      onClick={() => pickModel(i)}
                    >
                      <b>{m}</b>
                      {modelHints[i] ? <small>{modelHints[i]}</small> : null}
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
            <button type="submit" className="tsl-send" disabled={!busy && (disabled || !text.trim())} aria-label={busy ? "Stop" : "Send prompt"}>
              {busy ? (
                <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
                  <rect width="10" height="10" fill="currentColor" />
                </svg>
              ) : (
                <Icon name="up" size={14} stroke={1.8} />
              )}
            </button>
          </div>
        </form>
        <div className="tsl-run" data-open={open ? "" : undefined}>
          <div>
            {run ? (
              <div className="tsl-run-in">
                <div className="tsl-run-head">
                  <b>{run.plan.title}</b>
                  <span>{run.model}</span>
                </div>
                <ol className="tsl-steps">
                  {run.plan.steps.map((s, i) => {
                    const state = i < done ? "done" : i === done && busy ? "active" : "todo"
                    return (
                      <li key={s.tool + i} className="tsl-step" data-state={state} data-tone={state === "done" ? s.tone : undefined}>
                        {state === "done" ? (
                          <span className="tsl-check">
                            <Icon name="check" size={14} stroke={2} />
                          </span>
                        ) : state === "active" ? (
                          <span className="tsl-spin" aria-hidden="true">
                            <span />
                            <span />
                            <span />
                            <span />
                          </span>
                        ) : (
                          <span className="tsl-dot" aria-hidden="true" />
                        )}
                        <code>{s.tool}</code>
                        <em>{s.label}</em>
                        <time>{state === "done" ? s.time ?? (s.ms / 1000).toFixed(1) + "s" : ""}</time>
                      </li>
                    )
                  })}
                </ol>
                {answer || done === run.plan.steps.length ? (
                  <div className="tsl-answer" data-error={phase === "error" ? "" : undefined} aria-live="polite">
                    {answer.slice(0, shown)}
                    {busy ? <span className="tsl-caret" aria-hidden="true" /> : null}
                  </div>
                ) : null}
                <div className="tsl-run-foot">
                  {phase === "done" && answer ? (
                    <button type="button" className="tsl-mini" onClick={copy}>
                      <Icon name={copied ? "check" : "copy"} size={12} />
                      {copied ? "Copied" : "Copy"}
                    </button>
                  ) : null}
                  {!busy ? (
                    <button
                      type="button"
                      className="tsl-mini"
                      onClick={() => {
                        reset()
                        setText("")
                        if (areaRef.current) areaRef.current.focus()
                      }}
                    >
                      <Icon name="plus" size={12} />
                      New prompt
                    </button>
                  ) : null}
                  {footer}
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </div>
      {suggestions.length ? (
        <div className="tsl-chips" aria-label="Example prompts">
          {suggestions.slice(0, 3).map((s) => (
            <button
              key={s}
              type="button"
              className="tsl-chip"
              disabled={busy || disabled}
              onClick={() => {
                reset()
                start(s)
              }}
            >
              {shortLabel(s, 44)}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}

// ---------- problem ----------

function IsoIcon({ icon, uid }: { icon: TslProblemIcon; uid: string }) {
  const top = "url(#" + uid + "-top)"
  const left = "url(#" + uid + "-left)"
  const right = "#070b24"
  const edge = "rgba(255,255,255,.14)"
  const Box = ({ b, cls, glow }: { b: { top: string; left: string; right: string }; cls?: string; glow?: boolean }) => (
    <g className={cls}>
      <polygon points={b.left} fill={glow ? "var(--tsl-c1)" : left} stroke={edge} strokeWidth=".6" />
      <polygon points={b.right} fill={glow ? "var(--tsl-c0)" : right} stroke={edge} strokeWidth=".6" />
      <polygon points={b.top} fill={glow ? "var(--tsl-c2)" : top} stroke={edge} strokeWidth=".6" />
    </g>
  )
  let body: React.ReactNode = null
  if (icon === "layers") {
    body = (
      <>
        <Box b={isoBox(-26, -26, 0, 52, 52, 8)} />
        <Box b={isoBox(-26, -26, 18, 52, 52, 8)} cls="lift1" />
        <Box b={isoBox(-26, -26, 36, 52, 52, 8)} cls="lift2" />
        <Box b={isoBox(-10, -10, 54, 20, 20, 10)} cls="lift3" glow />
      </>
    )
  } else if (icon === "steps") {
    const hs = [10, 20, 30, 42]
    body = (
      <>
        <Box b={isoBox(-38, -20, 0, 76, 40, 4)} />
        {hs.map((h, i) => (
          <Box key={i} b={isoBox(-36 + i * 18, -18, 4, 18, 36, h)} cls="rise" glow={i === 3} />
        ))}
      </>
    )
  } else {
    body = (
      <>
        <Box b={isoBox(-30, -30, 0, 60, 60, 5)} />
        <Box b={isoBox(-12, -12, 5, 24, 24, 24)} glow />
        <Box b={isoBox(-22, -22, 5, 44, 44, 40)} cls="liftlid" />
      </>
    )
  }
  return (
    <svg className="tsl-pcard-icon" viewBox="-62 -92 124 124" aria-hidden="true">
      <defs>
        <linearGradient id={uid + "-top"} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--tsl-c2)" />
          <stop offset="1" stopColor="var(--tsl-c1)" />
        </linearGradient>
        <linearGradient id={uid + "-left"} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="var(--tsl-c0)" />
          <stop offset="1" stopColor="#0b1440" />
        </linearGradient>
      </defs>
      {body}
    </svg>
  )
}

export function Problem({ data }: { data: TslProblem }) {
  const { reduced } = useTsl()
  const uid = React.useId().replace(/:/g, "")
  const [open, setOpen] = React.useState(-1)
  const words = toneWords(data.title)
  const icons = ["layers", "steps", "eye"]
  return (
    <section className="tsl-sec tsl-night" data-sec="problem" aria-labelledby={uid + "-h"}>
      <span className="tsl-cross" data-side="l" />
      <span className="tsl-cross" data-side="r" />
      <div className="tsl-center">
        <Kicker text={data.kicker} reduced={reduced} />
        <h2 className="tsl-h2 tsl-scrub" id={uid + "-h"}>
          {words.map((w, i) => (
            <React.Fragment key={i}>
              <span className="tsl-w" data-muted={w.muted ? "" : undefined}>
                {w.word}
              </span>{" "}
            </React.Fragment>
          ))}
        </h2>
      </div>
      <div className="tsl-pcards">
        {data.items.map((it, i) => (
          <article key={it.title} className="tsl-pcard" data-open={open === i ? "" : undefined}>
            <span className="tsl-pcard-px" aria-hidden="true">
              {[0, 1, 2, 3, 4, 5, 6, 7, 8].map((k) => (
                <span key={k} style={{ transitionDelay: k * 30 + "ms" }} />
              ))}
            </span>
            <IsoIcon icon={(it.icon || icons[i % 3]) as TslProblemIcon} uid={uid + "-" + i} />
            <div>
              <h3>
                <button type="button" aria-expanded={open === i} aria-controls={uid + "-p" + i} onClick={() => setOpen((o) => (o === i ? -1 : i))}>
                  {it.title}
                </button>
              </h3>
              <div className="tsl-pcard-body" id={uid + "-p" + i}>
                <div>
                  <p>{it.text}</p>
                </div>
              </div>
            </div>
            <span className="tsl-pcard-num" aria-hidden="true">
              0{i + 1}
            </span>
          </article>
        ))}
      </div>
    </section>
  )
}

// ---------- solution ----------

export function Solution({ data, renderStage }: { data: TslSolution; renderStage: (visual: string, active: boolean) => React.ReactNode }) {
  const { ramp, reduced } = useTsl()
  const uid = React.useId().replace(/:/g, "")
  const [active, setActive] = React.useState(0)
  const stageState = React.useRef({ cell: 15, shift: 0 })
  const rootRef = React.useRef(null as HTMLElement | null)
  const cardRef = React.useRef(null as HTMLDivElement | null)
  const innerRef = React.useRef(null as HTMLDivElement | null)
  const lastH = React.useRef(0)
  const words = toneWords(data.title)
  const feats = data.features
  const f = feats[active] || feats[0]

  // each step owns the stage while it crosses the middle of the screen
  useIsoLayoutEffect(() => {
    const root = rootRef.current
    if (!root) return
    const ctx = gsap.context(() => {
      const mm = gsap.matchMedia()
      mm.add("(min-width: 1024px)", () => {
        const items = gsap.utils.toArray(".tsl-step-item") as HTMLElement[]
        items.forEach((el, i) => {
          ScrollTrigger.create({
            trigger: el,
            start: "top 55%",
            end: "bottom 55%",
            onToggle: (self) => {
              if (self.isActive) setActive(i)
            },
          })
        })
      })
    }, root)
    return () => ctx.revert()
  }, [feats.length])

  // the card morphs to the new content's height, the mosaic drifts to a new pattern
  useIsoLayoutEffect(() => {
    const card = cardRef.current
    const inner = innerRef.current
    if (!card || !inner) return
    const to = inner.offsetHeight
    if (reduced || !lastH.current) {
      lastH.current = to
      stageState.current.shift = active * 1.6
      return
    }
    const from = lastH.current
    lastH.current = to
    const ctx = gsap.context(() => {
      gsap.fromTo(card, { height: from }, { height: to, duration: 0.7, ease: "expo.out", clearProps: "height" })
      gsap.fromTo(inner, { opacity: 0, y: 14, filter: "blur(6px)" }, { opacity: 1, y: 0, filter: "blur(0px)", duration: 0.6, ease: "power3.out", clearProps: "filter" })
      gsap.to(stageState.current, { shift: active * 1.6, duration: 1.4, ease: "power2.inOut" })
      gsap.fromTo(stageState.current, { cell: 34 }, { cell: 15, duration: 0.9, ease: "steps(6)" })
    })
    return () => ctx.revert()
  }, [active, reduced])

  return (
    <section className="tsl-sec" data-sec="product" ref={rootRef} aria-labelledby={uid + "-h"}>
      <span className="tsl-cross" data-side="l" />
      <span className="tsl-cross" data-side="r" />
      <div className="tsl-center">
        <Kicker text={data.kicker} reduced={reduced} />
        <h2 className="tsl-h2 tsl-scrub" id={uid + "-h"}>
          {words.map((w, i) => (
            <React.Fragment key={i}>
              <span className="tsl-w" data-muted={w.muted ? "" : undefined}>
                {w.word}
              </span>{" "}
            </React.Fragment>
          ))}
        </h2>
      </div>
      <div className="tsl-feat">
        <div className="tsl-feat-steps">
          <span className="tsl-feat-rail" aria-hidden="true">
            <i style={{ height: 100 / feats.length + "%", transform: "translateY(" + active * 100 + "%)" }} />
          </span>
          {feats.map((ft, i) => (
            <article key={ft.kicker} className="tsl-step-item" data-active={i === active ? "" : undefined}>
              <span className="tsl-kicker">
                <KickerGlyph />
                {ft.kicker}
              </span>
              <h3 className="tsl-h3">{ft.title}</h3>
              <p>{ft.text}</p>
              {ft.points ? (
                <ul className="tsl-points">
                  {ft.points.map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
              ) : null}
            </article>
          ))}
        </div>
        <div className="tsl-feat-tabs" role="tablist" aria-label="Product areas">
          {feats.map((ft, i) => (
            <button
              key={ft.kicker}
              type="button"
              role="tab"
              aria-selected={i === active}
              onClick={() => setActive(i)}
              onKeyDown={(e) => {
                const n = nextIndex(i, e.key, feats.length)
                if (n > -1) {
                  e.preventDefault()
                  setActive(n)
                  const sib = (e.currentTarget.parentElement as HTMLElement).children[n] as HTMLElement
                  if (sib) sib.focus()
                }
              }}
            >
              {ft.kicker}
            </button>
          ))}
        </div>
        <div className="tsl-feat-mobile">
          <h3 className="tsl-h3">{f.title}</h3>
          <p>{f.text}</p>
        </div>
        <div className="tsl-stage">
          <Mosaic scene="silk" ramp={ramp} state={stageState.current} reduced={reduced} interactive />
          <span className="tsl-stage-tag" aria-hidden="true">
            0{active + 1} / {f.kicker}
          </span>
          <div className="tsl-stage-card" ref={cardRef}>
            <div ref={innerRef} key={active}>
              {renderStage(f.visual, true)}
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

// ---------- stats ----------

export function Stats({ stats, label = "By the numbers" }: { stats: TslStat[]; label?: string }) {
  const { reduced } = useTsl()
  const ref = React.useRef(null as HTMLElement | null)
  const seen = useInView(ref)
  const nums = React.useRef([] as (HTMLElement | null)[])
  const [lit, setLit] = React.useState(false)
  React.useEffect(() => {
    if (!seen) return
    setLit(true)
    if (reduced) return
    const tweens = stats.map((s, i) => {
      const el = nums.current[i]
      const o = { v: 0 }
      return gsap.to(o, {
        v: s.value,
        duration: 1.8,
        delay: i * 0.12,
        ease: "expo.out",
        onUpdate: () => {
          if (el) el.textContent = (s.prefix || "") + formatStat(o.v, s.decimals || 0) + (s.suffix || "")
        },
      })
    })
    return () => tweens.forEach((t) => t.kill())
  }, [seen, reduced, stats])
  return (
    <section className="tsl-sec tsl-stats" data-sec="stats" ref={ref} aria-label={label}>
      <span className="tsl-cross" data-side="l" />
      <span className="tsl-cross" data-side="r" />
      {stats.map((s, i) => (
        <div className="tsl-stat" key={s.label}>
          <b
            ref={(el) => {
              nums.current[i] = el
            }}
          >
            {(s.prefix || "") + formatStat(s.value, s.decimals || 0) + (s.suffix || "")}
          </b>
          <span className="tsl-stat-px" aria-hidden="true">
            {Array.from({ length: 12 }, (_, k) => (
              <span key={k} data-on={lit && k < 4 + ((i * 3 + 5) % 8) ? "" : undefined} style={{ transitionDelay: k * 50 + i * 80 + "ms" }} />
            ))}
          </span>
          <p>{s.label}</p>
        </div>
      ))}
    </section>
  )
}

// ---------- call to action ----------

export function Cta({
  title,
  text,
  actions,
  children,
}: {
  title: string
  text?: React.ReactNode
  actions: TslLink[]
  children?: React.ReactNode
}) {
  const { ramp, reduced } = useTsl()
  const uid = React.useId().replace(/:/g, "")
  const state = React.useRef({ cell: 18, shift: 0 })
  return (
    <section className="tsl-sec tsl-cta" data-sec="start" aria-labelledby={uid + "-h"}>
      <span className="tsl-cross" data-side="l" />
      <span className="tsl-cross" data-side="r" />
      <div className="tsl-cta-panel">
        <Mosaic scene="bloom" ramp={ramp} state={state.current} reduced={reduced} interactive />
        <div className="tsl-cta-card">
          <h2 className="tsl-h2" id={uid + "-h"}>
            {title}
          </h2>
          {text ? <p>{text}</p> : null}
          {children}
          <div className="tsl-cta-actions">
            {actions.map((a, i) => (
              <a key={a.label} className={"tsl-btn " + (i === 0 ? "tsl-btn-solid" : "tsl-btn-line")} href={a.href || "#"}>
                {a.label}
                {i === 0 ? <Icon name="arrow" /> : null}
              </a>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}

// ---------- footer ----------

/** The brand set in pixels. They fly in when the footer arrives and scatter from the pointer. */
function PixelWordmark({ text, ramp, reduced }: { text: string; ramp: string[]; reduced: boolean }) {
  const wrapRef = React.useRef(null as HTMLDivElement | null)
  const canvasRef = React.useRef(null as HTMLCanvasElement | null)
  React.useEffect(() => {
    const wrap = wrapRef.current
    const canvas = canvasRef.current
    if (!wrap || !canvas) return
    const ctx = canvas.getContext("2d")
    if (!ctx) return
    type P = { x: number; y: number; vx: number; vy: number; tx: number; ty: number; c: number }
    let ps: P[] = []
    let w = 0
    let h = 0
    let cell = 6
    let raf = 0
    let running = false
    let visible = false
    let arrived = reduced
    const ptr = { x: -9999, y: -9999, on: false }
    const n = ramp.length

    const build = () => {
      const r = wrap.getBoundingClientRect()
      w = Math.max(1, Math.round(r.width))
      h = Math.max(1, Math.round(r.height))
      canvas.style.width = w + "px"
      cell = Math.max(4, Math.round(w / 190))
      const family = "Charter, 'Bitstream Charter', 'Iowan Old Style', Georgia, serif"
      const probe = document.createElement("canvas").getContext("2d")
      if (!probe) return
      probe.font = "400 100px " + family
      const m = probe.measureText(text)
      const left = m.actualBoundingBoxLeft || 0
      const span = (m.actualBoundingBoxRight || m.width) + left
      const size = (100 * w) / Math.max(1, span)
      const asc = (m.actualBoundingBoxAscent || 72) * (size / 100)
      const desc = (m.actualBoundingBoxDescent || 0) * (size / 100)
      const fit = Math.ceil(asc + desc + cell)
      if (Math.abs(fit - h) > 2) {
        // the type sets the box: the ResizeObserver rebuilds at the new height
        wrap.style.height = fit + "px"
        h = fit
      }
      const dpr = Math.min(2, window.devicePixelRatio || 1)
      canvas.width = Math.round(w * dpr)
      canvas.height = Math.round(h * dpr)
      canvas.style.height = h + "px"
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      const off = document.createElement("canvas")
      off.width = w
      off.height = h
      const o = off.getContext("2d", { willReadFrequently: true })
      if (!o) return
      o.font = "400 " + size + "px " + family
      o.textBaseline = "alphabetic"
      o.fillStyle = "#000"
      o.fillText(text, (left * size) / 100, Math.round(asc + cell / 2))
      const data = o.getImageData(0, 0, w, h).data
      const next: P[] = []
      for (let y = Math.floor(cell / 2); y < h; y += cell) {
        for (let x = Math.floor(cell / 2); x < w; x += cell) {
          if (data[(y * w + x) * 4 + 3] < 120) continue
          const tx = x - cell / 2
          const ty = y - cell / 2
          const v = clamp01(fbm(x / 140, y / 90, 4) * 0.9 + (1 - y / h) * 0.25 - 0.1)
          const old = arrived
          next.push({
            x: old ? tx : tx + (Math.random() - 0.5) * w * 0.4,
            y: old ? ty : ty + h * (0.6 + Math.random() * 0.9),
            vx: 0,
            vy: 0,
            tx,
            ty,
            c: Math.min(n - 1, Math.round(v * (n - 1))),
          })
        }
      }
      ps = next
    }

    const draw = () => {
      ctx.clearRect(0, 0, w, h)
      for (let b = 0; b < n; b++) {
        ctx.fillStyle = ramp[b]
        ctx.beginPath()
        for (const p of ps) if (p.c === b) ctx.rect(p.x, p.y, cell - 1, cell - 1)
        ctx.fill()
      }
    }

    const step = () => {
      let energy = 0
      const R = 90
      for (const p of ps) {
        let ax = (p.tx - p.x) * 0.07
        let ay = (p.ty - p.y) * 0.07
        if (ptr.on) {
          const dx = p.x - ptr.x
          const dy = p.y - ptr.y
          const d2 = dx * dx + dy * dy
          if (d2 < R * R) {
            const d = Math.sqrt(d2) || 1
            const f = (1 - d / R) * 6
            ax += (dx / d) * f
            ay += (dy / d) * f
          }
        }
        p.vx = (p.vx + ax) * 0.8
        p.vy = (p.vy + ay) * 0.8
        p.x += p.vx
        p.y += p.vy
        energy += Math.abs(p.vx) + Math.abs(p.vy) + Math.abs(p.tx - p.x) + Math.abs(p.ty - p.y)
      }
      return energy / Math.max(1, ps.length)
    }

    const loop = () => {
      const e = step()
      draw()
      if (e < 0.02 && !ptr.on) {
        // settled: snap and sleep until something moves it again
        for (const p of ps) {
          p.x = p.tx
          p.y = p.ty
        }
        draw()
        running = false
        return
      }
      raf = requestAnimationFrame(loop)
    }
    const start = () => {
      if (running || reduced || !visible || document.hidden) return
      running = true
      raf = requestAnimationFrame(loop)
    }

    const rebuild = () => {
      build()
      draw()
      start()
    }
    const fonts = (document as Document & { fonts?: { ready: Promise<unknown> } }).fonts
    rebuild()
    if (fonts) fonts.ready.then(() => rebuild(), () => {})
    const ro = new ResizeObserver(() => {
      arrived = true
      rebuild()
    })
    ro.observe(wrap)
    const io = new IntersectionObserver(
      (es) => {
        visible = es.some((e) => e.isIntersecting)
        if (visible) {
          arrived = true
          start()
        }
      },
      { rootMargin: "0px 0px -10% 0px" }
    )
    io.observe(wrap)
    const move = (e: PointerEvent) => {
      if (e.pointerType !== "mouse") return
      const r = wrap.getBoundingClientRect()
      ptr.x = e.clientX - r.left
      ptr.y = e.clientY - r.top
      ptr.on = true
      start()
    }
    const leave = () => {
      ptr.on = false
      start()
    }
    wrap.addEventListener("pointermove", move)
    wrap.addEventListener("pointerleave", leave)
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      io.disconnect()
      wrap.removeEventListener("pointermove", move)
      wrap.removeEventListener("pointerleave", leave)
    }
  }, [text, ramp, reduced])
  return (
    <div className="tsl-wordmark" ref={wrapRef} aria-hidden="true">
      <canvas ref={canvasRef} />
    </div>
  )
}

function Footer({ brand, logo, data, ramp, reduced }: { brand: string; logo?: React.ReactNode; data: TslFooter; ramp: string[]; reduced: boolean }) {
  return (
    <footer className="tsl-sec tsl-foot" data-sec="footer">
      <span className="tsl-cross" data-side="l" />
      <span className="tsl-cross" data-side="r" />
      <div className="tsl-foot-top">
        <div>
          <span className="tsl-brand">
            {logo || <Mark />}
            <span>{brand}</span>
          </span>
          <p>{data.tagline}</p>
        </div>
        {data.columns.map((c) => (
          <nav key={c.title} aria-label={c.title}>
            <h4>{c.title}</h4>
            <ul>
              {c.links.map((l) => (
                <li key={l.label}>
                  <a href={l.href || "#"}>{l.label}</a>
                </li>
              ))}
            </ul>
          </nav>
        ))}
      </div>
      <PixelWordmark text={brand} ramp={ramp} reduced={reduced} />
      <div className="tsl-foot-bottom">
        <span>{data.note}</span>
        {data.status ? (
          <span className="tsl-status">
            <i aria-hidden="true" />
            {data.status}
          </span>
        ) : null}
      </div>
    </footer>
  )
}

// ---------- hero ----------

export type TslHero = {
  badge?: { mark: string; label: string } | null
  title: string
  text?: string
  primary?: TslLink
  secondary?: TslLink
  prompt: {
    label: string
    placeholder: string
    suggestions: string[]
    models: string[]
    modelHints?: string[]
    footer?: React.ReactNode
  }
  ruler?: [string, string]
}

/** Headline, actions and the mosaic panel holding a working prompt card. */
export function Hero({ data, planner }: { data: TslHero; planner: TslPlanner }) {
  const { ramp, reduced } = useTsl()
  const uid = React.useId().replace(/:/g, "")
  const rootRef = React.useRef(null as HTMLElement | null)
  const heroState = React.useRef({ cell: 18, shift: 0 })
  const titleLines = String(data.title || "").split("\n")

  // the hero assembles, then breaks back into blocks as you scroll past
  useIsoLayoutEffect(() => {
    const root = rootRef.current
    if (!root || reduced) return
    const ctx = gsap.context(() => {
      const tl = gsap.timeline({ defaults: { ease: "expo.out" } })
      if (data.badge) tl.from(".tsl-badge", { y: 12, opacity: 0, duration: 0.8 }, 0.05)
      tl.from(".tsl-line > span", { yPercent: 115, duration: 1.15, stagger: 0.09 }, 0.1)
        .from(".tsl-hero-text", { y: 14, opacity: 0, duration: 0.9 }, 0.45)
        .from(".tsl-hero-actions > *", { y: 12, opacity: 0, duration: 0.8, stagger: 0.07 }, 0.55)
        .fromTo(".tsl-hero-panel", { clipPath: "inset(10% 7% 10% 7%)" }, { clipPath: "inset(0% 0% 0% 0%)", duration: 1.5, ease: "expo.inOut", clearProps: "clipPath" }, 0.25)
        .fromTo(heroState.current, { cell: 92 }, { cell: 18, duration: 1.7, ease: "steps(9)" }, 0.25)
        .from(".tsl-prompt-card", { y: 34, opacity: 0, scale: 0.96, duration: 1.1 }, 1.0)
        .from(".tsl-chips > *", { y: 10, opacity: 0, duration: 0.7, stagger: 0.06 }, 1.25)
      gsap.fromTo(
        heroState.current,
        { cell: 18 },
        { cell: 46, ease: "none", immediateRender: false, scrollTrigger: { trigger: ".tsl-hero-panel", start: "center 45%", end: "bottom top", scrub: true } }
      )
      gsap.to(".tsl-prompt-wrap", { yPercent: -14, ease: "none", scrollTrigger: { trigger: ".tsl-hero-panel", start: "top top", end: "bottom top", scrub: true } })
    }, root)
    return () => ctx.revert()
  }, [reduced])

  return (
    <section className="tsl-sec tsl-hero" data-sec="top" ref={rootRef} aria-labelledby={uid + "-h1"}>
      {data.badge ? (
        <span className="tsl-badge">
          <i aria-hidden="true">{data.badge.mark}</i>
          {data.badge.label}
        </span>
      ) : null}
      <h1 className="tsl-h1" id={uid + "-h1"}>
        {titleLines.map((line, i) => (
          <React.Fragment key={i}>
            <span className="tsl-line">
              <span>{line}</span>
            </span>{" "}
          </React.Fragment>
        ))}
      </h1>
      {data.text ? <p className="tsl-lead tsl-hero-text">{data.text}</p> : null}
      <div className="tsl-hero-actions">
        {data.secondary ? (
          <a className="tsl-btn tsl-btn-line" href={data.secondary.href || "#"}>
            {data.secondary.label}
          </a>
        ) : null}
        {data.primary ? (
          <a className="tsl-btn tsl-btn-solid" href={data.primary.href || "#"}>
            <Icon name="arrow" />
            {data.primary.label}
          </a>
        ) : null}
      </div>
      <div className="tsl-hero-panel">
        <Mosaic scene="columns" ramp={ramp} state={heroState.current} reduced={reduced} interactive label="A shifting mosaic of warm pixels that sharpens under the pointer" />
        <PromptCard
          label={data.prompt.label}
          placeholder={data.prompt.placeholder}
          suggestions={data.prompt.suggestions}
          models={data.prompt.models}
          modelHints={data.prompt.modelHints}
          reduced={reduced}
          planner={planner}
          footer={data.prompt.footer}
        />
        {data.ruler ? (
          <div className="tsl-hero-ruler" aria-hidden="true">
            <span>{data.ruler[0]}</span>
            <span>{data.ruler[1]}</span>
          </div>
        ) : null}
      </div>
    </section>
  )
}

/** A marquee of names with small drawn marks. */
export function LogoStrip({ logos, label }: { logos: TslLogo[]; label: string }) {
  return (
    <section className="tsl-logos" aria-label={label}>
      <div className="tsl-logos-track">
        {[0, 1].map((k) => (
          <div key={k} style={{ display: "flex" }} aria-hidden={k === 1 ? true : undefined}>
            {logos.map((l) => (
              <span className="tsl-logo" key={l.name + k} data-style={l.style || "bold"}>
                <LogoMark mark={l.mark} />
                {l.name}
              </span>
            ))}
          </div>
        ))}
      </div>
    </section>
  )
}

/** A plain section in the frame: kicker, serif headline, optional lead, then content. */
export function Section({
  id,
  kicker,
  title,
  lead,
  night,
  children,
}: {
  id?: string
  kicker?: string
  title?: string
  lead?: React.ReactNode
  night?: boolean
  children?: React.ReactNode
}) {
  const uid = React.useId().replace(/:/g, "")
  const words = title ? toneWords(title) : []
  return (
    <section className={"tsl-sec" + (night ? " tsl-night" : "")} data-sec={id} aria-labelledby={title ? uid + "-h" : undefined}>
      <span className="tsl-cross" data-side="l" />
      <span className="tsl-cross" data-side="r" />
      {kicker || title ? (
        <div className="tsl-center">
          {kicker ? <Kicker text={kicker} /> : null}
          {title ? (
            <h2 className="tsl-h2 tsl-scrub" id={uid + "-h"}>
              {words.map((w, i) => (
                <React.Fragment key={i}>
                  <span className="tsl-w" data-muted={w.muted ? "" : undefined}>
                    {w.word}
                  </span>{" "}
                </React.Fragment>
              ))}
            </h2>
          ) : null}
          {lead ? <p className="tsl-lead tsl-sec-lead">{lead}</p> : null}
        </div>
      ) : null}
      {children}
    </section>
  )
}

// ---------- the shell ----------

export interface TesselShellProps {
  brand: string
  logo?: React.ReactNode
  /** Where the brand links to. */
  home?: string
  announcement?: TslLink | null
  nav: TslNavItem[]
  signIn?: TslLink | null
  getStarted: TslLink
  footer?: TslFooter | null
  palette?: TslPaletteName | TslPaletteInput
  theme?: "system" | "light" | "dark"
  themeToggle?: boolean
  /** Basis for the hero panel's and the call to action's height. */
  height?: string
  className?: string
  children: React.ReactNode
}

/** Announcement, sticky nav, the hairline frame, the footer and the theme. Pages put sections inside. */
export function TesselShell({
  brand,
  logo,
  home = "#top",
  announcement = null,
  nav,
  signIn = null,
  getStarted,
  footer = null,
  palette = "cobalt",
  theme = "system",
  themeToggle = true,
  height = "100svh",
  className,
  children,
}: TesselShellProps) {
  const rootRef = React.useRef(null as HTMLDivElement | null)
  const reduced = useReducedMotion()
  const [dark, setDark] = useTheme(theme)
  const [bar, setBar] = React.useState(true)
  const uid = React.useId().replace(/:/g, "")

  const palKey = typeof palette === "string" ? palette : JSON.stringify(palette)
  const pal = React.useMemo(() => resolvePalette(palette), [palKey])
  const stops = dark ? darkStops(pal.stops) : pal.stops
  const stopsKey = stops.join()
  const ramp = React.useMemo(() => buildRamp(stops, 28), [stopsKey])
  const ctxValue = React.useMemo(() => ({ ramp, reduced, dark }), [ramp, reduced, dark])

  // in-page links scroll to their section
  const onClick = (e: React.MouseEvent) => {
    const a = (e.target as HTMLElement).closest("a")
    if (!a) return
    const href = a.getAttribute("href") || ""
    if (href === "#") {
      e.preventDefault()
      return
    }
    if (!href.startsWith("#")) return
    const root = rootRef.current
    const el = root && root.querySelector('[data-sec="' + href.slice(1) + '"]')
    if (!el) return
    e.preventDefault()
    el.scrollIntoView({ behavior: reduced ? "auto" : "smooth", block: "start" })
  }

  // sections reveal and scrub as you scroll
  useIsoLayoutEffect(() => {
    const root = rootRef.current
    if (!root || reduced) return
    const has = (sel: string) => !!root.querySelector(sel)
    const ctx = gsap.context(() => {
      gsap.utils.toArray(".tsl-scrub").forEach((el) => {
        const words = (el as HTMLElement).querySelectorAll(".tsl-w")
        gsap.fromTo(words, { opacity: 0.16 }, { opacity: 1, stagger: 0.08, ease: "none", scrollTrigger: { trigger: el as HTMLElement, start: "top 82%", end: "bottom 50%", scrub: true } })
      })
      if (has(".tsl-pcards")) gsap.from(".tsl-pcard", { y: 48, opacity: 0, duration: 1.1, stagger: 0.12, scrollTrigger: { trigger: ".tsl-pcards", start: "top 82%" } })
      if (has(".tsl-feat"))
        gsap.fromTo(".tsl-stage", { clipPath: "inset(8% 6% 8% 6%)" }, { clipPath: "inset(0% 0% 0% 0%)", duration: 1.3, ease: "expo.inOut", clearProps: "clipPath", scrollTrigger: { trigger: ".tsl-feat", start: "top 75%" } })
      if (has(".tsl-cta-panel")) {
        gsap.fromTo(".tsl-cta-panel", { clipPath: "inset(12% 8% 12% 8%)" }, { clipPath: "inset(0% 0% 0% 0%)", ease: "none", scrollTrigger: { trigger: ".tsl-cta-panel", start: "top 95%", end: "top 35%", scrub: true } })
        gsap.from(".tsl-cta-card", { y: 40, opacity: 0, duration: 1, scrollTrigger: { trigger: ".tsl-cta-panel", start: "top 60%" } })
      }
      const reveals = gsap.utils.toArray("[data-reveal]") as HTMLElement[]
      if (reveals.length) {
        gsap.set(reveals, { y: 28, opacity: 0 })
        ScrollTrigger.batch(reveals, {
          start: "top 88%",
          once: true,
          onEnter: (batch) => gsap.to(batch, { y: 0, opacity: 1, duration: 1, ease: "expo.out", stagger: 0.08, overwrite: true }),
        })
      }
    }, root)

    // sections change height (the run panel opens, fonts land): keep triggers honest
    let t = 0
    const ro = new ResizeObserver(() => {
      clearTimeout(t)
      t = window.setTimeout(() => ScrollTrigger.refresh(), 160)
    })
    ro.observe(root)
    return () => {
      clearTimeout(t)
      ro.disconnect()
      ctx.revert()
    }
  }, [reduced])

  const accent = pal.accent
  const rootStyle = {
    "--tsl-h": height,
    "--tsl-accent": dark ? mixHex(accent, "#ffffff", 0.18) : accent,
    "--tsl-on-accent": onColor(dark ? mixHex(accent, "#ffffff", 0.18) : accent),
    "--tsl-c0": stops[0],
    "--tsl-c1": stops[1],
    "--tsl-c2": stops[2],
    "--tsl-c3": stops[3],
  } as React.CSSProperties

  return (
    <TslContext.Provider value={ctxValue}>
      <div ref={rootRef} className={"tsl-root" + (className ? " " + className : "")} style={rootStyle} data-theme={dark ? "dark" : "light"} onClick={onClick}>
        <style>{TSL_CSS}</style>
        <a href={"#" + uid + "-main"} className="tsl-skip" onClick={(e) => e.stopPropagation()}>
          Skip to content
        </a>
        {announcement && bar ? (
          <div className="tsl-announce">
            <a href={announcement.href || "#"}>
              <span>
                <b>{brand}</b> {announcement.label}
              </span>
              <Icon name="arrow" size={12} />
            </a>
            <button type="button" className="tsl-announce-x" aria-label="Dismiss announcement" onClick={() => setBar(false)}>
              <Icon name="x" size={11} />
            </button>
          </div>
        ) : null}
        <Nav brand={brand} logo={logo} home={home} nav={nav} signIn={signIn} getStarted={getStarted} dark={dark} onTheme={() => setDark(!dark)} themeToggle={themeToggle} />
        <div className="tsl-frame">
          <main id={uid + "-main"}>{children}</main>
          {footer ? <Footer brand={brand} logo={logo} data={footer} ramp={ramp} reduced={reduced} /> : null}
        </div>
      </div>
    </TslContext.Provider>
  )
}
