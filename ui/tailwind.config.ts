import type { Config } from "tailwindcss";

// Quiet control-surface palette (frontend scope): near-black dark, warm paper light, one
// warm amber accent, hairline borders. Tokens are CSS variables (see globals.css) so a
// theme is a class swap, not a rebuild.
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
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
  plugins: [],
} satisfies Config;
