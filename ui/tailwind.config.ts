import type { Config } from "tailwindcss";
import tailwindcssAnimate from "tailwindcss-animate";

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
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: "hsl(var(--primary))",
        "primary-foreground": "hsl(var(--primary-foreground))",
        secondary: "hsl(var(--secondary))",
        "secondary-foreground": "hsl(var(--secondary-foreground))",
        "muted-foreground": "hsl(var(--muted-foreground))",
        "accent-foreground": "hsl(var(--accent-foreground))",
        destructive: "hsl(var(--destructive))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        sidebar: "hsl(var(--sidebar))",
        "sidebar-foreground": "hsl(var(--sidebar-foreground))",
        "sidebar-primary": "hsl(var(--sidebar-primary))",
        "sidebar-primary-foreground": "hsl(var(--sidebar-primary-foreground))",
        "sidebar-accent": "hsl(var(--sidebar-accent))",
        "sidebar-accent-foreground": "hsl(var(--sidebar-accent-foreground))",
        "sidebar-border": "hsl(var(--sidebar-border))",
        "sidebar-ring": "hsl(var(--sidebar-ring))",
      },
    },
  },
  plugins: [tailwindcssAnimate],
} satisfies Config;
