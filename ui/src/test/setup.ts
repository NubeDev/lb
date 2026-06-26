// Vitest setup: jest-dom matchers + reset the in-memory node fake between tests so each test
// starts from an empty store (the fake is module-global by design).
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";

import { __resetFake } from "@/lib/ipc/fake";

afterEach(() => {
  __resetFake();
});
