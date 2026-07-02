// Unit test for the humanize fallback (widget-kit scope) — the ONLY non-author-declared label source.
// Pure logic, no IO. Covers the scope's named cases (`maxRuns` → "Max Runs", `nextAttemptTs` → "Next
// Attempt Ts") plus snake/kebab/acronym boundaries.

import { describe, expect, it } from "vitest";

import { humanize } from "./humanize";

describe("humanize (the label fallback)", () => {
  it("title-cases camelCase names (the scope's headline cases)", () => {
    expect(humanize("maxRuns")).toBe("Max Runs");
    expect(humanize("nextAttemptTs")).toBe("Next Attempt Ts");
    expect(humanize("principalSub")).toBe("Principal Sub");
  });

  it("splits snake_case and kebab-case", () => {
    expect(humanize("action_kind")).toBe("Action Kind");
    expect(humanize("max-runs")).toBe("Max Runs");
  });

  it("keeps a single lowercase word title-cased", () => {
    expect(humanize("schedule")).toBe("Schedule");
    expect(humanize("id")).toBe("Id");
    expect(humanize("ts")).toBe("Ts");
  });

  it("splits an acronym from a following word", () => {
    expect(humanize("HTTPServer")).toBe("HTTP Server");
  });
});
