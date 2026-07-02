import { defineConfig } from "vitest/config";

// Unit tests run in node; render tests opt into happy-dom per-file.
// dangerouslyIgnoreUnhandledErrors: drei <Text> (troika) fetches its fallback-font
// index from a CDN; offline/CI that fetch is aborted at env teardown and surfaces as
// an unhandled rejection AFTER all assertions passed. The real components still
// render (per-shape Suspense) — this only silences the teardown abort, not failures.
export default defineConfig({
  test: {
    dangerouslyIgnoreUnhandledErrors: true,
  },
});
