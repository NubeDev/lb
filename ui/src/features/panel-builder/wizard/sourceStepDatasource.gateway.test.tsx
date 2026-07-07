// SourceStep — the datasource track (panel-wizard scope, user ask: "reuse 100% of the datasource
// Query page, including the saved queries"). Against a REAL gateway: registering a federation
// datasource surfaces it in the wizard's Datasource select; picking it mounts the FULL
// `QueryWorkbench` (the exact component the Datasources detail page mounts — Builder⇄Code, Run,
// saved queries); loading a saved SQL query ADOPTS it as the panel's primary target via the
// workbench's `onUseSql` seam → `federation.query {source, sql}` (the editor Query tab's wire shape).
//
// No sidecar spawns in this env, so the query does not return rows here — the rust
// `federation_sqlite_test.rs` e2e owns the data round-trip. What THIS suite pins is the binding:
// real `datasource.add`, real `query.save`/`query.get`, the real dialog UI, and the adopted target.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import { addDatasource } from "@/lib/datasources";
import { saveQuery } from "@/lib/queries";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { PanelWizard } from "@/features/panel-builder/wizard/PanelWizard";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `srcds-${n++}`;

describe("SourceStep — datasource track reuses the QueryWorkbench + saved queries (real gateway)", () => {
  it("pick datasource → workbench mounts → loading a saved query adopts federation.query{source,sql}", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // A real registered datasource (roster path) + a real saved query against it.
    await addDatasource({ name: "demo-buildings", kind: "sqlite", endpoint: "127.0.0.1:0", dsn: "/tmp/lb-wizard-demo.db" });
    await saveQuery({
      id: "all-points",
      name: "All Points",
      lang: "raw",
      text: "SELECT id, meter_id, name FROM point ORDER BY meter_id, id",
      target: "datasource:demo-buildings",
    });

    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-any" onExit={() => {}} />
      </WithDashboardCache>,
    );

    // The registered datasource appears in the wizard's Datasource select.
    const dsSelect = (await screen.findByLabelText("wizard datasource", {}, { timeout: 5000 })) as HTMLSelectElement;
    await waitFor(() => {
      expect([...dsSelect.options].map((o) => o.value)).toContain("demo-buildings");
    });
    await user.selectOptions(dsSelect, "demo-buildings");

    // The FULL QueryWorkbench mounts (the same component the Datasources detail page mounts) and the
    // target is already the federation shape (empty SQL until a query is adopted).
    await waitFor(() => expect(screen.getByLabelText("query workbench")).toBeInTheDocument());
    expect(screen.getByLabelText("wizard source picked").textContent).toContain("federation.query");

    // Open the saved-queries dialog (the 100%-reuse headline) and load the saved query.
    await user.click(screen.getByLabelText("open saved query"));
    await waitFor(() => expect(screen.getByLabelText("saved query list")).toBeInTheDocument());
    await user.click(screen.getByLabelText("open All Points"));

    // The load resolves via the real `query.get` and the workbench's onUseSql seam ADOPTS the SQL as
    // the panel's source: `federation.query {source: demo-buildings, sql: <saved text>}`.
    await waitFor(() => {
      const picked = screen.getByLabelText("wizard source picked").textContent ?? "";
      expect(picked).toContain("federation.query");
      expect(picked).toContain("demo-buildings");
      expect(picked).toContain("SELECT id, meter_id, name FROM point");
    }, { timeout: 5000 });

    // Next → chart type, Back → source: the wizard REMOUNTS the step; the adopted query must
    // survive (the workbench re-seeds from the persisted EditorState via `initial` — no data loss).
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard chart-type step")).toBeInTheDocument());
    await user.click(screen.getByText("Back"));
    await waitFor(() => expect(screen.getByLabelText("wizard source step")).toBeInTheDocument());
    // The target binding is intact...
    const picked = screen.getByLabelText("wizard source picked").textContent ?? "";
    expect(picked).toContain("federation.query");
    expect(picked).toContain("SELECT id, meter_id, name FROM point");
    // ...and the REMOUNTED workbench editor still carries the authored SQL (not an empty editor).
    await waitFor(() => {
      const wb = document.querySelector('[aria-label="query workbench"]');
      expect(wb?.textContent ?? "").toContain("SELECT id, meter_id, name FROM point");
    }, { timeout: 5000 });

    cleanup();
  }, 30_000);
});
