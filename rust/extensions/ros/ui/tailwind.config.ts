import type { Config } from "tailwindcss";
import { extTailwindPreset } from "@nube/ext-ui-sdk/tailwind";

// Preflight off + utilities scoped under [data-ext-root] (docs/extensions/README.md §3a) — the host
// owns the theme, this extension only consumes host tokens through the cascade. No :root{}/.dark{}
// blocks here, ever.
export default {
  presets: [extTailwindPreset()],
  content: ["./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: { accent: "hsl(var(--accent))" },
    },
  },
} satisfies Config;
