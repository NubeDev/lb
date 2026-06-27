/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// Build the proof-panel UI as a single ESM `remoteEntry.js` the shell dynamic-imports (ui-federation
// scope) — the rubix-cube import-map pattern, NOT `@originjs/vite-plugin-federation`. The crux is
// `external`: `react`/`react-dom`/`react-dom/client`/`react/jsx-runtime` are NOT bundled; their bare
// imports survive into the output and the shell's import map (index.html + /shims/*.mjs) resolves them
// to the host's SINGLE React. So the page renders in-process against the SAME React — no second copy,
// no "Invalid hook call". The bundle ships only its own page logic + compiled Tailwind CSS, which
// `remoteEntry.ts` injects at runtime via a `?inline` import (cssCodeSplit off → one CSS string).
//
// Output is `dist/remoteEntry.js` (lib `fileName`); `make publish-ext` copies `dist/*` into the node's
// `extensions-ui/proof-panel/`, so the shell loads it at `/extensions/proof-panel/ui/remoteEntry.js` —
// the path the manifest's `[ui] entry = "remoteEntry.js"` names.
export default defineConfig({
  plugins: [react()],
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
  resolve: {
    alias: { "@": path.resolve(__dirname, "src") },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
});
