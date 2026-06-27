import type { Config } from "tailwindcss";

// Mirrors the shell's palette so the federated page looks native: tokens are CSS variables (see
// src/styles/tokens.css) — near-black dark control surface, hairline borders, ONE warm amber accent.
// A theme is a `.dark` class swap, not a rebuild.
export default {
  darkMode: ["class"],
  content: ["./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: "hsl(var(--bg))",
        panel: "hsl(var(--panel))",
        border: "hsl(var(--border))",
        fg: "hsl(var(--fg))",
        muted: "hsl(var(--muted))",
        accent: "hsl(var(--accent))",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
} satisfies Config;
