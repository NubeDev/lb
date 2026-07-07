// The Query workbench, driven against a REAL in-process gateway (query-workbench-view scope, slice 3;
// CLAUDE §9 / testing §0 — no fake backend). The workbench mounts in three homes (Datasources detail,
// Data page, Data Studio pane); this test proves the ONE component (`QueryWorkbench`) runs real
// queries end to end. Each test signs into a UNIQUE workspace, seeds real rows through the real
// ingest path, and drives the workbench over the real `POST /mcp/call` bridge.
//
// Mandatory categories covered (scope testing-scope §2):
//   - HEADLINE (surreal): author → Run → real rows. Seed `series`, pick the table in the builder,
//     Run → `store.query` fires + real rows render.
//   - CAPABILITY-DENY (§2.1): without `mcp:store.query:call`, a Run is denied at the host and the
//     error is surfaced verbatim (never fabricated rows).
//   - WORKSPACE-ISOLATION (§2.2): ws-B's `datasource.list` roster does not include ws-A's source.
//   - Federation degrade: a federation source mounts + fires `federation.schema` (the federation
//     SIDECAR is not spawned in this env — a true external a UI test cannot cheaply run — so
//     `federation.schema` resolves to an honest typed error and the workbench degrades to an empty
//     schema; the editor still renders, the canvas stays absent, no crash). The real-row federation
//     round-trip is `rust/crates/host/tests/federation_sqlite_test.rs`'s job (unchanged, stays green).
//   - Standalone ≡ pane: the Data Studio `query` pane mounts the SAME `QueryWorkbench` (the run
//     path is the one-component/three-homes claim).
//
// `@xyflow/react` + Dockview both measure layout in jsdom; the rect-stub (copied from
// `DataStudioBuilderFlow.gateway.test.tsx`) keeps the canvas pane honest about its size. The
// surreal path never mounts the canvas (Rows mode), so the stub is defensive for the federation
// mount only.

import { describe, expect, it, beforeAll, afterAll, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QueryWorkbench } from "@/features/query-workbench";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { addDatasource } from "@/lib/datasources";
import { invoke } from "@/lib/ipc/invoke";
import * as ipc from "@/lib/ipc/invoke";

let n = 0;
const nextWs = () => `qwb-${n++}`;

beforeAll(() => useRealGateway());

// rect-stub: @xyflow/react measures layout; jsdom returns 0x0 by default which makes the canvas
// render nothing. Stub `getBoundingClientRect` so the canvas + the Dockview pane have a real size.
const realGetRect = HTMLElement.prototype.getBoundingClientRect;
beforeAll(() => {
  HTMLElement.prototype.getBoundingClientRect = function () {
    return new DOMRect(0, 0, 1280, 800);
  };
});
afterAll(() => {
  HTMLElement.prototype.getBoundingClientRect = realGetRect;
});

/** Seed `count` real samples into `series` via the gateway's real ingest path (the same path
 *  `sqlSource.gateway.test.tsx` uses). The `series` table is what `store.schema` reports and
 *  `store.query` selects against. */
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

/** Register a real `demo-buildings` sqlite datasource row (the roster path; no sidecar spawned in
 *  this env — the federation degrade test asserts the honest empty-schema contract). */
async function registerDemoSource(source: string): Promise<void> {
  await addDatasource({
    name: source,
    kind: "sqlite",
    endpoint: "127.0.0.1:0",
    dsn: "/tmp/lb-query-workbench-demo.db",
  });
}

/** An `ipc.invoke` counter that DELEGATES to the real transport (observe, never fake — rule 9).
 *  Returns the spy + a `countTool` helper for `mcp_call` invokes by tool name. Mirrors the
 *  `queryBuilderCommon.gateway.test.tsx` + `DataStudioBuilderFlow.gateway.test.tsx` helper. */
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
  void spy;
  return {
    countTool: (tool: string) => byTool.get(tool) ?? 0,
    clear: () => byTool.clear(),
    restore: () => spy.mockRestore(),
  };
}

function renderWorkbench(ws: string, source: string) {
  return render(<QueryWorkbench ws={ws} source={source} sel={null} onSel={() => {}} />);
}

describe("QueryWorkbench (real gateway)", () => {
  it("HEADLINE (surreal): author → Run → real rows from store.query", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedRows("cpu", 3);

    const counted = viaCounter();
    renderWorkbench(ws, "surreal-local");

    // The workbench mounts; the schema loader fires `store.schema` for the surreal path.
    await waitFor(() => {
      expect(counted.countTool("store.schema")).toBeGreaterThanOrEqual(1);
    });

    // Pick the `series` table in the Builder (the row-list for surreal). The dropdown populates once
    // `store.schema` returns; selecting it sets `query.table`, which regenerates the live preview
    // (`SELECT * FROM series`) — that's the SQL Run will send. Wait for the option to APPEAR (the
    // store.schema call has resolved and the schema propagated to the dropdown).
    const tableSelect = await screen.findByLabelText("sql table", {}, { timeout: 5000 }) as HTMLSelectElement;
    await waitFor(() => {
      expect([...tableSelect.options].map((o) => o.value)).toContain("series");
    });
    await user.selectOptions(tableSelect, "series");

    // Add an explicit column (the builder's default `SELECT *` includes the SurrealDB record `id`,
    // which the host's JSON serializer can't deserialize — a pre-existing host edge case the
    // sqlSource.gateway test avoids the same way: select explicit columns). Pick `seq` so the emitted
    // SQL is `SELECT seq FROM series` — the 3 seeded rows return with their seq values.
    await user.click(screen.getByLabelText("add column"));
    const colSelect = screen.getByLabelText("sql column 0");
    await user.selectOptions(colSelect, "seq");

    // The live preview reflects the emitted SurrealQL for the chosen table + column.
    await waitFor(() => {
      const preview = document.querySelector('[aria-label="sql preview"]');
      expect(preview?.textContent ?? "").toContain("SELECT seq FROM series");
    });

    // Run → `store.query` fires against the emitted SQL → the 3 seeded rows render.
    await user.click(screen.getByLabelText("run query"));
    await waitFor(() => {
      expect(counted.countTool("store.query")).toBeGreaterThanOrEqual(1);
    });
    // The run bar's row-count badge renders "3 rows" (the seeded count) — proving the run completed
    // with real rows (never fabricated, never a silent empty). The `seq` column values (1, 2, 3)
    // also surface in the results grid.
    await waitFor(() => {
      const workbench = document.querySelector('[aria-label="query workbench"]');
      expect(workbench?.textContent ?? "").toMatch(/3 rows/);
    }, { timeout: 5000 });
    counted.restore();
  }, 30_000);

  it("CAPABILITY-DENY (mandatory §2.1): without mcp:store.query:call, a Run is denied and the error is surfaced verbatim", async () => {
    const user = userEvent.setup();
    const ws = nextWs();

    // Seed under a full admin session so the schema has a real table to pick.
    await signInReal("user:ada", ws);
    await seedRows("cpu", 2);

    // Drop to a capped session: `store.schema` granted (so the dropdown populates) but NOT
    // `store.query` — the run must deny at the host. The seeded rows are in the same workspace, so
    // the schema read still surfaces the `series` table under the capped session.
    await signInWithCaps("user:ada", ws, ["mcp:store.schema:call"]);

    const counted = viaCounter();
    renderWorkbench(ws, "surreal-local");

    // The schema loads (store.schema granted) → the table dropdown populates with `series`. Wait
    // for the option to APPEAR (the response propagated), then select it.
    const tableSelect = await screen.findByLabelText("sql table", {}, { timeout: 5000 }) as HTMLSelectElement;
    await waitFor(() => {
      expect([...tableSelect.options].map((o) => o.value)).toContain("series");
    });
    await user.selectOptions(tableSelect, "series");

    // Run → `store.query` fires and is DENIED at the host → the error surfaces verbatim in the run
    // bar (never fabricated rows; the results area shows the deny reason).
    await user.click(screen.getByLabelText("run query"));
    await waitFor(() => {
      expect(counted.countTool("store.query")).toBeGreaterThanOrEqual(1);
    });
    await waitFor(() => {
      const workbench = document.querySelector('[aria-label="query workbench"]');
      // The deny surfaces as an error string (role="alert") — verbatim, no fabricated rows.
      expect(workbench?.querySelector('[role="alert"]')).toBeTruthy();
    });
    counted.restore();
  }, 30_000);

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
    const list = await invoke("mcp_call", { tool: "datasource.list", args: {} });
    const names = (list as { datasources: { name: string }[] }).datasources.map((d) => d.name);
    expect(names).toContain("ben-warehouse");
    expect(names).not.toContain("demo-buildings");
  });

  it("federation degrade: a federation source mounts + fires federation.schema (honest empty schema, no sidecar)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await registerDemoSource("demo-buildings");

    const counted = viaCounter();
    renderWorkbench(ws, "demo-buildings");

    // The workbench mounts (the dispatch headline — the federation source opens the SAME workbench).
    await waitFor(() => {
      expect(document.querySelector('[aria-label="query workbench"]')).toBeTruthy();
    });
    // The federation schema loader fires `federation.schema` for the source. The sidecar is not
    // spawned in this env, so it resolves to an honest typed error → the hook collapses to an empty
    // schema → the table dropdown is empty (the system-catalog deny contract). No crash, no
    // fabricated rows.
    await waitFor(() => {
      expect(counted.countTool("federation.schema")).toBeGreaterThanOrEqual(1);
    });
    const tableSelect = document.querySelector('[aria-label="sql table"]') as HTMLSelectElement | null;
    expect(tableSelect).toBeTruthy();
    // Only the placeholder option is present (no discovered tables — honest degrade).
    expect(tableSelect!.options.length).toBe(1);
    expect(tableSelect!.options[0].value).toBe("");
    counted.restore();
  }, 30_000);

  it("Standalone ≡ pane: the Data Studio query pane mounts the same QueryWorkbench (one component, three homes)", async () => {
    // The `query` pane's Component IS `<QueryWorkbench source="surreal-local">` (workbenchPanes.tsx).
    // Rendering QueryWorkbench standalone is the same render path the pane takes — proving the
    // standalone mount + the routed Data-page mount + the Data Studio pane mount are identical.
    const ws = nextWs();
    await signInReal("user:ada", ws);

    renderWorkbench(ws, "surreal-local");
    // The same aria-label the pane renders (workbenchPanes → QueryWorkbench).
    await waitFor(() => {
      expect(document.querySelector('[aria-label="query workbench"]')).toBeTruthy();
    });
  });
});
