// The template AI-prompt builder (template-prompt slice): the copy/paste prompt must carry the engine
// contract, the REAL field names, and exactly the chosen data sample — the whole point is that the
// model designs against the user's actual shape.

import { describe, expect, it } from "vitest";

import { buildTemplatePrompt } from "./templatePrompt";

const rows = Array.from({ length: 40 }, (_, i) => ({
  time: 1751500000 + i,
  point_id: i % 2 ? "meter-001-kwh" : "meter-002-kwh",
  value: i * 1.5,
  rn: i + 1,
}));

describe("buildTemplatePrompt", () => {
  it("embeds the engine contract, the field names, and the sample size asked for", () => {
    const p = buildTemplatePrompt(rows, 10);
    expect(p).toContain("{{#each rows}}");
    expect(p).toContain("NO JavaScript");
    expect(p).toContain("hsl(var(--accent))");
    expect(p).toContain("Fields: time, point_id, value, rn");
    expect(p).toContain("Total rows at author time: 40");
    expect(p).toContain("Sample (10 rows)");
    // Exactly 10 sampled rows: the 10th value present, the 11th absent.
    expect(p).toContain(`"rn": 10`);
    expect(p).not.toContain(`"rn": 11`);
  });

  it('"all" embeds every row', () => {
    const p = buildTemplatePrompt(rows, "all");
    expect(p).toContain("Sample (40 rows)");
    expect(p).toContain(`"rn": 40`);
  });

  it("embeds the SQL + datasource when the target carries one", () => {
    const p = buildTemplatePrompt(rows, 10, {
      tool: "federation.query",
      source: "demo-buildings",
      sql: "SELECT * FROM point_reading LIMIT 100",
    });
    expect(p).toContain("Datasource: demo-buildings (via federation.query)");
    expect(p).toContain("SELECT * FROM point_reading LIMIT 100");
    expect(p).toContain("```sql");
  });

  it("names a structured (non-SQL) read honestly", () => {
    const p = buildTemplatePrompt(rows, 10, { tool: "series.read" });
    expect(p).toContain("Tool: series.read");
    expect(p).toContain("no SQL — a structured read");
  });

  it("stays honest with zero rows (query not run yet)", () => {
    const p = buildTemplatePrompt([], 10);
    expect(p).toContain("(none yet — run the query first)");
    expect(p).toContain("Sample (0 rows)");
  });
});
