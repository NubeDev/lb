// Pure unit tests for the keystroke parser (channels-command-palette scope) — NO network. Asserts
// mode classification, fuzzy ranking with a pre-selected best, and the `@`-token detection that
// drives chip insert/delete. The parser is the heart of the palette UX; it must be deterministic.

import { describe, expect, it } from "vitest";

import { parsePalette, fuzzyScore } from "./parsePalette";
import type { ToolDescriptor } from "@/lib/channel/palette.types";
import type { Candidate } from "./parsePalette";

const TOOLS: ToolDescriptor[] = [
  { name: "federation.query", title: "query", group: "federation" },
  { name: "agent.ask", title: "ask", group: "agent" },
  { name: "rules.eval", title: "eval", group: "rules" },
];

const SOURCES: Candidate[] = [
  { value: "warehouse", label: "warehouse" },
  { value: "analytics", label: "analytics" },
  { value: "wallet", label: "wallet" },
];

describe("parsePalette — mode classification", () => {
  it("plain text is chat mode (no palette)", () => {
    const p = parsePalette("hello world", TOOLS);
    expect(p.mode).toBe("chat");
    expect(p.candidates).toHaveLength(0);
    expect(p.selected).toBe(-1);
  });

  it("a leading slash is command mode", () => {
    const p = parsePalette("/", TOOLS);
    expect(p.mode).toBe("command");
    expect(p.candidates).toHaveLength(3); // all tools, catalog order
    expect(p.selected).toBe(0); // best pre-selected
  });

  it("an @ token (anywhere) is mention mode", () => {
    const p = parsePalette("/query @wa", TOOLS, SOURCES);
    expect(p.mode).toBe("mention");
    expect(p.query).toBe("wa");
  });

  it("a completed @chip (trailing space) reclassifies away from mention", () => {
    // Once the source is chosen and a space typed, the `@` token is no longer active.
    const p = parsePalette("/query @warehouse ", TOOLS, SOURCES);
    expect(p.mode).toBe("command"); // back to the command text (leading slash)
  });
});

describe("parsePalette — fuzzy ranking", () => {
  it("ranks the closest tool first and pre-selects it", () => {
    const p = parsePalette("/que", TOOLS);
    expect(p.mode).toBe("command");
    expect(p.candidates[0].value).toBe("federation.query");
    expect(p.selected).toBe(0);
  });

  it("filters out non-matches", () => {
    const p = parsePalette("/zzz", TOOLS);
    expect(p.candidates).toHaveLength(0);
    expect(p.selected).toBe(-1);
  });

  it("ranks mention candidates by the @ query, prefix wins", () => {
    const p = parsePalette("/query @wa", TOOLS, SOURCES);
    // both "warehouse" and "wallet" match "wa"; both prefix — order is stable by score then input.
    const values = p.candidates.map((c) => c.value);
    expect(values).toContain("warehouse");
    expect(values).toContain("wallet");
    expect(values).not.toContain("analytics");
  });

  it("a contiguous prefix outscores a scattered subsequence", () => {
    // "war" is a prefix of warehouse, a scattered subsequence of "wallet redo" (n/a) — score lower.
    expect(fuzzyScore("war", "warehouse")! < fuzzyScore("war", "answer party")!).toBe(true);
  });

  it("an empty query preserves catalog order (score 0 for all)", () => {
    const p = parsePalette("/", TOOLS);
    expect(p.candidates.map((c) => c.value)).toEqual([
      "federation.query",
      "agent.ask",
      "rules.eval",
    ]);
  });
});

describe("fuzzyScore", () => {
  it("returns null when the query is not a subsequence", () => {
    expect(fuzzyScore("xyz", "warehouse")).toBeNull();
  });
  it("an empty query matches anything with score 0", () => {
    expect(fuzzyScore("", "anything")).toBe(0);
  });
});
