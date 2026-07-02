/// <reference types="vitest" />
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

// The federation-remote build (thecrew-extension-scope.md §UI lift): a single ESM `remoteEntry.js`
// the shell dynamic-imports (ui-federation scope), the proof-panel pattern. The crux is `external`:
// `react`/`react-dom`/`react-dom/client`/`react/jsx-runtime` are NOT bundled — their bare imports
// survive into the output and the shell's import map resolves them to the host's SINGLE React
// (no second copy, no "Invalid hook call"). three.js IS bundled — the federation payoff is that
// only this remote carries the ~1MB engine. Compiled Tailwind CSS is injected at runtime by
// `remoteEntry.ts` via a `?inline` import (cssCodeSplit off → one CSS string).
//
// `pnpm dev` still runs the standalone playground (index.html → main.tsx); the lib build below is
// what `build.sh` emits for publish.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    target: "esnext",
    cssCodeSplit: false,
    lib: {
      entry: "src/remoteEntry.ts",
      formats: ["es"],
      fileName: () => "remoteEntry.js",
    },
    rollupOptions: {
      external: ["react", "react-dom", "react-dom/client", "react/jsx-runtime"],
    },
  },
});
