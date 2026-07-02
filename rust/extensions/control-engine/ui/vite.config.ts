/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// Build the control-engine UI as a single ESM `remoteEntry.js` the shell dynamic-imports (ui-federation
// scope) — the same import-map pattern proof-panel uses (NOT @originjs/vite-plugin-federation). The crux
// is `external`: react/react-dom/react-dom/client/react/jsx-runtime are NOT bundled; their bare imports
// survive into the output and the shell's import map resolves them to the host's SINGLE React. The page
// renders in-process against the SAME React — no second copy, no "Invalid hook call". The bundle ships
// its own page logic + the vendored CeEditor + compiled CSS, injected at runtime via a `?inline` import.
//
// `@nube/ce-wiresheet` resolution (S7 decision): the ext UI is a STANDALONE package (its own lockfile,
// built by build.sh — NOT a pnpm workspace member, matching proof-panel). So it resolves the vendored
// package by ALIAS to its built `dist/` (produced by `packages/ce-wiresheet`'s `pnpm build:lib`, which
// build.sh runs first). The alias points at the built ESM entry + its bundled stylesheet — no publish
// dance, no second React (react is external in both this build AND the ce-wiresheet lib build).
const CE_WIRESHEET_DIST = path.resolve(__dirname, "../../../../packages/ce-wiresheet/dist");

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
    alias: [
      { find: "@", replacement: path.resolve(__dirname, "src") },
      // The vendored editor's bundled stylesheet (exact match, incl. the `?inline` query) — MUST come
      // before the bare-package alias so it isn't swallowed by the prefix rule.
      { find: /^@nube\/ce-wiresheet\/style\.css(\?.*)?$/, replacement: path.join(CE_WIRESHEET_DIST, "ce-wiresheet.css") + "$1" },
      // The vendored editor entry (exact match on the bare package specifier).
      { find: /^@nube\/ce-wiresheet(\?.*)?$/, replacement: path.join(CE_WIRESHEET_DIST, "ce-wiresheet.js") + "$1" },
    ],
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // In unit tests the heavy vendored editor (xyflow/codemirror, needs the built dist) is replaced by a
    // TEST DOUBLE of the vendored component's SURFACE — the `CeEditor` prop shape + re-exported types.
    // This is a double of a UI COMPONENT, not of node/bridge behavior (rule 9): the bridge itself is
    // exercised for real against a stub bridge INTERFACE in bridge-transport.test.ts.
    alias: {
      "@nube/ce-wiresheet": path.resolve(__dirname, "src/test/ce-wiresheet.stub.tsx"),
    },
  },
});
