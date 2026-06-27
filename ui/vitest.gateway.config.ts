// A second Vitest project for the **real-gateway** tests (data-console scope; the first step of
// retiring the `*.fake.ts` backend — CLAUDE §9, testing §0). It is separate from the default config
// (`vite.config.ts`, which still drives the legacy fake-backed suite) so the migration is incremental:
// only `*.gateway.test.tsx` files run here, against a REAL spawned node — no fake in sight.
//
// `globalSetup` spawns the `test_gateway` bin and provides its URL; the tests stub
// `VITE_GATEWAY_URL` to it so `invoke` takes the real HTTP path. Single-threaded so the shared real
// server isn't hit by parallel workers racing on one workspace's rows.

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@": path.resolve(__dirname, "src") } },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.gateway.test.tsx"],
    setupFiles: ["./src/test/setup-gateway.ts"],
    globalSetup: ["./src/test/real-gateway.ts"],
    // One real backend, one workspace per test — keep it serial so seeds don't interleave.
    pool: "threads",
    poolOptions: { threads: { singleThread: true } },
    testTimeout: 20_000,
  },
});
