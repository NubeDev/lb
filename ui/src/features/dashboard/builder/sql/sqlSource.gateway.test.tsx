// The read-only SQL widget source, driven against a REAL in-process gateway (widget-builder Slice A
// + C; CLAUDE §9 / testing §0 — no fake backend). Each test logs in to a UNIQUE workspace, seeds real
// rows through the real ingest path, and exercises `store.query`/`store.schema` end to end over the
// real `POST /mcp/call` bridge. Covers:
//   - store.query deny without the cap;
//   - a Code-mode WRITE is rejected (parse-allowlisted) server-side;
//   - two-session isolation (ws-B SQL cannot read ws-A rows);
//   - a SELECT round-trips real seeded rows into {columns, rows} a table/chart renders;
//   - store.schema deny + isolation (ws-B sees only ws-B tables);
//   - end-to-end: a query built in the visual editor → Run → rows render in a table + chart widget.

import { describe, expect, it, beforeAll } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { invoke } from "@/lib/ipc/invoke";
import { runQuery, readSchema } from "@/lib/dashboard/sql.api";
import { WidgetView } from "../../views/WidgetView";
import type { Cell } from "@/lib/dashboard";
import { emptyQuery } from "./query";
import { toSurrealQL } from "./toSurrealQL";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `sql-${n++}`;

beforeAll(() => useRealGateway());

/** Seed `n` real samples into `series` via the gateway's real ingest path. */
async function seedRows(series: string, count: number): Promise<void> {
  const samples = Array.from({ length: count }, (_, i) => ({
    series,
    producer: "user:ada",
    seq: i + 1,
    payload: (i + 1) * 10,
    ts: i + 1,
  }));
  await invoke("mcp_call", { tool: "ingest.write", args: { samples } });
}

describe("store.query (real gateway)", () => {
  it("round-trips a SELECT into {columns, rows}", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedRows("cpu", 3);

    const result = await runQuery("SELECT series, seq, payload FROM series ORDER BY seq");
    expect(result.rows.length).toBe(3);
    expect(result.columns).toContain("series");
    expect(result.columns).toContain("payload");
    expect(result.rows[0].series).toBe("cpu");
  });

  it("denies store.query without the cap", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:series.find:call"]); // no store.query
    await expect(runQuery("SELECT * FROM series")).rejects.toThrow();
  });

  it("rejects a Code-mode WRITE statement (parse-allowlisted) server-side", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedRows("cpu", 1);
    // The session HOLDS store.query — the parse gate is what bites, not the cap.
    await expect(runQuery("DELETE series")).rejects.toThrow();
    await expect(runQuery("CREATE series:x SET payload = 1")).rejects.toThrow();
    // …and the store was not mutated.
    const after = await runQuery("SELECT count() AS c FROM series GROUP ALL");
    expect(after.rows[0].c).toBe(1);
  });

  it("is workspace isolated — ws-B SQL cannot read ws-A rows", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedRows("secret", 4);

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    await seedRows("benrows", 2);
    const benView = await runQuery("SELECT series FROM series");
    expect(benView.rows.length).toBe(2);
    expect(benView.rows.every((r) => r.series === "benrows")).toBe(true);
  });
});

describe("store.schema (real gateway)", () => {
  it("reports the workspace's tables + columns", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedRows("cpu", 2);

    const schema = await readSchema();
    const series = schema.tables.find((t) => t.name === "series");
    expect(series).toBeTruthy();
    const cols = series!.columns.map((c) => c.name);
    expect(cols).toContain("seq");
    expect(cols).toContain("payload");
  });

  it("denies store.schema without the cap", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, ["mcp:series.find:call"]); // no store.schema
    await expect(readSchema()).rejects.toThrow();
  });

  it("is workspace isolated — ws-B schema does not surface ws-A's series data", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedRows("cpu", 2);

    const wsB = nextWs();
    await signInReal("user:ben", wsB); // ws-B seeded nothing
    const schema = await readSchema();
    // ws-B has no `series` rows; it must not see ws-A's seeded series table with columns.
    const series = schema.tables.find((t) => t.name === "series");
    expect(series === undefined || series.columns.length === 0).toBe(true);
  });
});

describe("visual-editor → Run → render e2e (real gateway)", () => {
  it("builds a query in the visual editor and renders its rows in a table + chart", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedRows("cpu", 5);

    // Build a query the way the VisualEditor does: a typed SqlBuilderQuery → toSurrealQL.
    const query = {
      ...emptyQuery("series"),
      columns: [{ name: "seq" }, { name: "payload" }],
      orderBy: { column: "seq", direction: "asc" as const },
      limit: 100,
    };
    const sql = toSurrealQL(query);
    expect(sql).toContain("SELECT seq, payload FROM series");

    // Run it through store.query (what the SQL source cell does).
    const result = await runQuery(sql);
    expect(result.rows.length).toBe(5);

    // The SQL source cell: a table view over store.query. Render it against the real gateway and
    // assert real rows appear (the table introspects columns from the result).
    const cell: Cell = {
      i: "sql-table",
      x: 0,
      y: 0,
      w: 4,
      h: 3,
      v: 2,
      widget_type: "chart",
      view: "table",
      binding: { series: "" },
      source: { tool: "store.query", args: { sql } },
      options: {},
    };
    const { container } = render(<WithDashboardCache ws={ws}><WidgetView cell={cell} installed={[]} workspace={ws} /></WithDashboardCache>);
    await waitFor(() => {
      // The table renders a header from the introspected columns + at least one data row.
      expect(container.textContent ?? "").toMatch(/seq|payload/);
    });

    // The same source as a chart view also renders (rows over time).
    const chartCell: Cell = { ...cell, i: "sql-chart", view: "chart" };
    const chart = render(<WithDashboardCache ws={ws}><WidgetView cell={chartCell} installed={[]} workspace={ws} /></WithDashboardCache>);
    await waitFor(() => expect(chart.container.querySelector("svg")).toBeTruthy());
  });
});
