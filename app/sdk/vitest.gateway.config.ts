// The real-gateway Vitest project for the app sdk — the exact `ui/ pnpm test:gateway` pattern
// (rule 9: no fakes). `globalSetup` builds + spawns the REAL `test_gateway` bin and provides its
// URL; every test constructs a `GatewayClient` against it and drives real routes with real signed
// tokens. Node environment (the client is platform-free — no DOM, no RN runtime needed here).

import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/**/*.gateway.test.ts"],
    globalSetup: ["./tests/real-gateway.ts"],
    // One real backend; keep it serial so seeds don't interleave across workspaces under test.
    pool: "threads",
    poolOptions: { threads: { singleThread: true } },
    testTimeout: 20_000,
  },
});
