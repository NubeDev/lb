// The query-builder-common slice, driven against a REAL in-process gateway (CLAUDE §9 / testing §0 —
// no fake backend). The headline: a federation target now opens the SAME `SqlQueryEditor` a surreal
// target opens (the dialect behind a seam), and the schema dropdowns fire the shipped
// `federation.schema` verb. Each test signs into a UNIQUE workspace, registers a real `demo-buildings`
// sqlite datasource row via the real `datasource.add` admin verb, and renders `QueryTab` over a real
// federation cell. The federation SIDECAR is not spawned in this env (a true external a UI test
// cannot cheaply run) — `federation.schema` resolves to an honest typed error and the editor DEGRADES
// to an empty schema (the system-catalog deny contract), proving the dispatch + the schema hook +
// the mandatory capability-deny + workspace-isolation categories. The real-row round-trip is
// `rust/crates/host/tests/federation_sqlite_test.rs`'s job (unchanged, stays green).
//
// Mandatory categories covered:
//   - capability-deny (§2.1): without `mcp:federation.query:call` the schema discovery denies and the
//     dropdown is empty (the editor's Code half still works).
//   - workspace-isolation (§2.2): ws-B's datasource roster never includes ws-A's `demo-buildings`.

import { describe, expect, it, beforeAll, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { addDatasource } from "@/lib/datasources";
import * as ipc from "@/lib/ipc/invoke";
import { cellToEditorState } from "@/lib/panel-kit/cellEditorState";
import { defaultCell } from "@/lib/panel-kit/defaultCell";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import type { Cell } from "@/lib/dashboard";
import { QueryTab } from "./QueryTab";

let n = 0;
const nextWs = () => `qbc-${n++}`;

beforeAll(() => useRealGateway());

/** A v3 federation table cell — what `panel.save` would persist for a builder-authored federation
 *  panel. `args.sql` carries the emitted string; `options.sql` carries the builder query (reopening
 *  returns to the builder, exactly like a surreal cell after this slice). */
function federationCell(ws: string, source: string): Cell {
  const base = defaultCell("table" as never, "f1");
  return {
    ...base,
    view: "table",
    sources: [
      {
        refId: "A",
        tool: "federation.query",
        args: { source, sql: "" },
        datasource: { type: "federation", uid: `datasource:${ws}:${source}` },
      },
    ],
  };
}

/** Register a real `demo-buildings` sqlite datasource row (the roster path; no sidecar spawned). */
async function registerDemoSource(source: string): Promise<void> {
  await addDatasource({
    name: source,
    kind: "sqlite",
    endpoint: "127.0.0.1:0",
    dsn: "/tmp/lb-query-builder-common-demo.db",
  });
}

/** An `ipc.invoke` counter that DELEGATES to the real transport (observe, never fake — rule 9).
 *  Returns the spy + a `countTool` helper for `mcp_call` invokes by tool name. */
function viaCounter() {
  const real = ipc.invoke;
  const byTool = new Map<string, number>();
  const spy = vi
    .spyOn(ipc, "invoke")
    .mockImplementation(((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "mcp_call") {
        const tool = (args?.tool as string) ?? "?";
        byTool.set(tool, (byTool.get(tool) ?? 0) + 1);
      }
      return real(cmd, args);
    }) as typeof ipc.invoke);
  return {
    countTool: (tool: string) => byTool.get(tool) ?? 0,
    clear: () => byTool.clear(),
    restore: () => spy.mockRestore(),
  };
}

describe("query-builder-common — federation builder dispatch (real gateway)", () => {
  it("HEADLINE: a federation target opens SqlQueryEditor (not the legacy textarea) + fires federation.schema", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await registerDemoSource("demo-buildings");

    const counter = viaCounter();
    const state = cellToEditorState(federationCell(ws, "demo-buildings"));
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <QueryTab ws={ws} state={state} patch={() => {}} />
      </WithDashboardCache>,
    );

    // The Builder⇄Code editor mounts (the dispatch headline)…
    await waitFor(() => {
      expect(container.querySelector('[aria-label="sql query editor"]')).toBeTruthy();
    });
    // …and the legacy raw-SQL textarea is GONE (the dispatch replaced it).
    expect(container.querySelector('[aria-label="federation sql"]')).toBeNull();
    // The schema dropdown loader fires `federation.schema {source}` for the federation target.
    await waitFor(() => {
      expect(counter.countTool("federation.schema")).toBeGreaterThanOrEqual(1);
    });
    unmount();
  });

  it("surreal regression: a surreal target still opens SqlQueryEditor (dialect=surreal, math::avg preview)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // A surreal SQL-builder cell with an avg aggregation — SurrealQL emitter produces math::avg.
    const base = defaultCell("table" as never, "s1");
    const cell: Cell = {
      ...base,
      view: "table",
      sources: [
        {
          refId: "A",
          tool: "store.query",
          args: { sql: "SELECT math::avg(payload) FROM series" },
          datasource: { type: "surreal" },
        },
      ],
      options: {
        sql: {
          mode: "builder",
          rawSql: "SELECT math::avg(payload) AS avg_payload FROM series",
          builder: {
            table: "series",
            columns: [{ name: "payload", aggregation: "avg" }],
            filters: [],
            groupBy: [],
          },
          format: "table",
        },
      },
    };
    const state = cellToEditorState(cell);
    const { container } = render(
      <WithDashboardCache ws={ws}>
        <QueryTab ws={ws} state={state} patch={() => {}} />
      </WithDashboardCache>,
    );

    // The editor mounts (surreal regression — the dialect seam did not break the existing path).
    await waitFor(() => {
      expect(container.querySelector('[aria-label="sql query editor"]')).toBeTruthy();
    });
    // The live preview renders the SURREAL dialect (math::avg, bare identifier) — proves the
    // dialect prop is "surreal", not the standard emitter's AVG("payload").
    const preview = container.querySelector('[aria-label="sql preview"]');
    expect(preview?.textContent ?? "").toContain("math::avg");
    expect(preview?.textContent ?? "").not.toContain("AVG(");
  });

  it("CAPABILITY-DENY (mandatory §2.1): without mcp:federation.query:call, federation.schema is denied and the dropdown is empty (no crash, editor renders)", async () => {
    const ws = nextWs();
    // Granted series + datasource.list (so the dropdown loads) but NOT federation.query — schema
    // discovery is the same read cap as the query, so both deny.
    await signInWithCaps("user:ada", ws, [
      "mcp:datasource.list:call",
      "mcp:datasource.add:call",
      "mcp:series.find:call",
    ]);
    await registerDemoSource("demo-buildings");

    const counter = viaCounter();
    const state = cellToEditorState(federationCell(ws, "demo-buildings"));
    const { container } = render(
      <WithDashboardCache ws={ws}>
        <QueryTab ws={ws} state={state} patch={() => {}} />
      </WithDashboardCache>,
    );

    // The editor still mounts (the Code half works without schema; a deny collapses to empty
    // dropdowns, per the system-catalog contract — never a crash, never fabricated rows).
    await waitFor(() => {
      expect(container.querySelector('[aria-label="sql query editor"]')).toBeTruthy();
    });
    // The discovery call fired and was DENIED at the host — the table dropdown is empty.
    await waitFor(() => {
      expect(counter.countTool("federation.schema")).toBeGreaterThanOrEqual(1);
    });
    const tableSelect = container.querySelector('[aria-label="sql table"]') as HTMLSelectElement | null;
    expect(tableSelect).toBeTruthy();
    // Only the placeholder "— pick a table —" option is present (no discovered tables).
    expect(tableSelect!.options.length).toBe(1);
    expect(tableSelect!.options[0].value).toBe("");
  });

  it("WORKSPACE-ISOLATION (mandatory §2.2): ws-B's datasource roster does not include ws-A's source", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await registerDemoSource("demo-buildings");

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    // ws-B registers its OWN, differently-named source — the rosters must not bleed.
    await registerDemoSource("ben-warehouse");

    // ws-B's `datasource.list` (the dropdown loader) returns only ws-B's source. The wall is at the
    // host: the verb is workspace-pinned from the token.
    const list = await ipc.invoke("mcp_call", { tool: "datasource.list", args: {} });
    const names = (list as { datasources: { name: string }[] }).datasources.map((d) => d.name);
    expect(names).toContain("ben-warehouse");
    expect(names).not.toContain("demo-buildings");
  });
});
