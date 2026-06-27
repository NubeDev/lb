// Vitest setup for the DEFAULT suite (pure component/hook/logic tests — no backend). The in-memory
// `*.fake.ts` node fakes are DELETED (CLAUDE §9): tests that need a backend run against a real spawned
// node under `vitest.gateway.config.ts` (see `src/test/setup-gateway.ts`). This suite only loads the
// jest-dom matchers and clears the session between tests.
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";

import { setSession } from "@/lib/session/session.store";

afterEach(() => {
  setSession(null);
});
