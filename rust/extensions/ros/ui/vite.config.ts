/// <reference types="vitest" />
import { defineConfig } from "vite";
import { defineExtConfig } from "@nube/ext-ui-sdk/vite";
import path from "node:path";

// The Vite config is just the SDK preset (docs/extensions/README.md §3a) — lib-mode ESM
// `remoteEntry.js`, React externalised so the remote resolves through the shell's single React.
export default defineConfig({
  ...defineExtConfig({ entry: "src/remoteEntry.tsx" }),
  resolve: {
    alias: { "@": path.resolve(__dirname, "src") },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
});
