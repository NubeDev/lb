/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import federation from "@originjs/vite-plugin-federation";
import path from "node:path";

// Module Federation REMOTE (ui-federation scope). This is the FRONTEND half of the `fleet-monitor`
// extension. It exposes exactly one module, `./mount`, and declares `react`/`react-dom` as SHARED
// singletons so the remote renders against the shell HOST's SAME React — no second copy bundled.
// The federation `filename` is `remoteEntry.js`, so the served container is `dist/assets/remoteEntry.js`
// — the path the manifest's `[ui] entry = "assets/remoteEntry.js"` and the shell's loader expect.
export default defineConfig({
  plugins: [
    react(),
    federation({
      name: "fleet_monitor",
      filename: "remoteEntry.js",
      exposes: { "./mount": "./src/mount.tsx" },
      shared: ["react", "react-dom"],
    }),
  ],
  // Federation container init uses top-level await; esnext is required (matches the shell host).
  build: { target: "esnext" },
  resolve: {
    alias: { "@": path.resolve(__dirname, "src") },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
});
