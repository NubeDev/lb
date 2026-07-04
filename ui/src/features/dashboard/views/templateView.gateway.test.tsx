// The render-template-inprocess view, driven against a REAL spawned gateway (render-template-inprocess
// scope, Testing plan — CLAUDE §9 / testing §0, no fake backend). These are the mandatory no-mock paths
// that prove the in-process tier carries the real data + security contracts the iframe did:
//
//   - ANY source: a `template` cell bound to a SERIES/SQL source renders the real seeded rows, IN-PROCESS
//     (no iframe). One source-agnostic path (no per-source code, no branch on the tool id; rule 10).
//   - `[data-call]` write: a click reaches the host over the real bridge; granted → ok; a data-call to
//     a tool the principal LACKS is denied at the host (guard 3 survives the tier change).
//   - WORKSPACE ISOLATION: a `render_template` saved in ws-A is invisible to ws-B (template.get → not
//     found; the hard wall, §6).
//   - REGRESSION: `plot` STILL mounts the iframe (its tier is untouched); `template` does not.
//
// NOTE on rules-as-source: the one path NOT covered here is a `template` bound to `{tool:"rules.run"}`.
// That RENDER path is broken at the host today for EVERY view (chart/table/template): `viz.query`'s
// recursive dispatch of `rules.run` returns empty rows, even though the direct `rules_run` route returns
// the rows. That is a pre-existing host-side gap (pipeline), separate from this client-only render
// scope; it has its own debug entry (debugging/frontend/rules-as-source-render-path-empty.md) and is
// skipped below with a precise note. The in-process template view itself is source-agnostic and renders
// whatever rows `usePanelData` resolves — proven by the series/SQL test below.
//
// `jsdom` has no `EventSource`, so a watch source is not exercised here — the read paths (series.read /
// store.query) are, and they are what a template binds the vast majority of the time.

import { describe, it, expect, beforeAll } from "vitest";
import { render, waitFor, fireEvent } from "@testing-library/react";

import type { Cell } from "@/lib/dashboard";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import { saveRule, runRule } from "@/lib/rules";
import { saveTemplate, getTemplate } from "@/lib/dashboard/template.api";
import { invoke } from "@/lib/ipc/invoke";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { WidgetView } from "../views/WidgetView";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

let n = 0;
const nextWs = () => `tpl-${n++}`;

beforeAll(() => useRealGateway());

/** Seed `count` real samples into `series` via the gateway's real ingest path. */
async function seedSeries(series: string, count: number): Promise<void> {
  const samples = Array.from({ length: count }, (_, i) => ({
    series,
    producer: "user:ada",
    seq: i + 1,
    payload: (i + 1) * 10,
    ts: i + 1,
  }));
  await invoke("mcp_call", { tool: "ingest.write", args: { samples } });
}

/** A `view:"template"` cell bound to a source, with inline body listing the rows + a data-call button. */
function templateCell(src: { tool: string; args: Record<string, unknown> }, code: string, tools: string[] = []): Cell {
  return {
    i: "t1",
    x: 0, y: 0, w: 6, h: 4, v: 3,
    widget_type: "chart",
    view: "template",
    binding: { series: "" },
    sources: [{ refId: "A", ...src, datasource: { type: "surreal" } }],
    options: { code },
    // Fold the data-call tool into the cell so cellTools (the leash) admits it; the host re-checks.
    ...(tools.length ? { action: { tool: tools[0], argsTemplate: {} } } : {}),
  };
}

describe("template view · real gateway · any-source renders in-process", () => {
  it("renders real rows from a SERIES/SQL source — one source-agnostic path, in-process", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries("cpu", 3);

    const cell = templateCell(
      { tool: "store.query", args: { sql: "SELECT seq, payload FROM series ORDER BY seq" } },
      `<ul data-rows>{{#each rows}}<li>{{seq}}: {{payload}}</li>{{/each}}</ul>`,
    );
    const { container } = render(
      <WithDashboardCache ws={ws}><WidgetView cell={cell} workspace={ws} /></WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-rows] li")).not.toBeNull());
    expect(container.querySelectorAll("[data-rows] li")).toHaveLength(3);
    expect(container.textContent).toContain("1: 10");
    expect(container.textContent).toContain("3: 30");
    // CRITICAL: a template view mounts NO iframe (the sandbox is gone for template).
    expect(container.querySelector("iframe")).toBeNull();
  });

  // SKIPPED — a known, pre-existing HOST-side gap, NOT a template-view bug. A `template` (or chart/
  // table) bound to `{tool:"rules.run"}` renders ZERO rows through `viz.query`, even though the direct
  // `rules_run` route returns the rows (verified: `runRule({ruleId})` → `{kind:"scalar", value:[3 rows]}`
  // ✓; the same rule via `viz.query` → `rows.length === 0` ✗). The gap is in the recursive dispatch
  // (`viz.query` → `call_tool_at_depth("rules.run")`) + the `RuleOutput` envelope normalization — a
  // pipeline change this client-only render scope explicitly excludes. Tracked in
  // docs/debugging/frontend/rules-as-source-render-path-empty.md. The in-process template view is
  // source-agnostic (proven by the series/SQL test above); it will render rule rows the moment the host
  // gap is fixed, with NO template-side change.
  it.skip("renders real rows from a RULES source (rules.run) — BLOCKED by host gap, see debug entry", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, [
      "mcp:rules.run:call", "mcp:rules.save:call", "store:rule:read", "store:rule:write",
      "mcp:viz.query:call",
    ]);
    await saveRule({ id: "hourly", name: "Hourly mean", body: `let rows = [#{ h: 0, v: 10 }, #{ h: 1, v: 20 }, #{ h: 2, v: 30 }]; rows` });
    // Sanity: the DIRECT rules.run route returns the rows (the gap is the viz.query path, not the rule).
    const direct = await runRule({ ruleId: "hourly" });
    expect(JSON.stringify(direct.output)).toContain("h");

    const cell = templateCell(
      { tool: "rules.run", args: { rule_id: "hourly" } },
      `<ul data-rows>{{#each rows}}<li>h={{h}} v={{v}}</li>{{/each}}</ul>`,
    );
    const { container } = render(
      <WithDashboardCache ws={ws}><WidgetView cell={cell} workspace={ws} /></WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-rows] li")).not.toBeNull());
    expect(container.querySelectorAll("[data-rows] li")).toHaveLength(3);
    expect(container.querySelector("iframe")).toBeNull();
  });
});

describe("template view · real gateway · [data-call] write + capability deny", () => {
  it("forwards an in-leash [data-call] click to the host (rules.run granted → data-called=\"ok\")", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveRule({ id: "echo", name: "Echo", body: `let rows = [#{ h: 0, v: 1 }]; rows` });

    const cell = templateCell(
      { tool: "rules.run", args: { rule_id: "echo" } },
      `<button data-call="rules.run" data-args='{"rule_id":"echo"}'>Recompute</button>`,
      ["rules.run"], // leash: rules.run is in the cell's tools
    );
    const { container } = render(
      <WithDashboardCache ws={ws}><WidgetView cell={cell} workspace={ws} /></WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-call]")).not.toBeNull());
    const btn = container.querySelector<HTMLButtonElement>("[data-call]")!;
    fireEvent.click(btn);
    // The real host granted the call → ok stamp. (data-call is a write, so it reaches the host verb.)
    await waitFor(() => expect(btn.getAttribute("data-called")).toBe("ok"));
  });

  it("DENIES a [data-call] write the principal lacks at the host (guard 3 survives the tier change)", async () => {
    const ws = nextWs();
    // Grant rules.run READ + the series cap, but NOT template.delete (the write we attempt).
    await signInWithCaps("user:ada", ws, ["mcp:rules.run:call", "mcp:rules.save:call", "store:rule:write"]);
    await saveRule({ id: "r", name: "R", body: `let rows = [#{ h: 0, v: 1 }]; rows` });

    const cell = templateCell(
      { tool: "rules.run", args: { rule_id: "r" } },
      `<button data-call="template.delete" data-args='{"id":"x"}'>Hostile</button>`,
      ["template.delete"], // locally leashed into the cell (the author named it)
    );
    const { container } = render(
      <WithDashboardCache ws={ws}><WidgetView cell={cell} workspace={ws} /></WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-call]")).not.toBeNull());
    const btn = container.querySelector<HTMLButtonElement>("[data-call]")!;
    fireEvent.click(btn);
    // The local leash allows it (the author added template.delete to the cell's tools), so it reaches
    // the host — which DENIES it for lack of `mcp:template.delete:call`. data-called="err".
    await waitFor(() => expect(btn.getAttribute("data-called")).toBe("err"));
  });
});

describe("template view · real gateway · workspace isolation (the hard wall)", () => {
  it("a render_template saved in ws-A is invisible to ws-B (template.get → NotFound)", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveTemplate("secret", "Acme secret", "template", `<p>acme-only {{rows.length}}</p>`);

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    // Ben in ws-B cannot read Ada's ws-A template by the same id — the hard wall (§6).
    await expect(getTemplate("secret")).rejects.toThrow();
  });
});

describe("template view · real gateway · save→get round-trip (the persistence contract)", () => {
  it("persists `options.code` (inline) across save → get → render", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries("cpu", 2);
    const code = `<ul data-rows>{{#each rows}}<li>{{seq}}: {{payload}}</li>{{/each}}</ul>`;
    const cell: Cell = {
      i: "t1", x: 0, y: 0, w: 6, h: 4, v: 3,
      widget_type: "chart", view: "template", binding: { series: "" },
      sources: [{ refId: "A", tool: "store.query", args: { sql: "SELECT seq, payload FROM series ORDER BY seq" }, datasource: { type: "surreal" } }],
      options: { code },
    };
    await saveDashboard("d", "Template", [cell]);
    const reloaded = await getDashboard("d");
    const back = reloaded.cells[0];
    // The body survived the host round-trip under options.code (not stripped, not moved).
    expect(back.view).toBe("template");
    expect((back.options as { code?: string }).code).toBe(code);
    // And the in-process view renders it from the reloaded cell.
    const { container } = render(
      <WithDashboardCache ws={ws}><WidgetView cell={back} workspace={ws} /></WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelectorAll("[data-rows] li").length).toBe(2));
    expect(container.querySelector("iframe")).toBeNull();
  });

  it("persists `options.templateId` (Saved mode) across save → get", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveTemplate("defrost", "Defrost", "template", `<p data-body>defrost body</p>`);
    const cell: Cell = {
      i: "t2", x: 0, y: 0, w: 6, h: 4, v: 3,
      widget_type: "chart", view: "template", binding: { series: "" },
      sources: [{ refId: "A", tool: "store.query", args: { sql: "SELECT 1" }, datasource: { type: "surreal" } }],
      options: { templateId: "defrost" },
    };
    await saveDashboard("d2", "Template Saved", [cell]);
    const back = (await getDashboard("d2")).cells[0];
    expect((back.options as { templateId?: string }).templateId).toBe("defrost");
    // And the in-process view resolves the saved body through template.get and renders it.
    const { container } = render(
      <WithDashboardCache ws={ws}><WidgetView cell={back} workspace={ws} /></WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-body]")).not.toBeNull());
    expect(container.querySelector("iframe")).toBeNull();
  });
});

describe("template view · real gateway · regression (plot stays iframe)", () => {
  it("a `plot` cell STILL mounts the iframe (its author JS eval's — the sandbox is load-bearing)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries("cpu", 2);
    const cell: Cell = {
      i: "p1", x: 0, y: 0, w: 6, h: 4, v: 3,
      widget_type: "chart", view: "plot", binding: { series: "" },
      sources: [{ refId: "A", tool: "store.query", args: { sql: "SELECT seq, payload FROM series" }, datasource: { type: "surreal" } }],
      options: { code: `async (bridge, el) => { el.textContent = "plot"; }` },
    };
    const { container } = render(
      <WithDashboardCache ws={ws}><WidgetView cell={cell} workspace={ws} /></WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("iframe")).not.toBeNull());
    // Plot mounts the iframe; template does not. The tier split is enforced.
    expect(container.querySelector("iframe")).not.toBeNull();
  });
});
