// Vitest setup for the DEFAULT suite (pure component/hook/logic tests — no backend). The in-memory
// `*.fake.ts` node fakes are DELETED (CLAUDE §9): tests that need a backend run against a real spawned
// node under `vitest.gateway.config.ts` (see `src/test/setup-gateway.ts`). This suite only loads the
// jest-dom matchers and clears the session between tests.
// jest-dom matchers. We DON'T use the `@testing-library/jest-dom/vitest` entry: under Vite/Vitest
// its internal `import { expect } from "vitest"` resolves to a different `expect` instance than the
// one the test runner hands each file, so its `expect.extend` silently no-ops and every
// `toBeInTheDocument()`/`toBeDisabled()` throws "Invalid Chai property". Importing the matchers
// directly and extending the `expect` WE import binds them to the runner's real instance.
import * as jestDomMatchers from "@testing-library/jest-dom/matchers";
import { afterEach, expect, vi } from "vitest";

expect.extend(jestDomMatchers);

import { setSession } from "@/lib/session/session.store";

window.scrollTo = vi.fn();

afterEach(() => {
  setSession(null);
});
