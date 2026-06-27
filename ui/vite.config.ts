import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import federation from "@originjs/vite-plugin-federation";
import path from "node:path";

// Vite + React. Tauri serves this build in the desktop shell; the same build is served to
// browsers via the SSE gateway at S3. The `@` alias mirrors the src root (FILE-LAYOUT).
//
// Module Federation HOST (ui-federation scope): the shell shares its `react`/`react-dom` as
// SINGLETONS so a federated extension remote (e.g. `fleet-monitor/ui`) renders in-process against
// the SAME React — no second copy, no hook-dispatcher mismatch, native-feeling. Remotes are loaded
// at RUNTIME by gateway URL (not known at build time), so we declare no static `remotes` here; the
// shell registers each remote dynamically via the federation runtime (see `ext-host/federation.ts`).
// `shared` must still be declared at build time for the host to expose its singletons to remotes.
export default defineConfig({
  plugins: [
    react(),
    federation({
      name: "shell",
      remotes: {},
      shared: ["react", "react-dom"],
    }),
  ],
  // Module Federation needs a modern target that supports top-level await (the remote container init).
  build: { target: "esnext" },
  resolve: {
    alias: { "@": path.resolve(__dirname, "src") },
  },
  // Tauri expects a fixed dev port and no clearing of the screen.
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // The real-gateway tests (`*.gateway.test.ts[x]`) need a spawned node; they run under their own
    // `vitest.gateway.config.ts` (`pnpm test:gateway`), not this fake-backed default suite.
    exclude: ["**/node_modules/**", "**/*.gateway.test.ts", "**/*.gateway.test.tsx"],
  },
});
