import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class"],
  content: [
    "./src/pages/**/*.{js,ts,jsx,tsx,mdx}",
    "./src/components/**/*.{js,ts,jsx,tsx,mdx}",
    "./src/app/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  theme: {
    extend: {
      colors: {
        background: "var(--background)",
        foreground: "var(--foreground)",
        obsidian: {
          50: "#f8fafc",
          100: "#f1f5f9",
          200: "#e2e8f0",
          300: "#cbd5e1",
          400: "#94a3b8",
          500: "#64748b",
          600: "#475569",
          700: "#334155",
          800: "#1e293b",
          850: "#131b2e",
          900: "#0b1122",
          950: "#050814",
          1000: "#02040a",
        },
        cyan: {
          DEFAULT: "#00f0ff",
          glow: "#38bdf8",
          dark: "#008ba3",
        },
        emerald: {
          DEFAULT: "#10b981",
          glow: "#34d399",
          neon: "#00f29b",
        },
        violet: {
          DEFAULT: "#8b5cf6",
          glow: "#a78bfa",
          neon: "#b026ff",
        },
        amber: {
          DEFAULT: "#f59e0b",
          glow: "#fbbf24",
          solar: "#ffaa00",
        },
        rose: {
          DEFAULT: "#f43f5e",
          glow: "#fb7185",
          danger: "#ff3366",
        },
      },
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Display",
          "Inter",
          "Segoe UI",
          "Roboto",
          "sans-serif",
        ],
        mono: [
          "JetBrains Mono",
          "SF Mono",
          "ui-monospace",
          "Menlo",
          "Monaco",
          "Consolas",
          "monospace",
        ],
      },
      animation: {
        "aurora": "aurora 60s linear infinite",
        "pulse-slow": "pulse 4s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "shimmer": "shimmer 2.5s linear infinite",
        "float": "float 6s ease-in-out infinite",
        "spin-slow": "spin 20s linear infinite",
        "radar": "radar 3s ease-out infinite",
        "border-glow": "borderGlow 4s ease infinite",
      },
      keyframes: {
        aurora: {
          from: { backgroundPosition: "50% 50%, 50% 50%" },
          to: { backgroundPosition: "350% 50%, 350% 50%" },
        },
        shimmer: {
          from: { backgroundPosition: "0 0" },
          to: { backgroundPosition: "-200% 0" },
        },
        float: {
          "0%, 100%": { transform: "translateY(0px)" },
          "50%": { transform: "translateY(-6px)" },
        },
        radar: {
          "0%": { transform: "scale(0.95)", opacity: "0.8" },
          "50%": { transform: "scale(1.8)", opacity: "0.0" },
          "100%": { transform: "scale(0.95)", opacity: "0.0" },
        },
        borderGlow: {
          "0%, 100%": { borderColor: "rgba(0, 240, 255, 0.3)" },
          "50%": { borderColor: "rgba(139, 92, 246, 0.4)" },
        },
      },
      boxShadow: {
        glow: "0 0 35px -5px rgba(0, 240, 255, 0.35)",
        "glow-violet": "0 0 35px -5px rgba(139, 92, 246, 0.35)",
        "glow-emerald": "0 0 35px -5px rgba(16, 185, 129, 0.35)",
        "glow-amber": "0 0 35px -5px rgba(245, 158, 11, 0.35)",
        "glow-rose": "0 0 35px -5px rgba(244, 63, 94, 0.35)",
        glass: "0 8px 32px 0 rgba(0, 0, 0, 0.5)",
        "glass-elevated": "0 20px 48px -8px rgba(0, 0, 0, 0.75), 0 0 1px 1px rgba(255, 255, 255, 0.08)",
        "inner-sheen": "inset 0 1px 1px 0 rgba(255, 255, 255, 0.12)",
      },
    },
  },
  plugins: [],
};

export default config;
