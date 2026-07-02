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
  // Replace `process.env.NODE_ENV` at build time. In a Vite LIB build (unlike an app build) Vite does
  // NOT inject this — and three.js / @react-three/fiber (bundled here, the federation payoff) read it
  // at module eval. Left unreplaced the browser throws `process is not defined` the moment the remote
  // loads, so the page never mounts (found live: "Could not load thecrew: process is not defined").
  // The shell app build defines it for its own graph; a federated remote must define its OWN.
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
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
