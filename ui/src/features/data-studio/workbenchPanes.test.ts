// The pages-as-panes registry (data-studio-10x scope, phase 2) — the dock's view-pane kinds as plain
// data. Pure data/logic (no JSX render in this test) so a Vitest unit suffices. Asserts the registry
// shape the open-view menu + the dock adapter depend on: each kind maps to a Component, the menu
// excludes the host surface (no recursive embedding), and the kind set is opaque data (rule 10 — the
// dock treats the kind as data, never branching on a host subsystem).

import { describe, expect, it } from "vitest";

import { VIEW_PANES, viewPane } from "./workbenchPanes";

describe("workbenchPanes — the pages-as-panes registry", () => {
  it("lists the 6 core surfaces the workbench can mount as panes", () => {
    const kinds = VIEW_PANES.map((p) => p.kind);
    // Flows, Rules, Data, Datasources, Query (the surreal-local workbench — query-workbench-view
    // slice 3), Ingest — the studio's debugging loop surfaces. Each opens the REAL routed view
    // component (the test asserts the kinds; the gateway suite proves the real mount).
    expect(kinds).toEqual(["flows", "rules", "data", "datasources", "query", "ingest"]);
  });

  it("excludes the host surface (Data Studio) — no recursive embedding", () => {
    // A pane that re-mounted the studio inside the studio would recurse. The registry deliberately
    // omits the studio itself; the dock never offers it in the "+ Open view" menu.
    expect(VIEW_PANES.find((p) => p.kind === "data-studio" || p.title === "Data Studio")).toBeUndefined();
  });

  it("each entry carries the stable kind + title + icon + Component needed to mount + persist", () => {
    for (const p of VIEW_PANES) {
      expect(typeof p.kind).toBe("string");
      expect(typeof p.title).toBe("string");
      expect(p.icon).toBeDefined();
      expect(typeof p.Component).toBe("function");
      expect(typeof p.surface).toBe("string");
    }
  });

  it("viewPane looks up an entry by kind (the dock adapter's path)", () => {
    expect(viewPane("flows")?.title).toBe("Flows");
    expect(viewPane("rules")?.title).toBe("Rules");
    // The Query pane (query-workbench-view slice 3) — its own kind ("query"), gated by the `data`
    // surface lens but with a distinct dock identity + the "Query" title.
    expect(viewPane("query")?.title).toBe("Query");
    expect(viewPane("query")?.surface).toBe("data");
    // An unknown kind returns undefined — the dock adapter renders "Unknown view pane." (no crash).
    expect(viewPane("nonexistent")).toBeUndefined();
  });

  it("the kinds are opaque data — the dock never branches on a host subsystem id (rule 10)", () => {
    // The registry is data: each kind is a plain string, the Component is a function reference. There is
    // no `if (kind === "flows")` branch anywhere in the dock adapter (`ViewDockPanel`); it looks up the
    // entry by `viewPane(params.kind)` and renders `def.Component`. A new pane kind joins the studio by
    // appending to this array — no other change.
    for (const p of VIEW_PANES) {
      // The kind is a stable, lowercase, hyphen-free identifier — never a host subsystem id.
      expect(p.kind).toMatch(/^[a-z][a-z]*$/);
    }
  });
});
