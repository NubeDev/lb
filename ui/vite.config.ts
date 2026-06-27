import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// Vite + React. Tauri serves this build in the desktop shell; the same build is served to browsers via
// the SSE gateway at S3. The `@` alias mirrors the src root (FILE-LAYOUT).
//
// Extension federation (ui-federation scope): the shell publishes its `react`/`react-dom`/
// `react-dom/client`/`react/jsx-runtime` as SINGLETONS on `globalThis.__lb*` (src/features/ext-host/
// singletons.ts) and declares an import map in `index.html` that maps those bare specifiers to the
// `/shims/*.mjs` re-exporters. An extension `remoteEntry.js` — built as an ESM lib with those modules
// externalised — is dynamic-imported at runtime by gateway URL (ext-host/federation.ts) and renders
// in-process against the host's SINGLE React. No build-time federation plugin is needed; this replaces
// `@originjs/vite-plugin-federation`, whose dynamic-remote share scope shipped a second React and broke
// hooks ("Invalid hook call"). See debugging/extensions/federated-remote-fails-in-dev-server.md.
export default defineConfig(({ command }) => {
  const nodeEnv = JSON.stringify(command === "build" ? "production" : "development");

  return {
    plugins: [react()],
    define: {
      "process.env.NODE_ENV": nodeEnv,
    },
    optimizeDeps: {
      esbuildOptions: {
        define: {
          "process.env.NODE_ENV": nodeEnv,
        },
      },
    },
    // esnext: extension remotes may use top-level await; keep the host build modern to match.
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
      // `vitest.gateway.config.ts` (`pnpm test:gateway`), not this default suite.
      include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
      exclude: ["**/node_modules/**", "e2e/**", "**/*.gateway.test.ts", "**/*.gateway.test.tsx"],
    },
  };
});
