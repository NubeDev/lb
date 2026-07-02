// Unit test for the shared table column-model (widget-kit scope) — proves the model resolves headers
// through the ONE presentation resolver (humanize fallback + label override), DROPS hidden columns, and
// reorders ONLY when `order` is declared (absent → first-seen order preserved). Pure logic.

import { describe, expect, it } from "vitest";

import type { FieldConfig } from "@/lib/dashboard";
import { columnsOf, resolveColumns, cellText } from "./columns";

const rows = [
  { maxRuns: 3, principalSub: "user:me", action: { kind: "channel-post" }, schedule: "0 8 * * 1" },
];

describe("columnsOf", () => {
  it("unions keys in first-seen order", () => {
    expect(columnsOf(rows)).toEqual(["maxRuns", "principalSub", "action", "schedule"]);
  });
});

describe("resolveColumns (headers, hide, order — one resolver)", () => {
  it("humanizes headers when no fieldConfig override is declared", () => {
    const cols = resolveColumns(rows);
    expect(cols.map((c) => c.header)).toEqual(["Max Runs", "Principal Sub", "Action", "Schedule"]);
  });

  it("uses a `byName` displayName override over humanize, and carries description", () => {
    const fc: FieldConfig = {
      defaults: {},
      overrides: [
        {
          matcher: { id: "byName", options: "action" },
          properties: [
            { id: "displayName", value: "Action" },
            { id: "description", value: "What fires" },
          ],
        },
      ],
    };
    const action = resolveColumns(rows, fc).find((c) => c.key === "action")!;
    expect(action.header).toBe("Action");
    expect(action.description).toBe("What fires");
  });

  it("drops a `hide`-marked column from the rendered set", () => {
    const fc: FieldConfig = {
      defaults: {},
      overrides: [
        { matcher: { id: "byName", options: "principalSub" }, properties: [{ id: "hide", value: true }] },
      ],
    };
    const keys = resolveColumns(rows, fc).map((c) => c.key);
    expect(keys).not.toContain("principalSub");
    expect(keys).toContain("maxRuns"); // a non-hidden column stays
  });

  it("keeps first-seen order when no `order` is declared (never reorders implicitly)", () => {
    expect(resolveColumns(rows).map((c) => c.key)).toEqual(["maxRuns", "principalSub", "action", "schedule"]);
  });

  it("reorders only the columns that declare `order` (ascending), others keep position after them", () => {
    const fc: FieldConfig = {
      defaults: {},
      overrides: [
        { matcher: { id: "byName", options: "schedule" }, properties: [{ id: "order", value: 1 }] },
        { matcher: { id: "byName", options: "maxRuns" }, properties: [{ id: "order", value: 2 }] },
      ],
    };
    // schedule(1), maxRuns(2) sort first; principalSub/action keep first-seen order after.
    expect(resolveColumns(rows, fc).map((c) => c.key)).toEqual([
      "schedule",
      "maxRuns",
      "principalSub",
      "action",
    ]);
  });
});

describe("cellText (nested value rendering)", () => {
  it("renders a nested object as JSON (a labeled cell, not a thrown blob)", () => {
    expect(cellText({ kind: "channel-post", channel: "team" })).toBe('{"kind":"channel-post","channel":"team"}');
  });
  it("renders null/undefined as empty and scalars verbatim", () => {
    expect(cellText(null)).toBe("");
    expect(cellText(undefined)).toBe("");
    expect(cellText(3)).toBe("3");
    expect(cellText("x")).toBe("x");
  });
});
