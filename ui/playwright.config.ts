import { defineConfig, devices } from "@playwright/test";

// Playwright E2E for extension pages (ui-federation scope). These run against the BUILT shell served
// by `make ui-preview` (port 4173) talking to a REAL node on :8080 — the federation bugs (two Reacts,
// "Invalid hook call") only surface in a real browser, so a unit test cannot stand in for this. The
// servers are started out-of-band (the node is already up; `make ui-preview` serves the build), so no
// `webServer` block here — the spec asserts they are reachable and skips with a clear message if not.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173",
    headless: true,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
