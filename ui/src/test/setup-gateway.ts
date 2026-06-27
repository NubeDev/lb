// Per-file Vitest setup for the real-gateway tests. Unlike the fake-backed suite's `setup.ts`, there
// are NO fakes to reset here — the backend is a real spawned node. We only load the jest-dom matchers
// and clear the session after each test (the next test logs in fresh against the real gateway).

import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

import { setSession } from "@/lib/session/session.store";

// react-flow (the Data page's graph view) measures the DOM with APIs jsdom doesn't implement. Polyfill
// the minimum it needs so the graph mounts under test (no real layout — we assert it renders).
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = globalThis.ResizeObserver ?? (ResizeObserverStub as never);
window.scrollTo = vi.fn();
if (!("DOMMatrixReadOnly" in globalThis)) {
  (globalThis as Record<string, unknown>).DOMMatrixReadOnly = class {
    m22 = 1;
    constructor() {}
  };
}
// react-flow reads element bounds; jsdom returns zeros, which is fine for a render assertion.
Element.prototype.getBoundingClientRect =
  Element.prototype.getBoundingClientRect ??
  (vi.fn(() => ({ x: 0, y: 0, width: 800, height: 600, top: 0, left: 0, right: 0, bottom: 0 })) as never);

beforeEach(() => {
  window.history.replaceState(null, "", "/#/channels?c=general");
});

afterEach(() => {
  // Unmount any rendered tree (the default suite gets this from vitest's auto-cleanup via globals; we
  // do it explicitly here so a prior test's DataView doesn't linger and double the picker entries).
  cleanup();
  setSession(null);
});
