/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// Build the echarts-panel UI as a single ESM `remoteEntry.js` the shell dynamic-imports (ui-federation
// scope) — the rubix-cube import-map pattern, NOT `@originjs/vite-plugin-federation`. The crux is
// `external`: `react`/`react-dom`/`react-dom/client`/`react/jsx-runtime` are NOT bundled; their bare
// imports survive into the output and the shell's import map (index.html + /shims/*.mjs) resolves them
// to the host's SINGLE React. So the tile renders in-process against the SAME React — no second copy,
// no "Invalid hook call". `echarts` is DELIBERATELY NOT externalised: it is bundled into this remote
// (the shell does not ship it), so the chart tile is self-contained. The bundle ships its own page/tile
// logic + echarts + compiled Tailwind CSS, which `remoteEntry.ts` injects at runtime via a `?inline`
// import (cssCodeSplit off → one CSS string).
//
// Output is `dist/remoteEntry.js` (lib `fileName`); `make publish-ext` copies `dist/*` into the node's
// `extensions-ui/echarts-panel/`, so the shell loads it at `/extensions/echarts-panel/ui/remoteEntry.js`
// — the path the manifest's `[ui] entry = "remoteEntry.js"` names.
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
      // React is externalised (shared with the host); echarts is NOT — it is bundled into the remote.
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
