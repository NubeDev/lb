// The render-template widget wizard, driven against a REAL in-process seeded gateway (setup scope;
// CLAUDE §9 — no fake backend). Proves the wizard is pure orchestration over the real verbs:
//   - registering the demo datasource lands a real `datasource.list` row;
//   - the Design step mounts the real in-process `TemplateView` preview;
//   - Save lands a real `render_template` via `template.save` — read back over the gateway;
//   - "Add to a new dashboard" lands a real `dashboard.save` with a `view:"template"` cell — read back.
// Plus the two mandatory categories (testing-scope §2): a CAP-DENY (a session without `dashboard.save`
// sees no save controls AND the host refuses `template.save`), and WORKSPACE ISOLATION (a template
// saved in ws-A is invisible in ws-B). A fresh workspace per test isolates the shared node.
//
// Note: no federation sidecar is spawned in this env (same as the Query-workbench + data→insight
// gateway tests), so the buildings query returns no rows here — we assert the REAL WRITE effects (the
// source row, the saved template, the saved dashboard) and that the query/design paths mount honestly.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";
import { TemplateWidgetWizard } from "./TemplateWidgetWizard";
import { CAP } from "@/lib/session/admin-caps";
import { listDatasources } from "@/lib/datasources";
import { getDashboard, listDashboards } from "@/lib/dashboard";
import { getTemplate, listTemplates } from "@/lib/dashboard/template.api";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

// The caps the wizard's steps drive: datasource read/add, query, template save/get/list, dashboard save.
const CAPS = [
  CAP.datasourceList,
  "mcp:datasource.add:call",
  "mcp:federation.query:call",
  "mcp:template.save:call",
  "mcp:template.get:call",
  "mcp:template.list:call",
  CAP.dashboardSave,
  CAP.dashboardGet,
  CAP.dashboardList,
];

let n = 0;
const nextWs = () => `tpl-wiz-${n++}`;

function renderWizard(ws: string, caps: string[]) {
  return render(
    <DashboardCacheProvider ws={ws}>
      <TemplateWidgetWizard ws={ws} caps={caps} />
    </DashboardCacheProvider>,
  );
}

beforeAll(() => useRealGateway());

describe("TemplateWidgetWizard (real seeded gateway)", () => {
  it("registers a datasource, saves a real render_template, and drops it on a real dashboard", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, CAPS);
    renderWizard(ws, CAPS);

    // ── Step 1 (intro): the overview names the five parts. ──
    await screen.findByText("Build a render-template widget");
    expect(screen.getByText(/Design — write the widget/)).toBeInTheDocument();
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 2 (datasource): register the demo → a REAL datasource.list row lands. ──
    await user.click(await screen.findByLabelText("Register the buildings demo datasource"));
    await waitFor(async () => {
      const rows = await listDatasources();
      expect(rows.some((d) => d.name === "demo-buildings")).toBe(true);
    });
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 3 (query): preloaded read-only; Run drives the real engine (empty here, no sidecar). ──
    await screen.findByLabelText("query");
    await user.click(screen.getByLabelText("Run the query"));
    await waitFor(() => expect(screen.getByLabelText("Run the query")).toBeEnabled());
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 4 (design): the real in-process TemplateView preview mounts + the inline editor is here. ──
    await screen.findByLabelText("widget preview");
    expect(screen.getByLabelText("template code")).toBeInTheDocument();
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 5 (ask an AI): the shipped Copy-AI-prompt control mounts. ──
    await screen.findByLabelText("copy ai prompt");
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 6 (save): Save lands a REAL render_template; read it back over the gateway. ──
    await user.click(await screen.findByLabelText("Save the template"));
    await screen.findByText(/Saved as/i);
    const saved = await waitFor(async () => {
      const list = await listTemplates();
      const row = list.find((t) => t.title === "Hourly energy by site");
      expect(row).toBeTruthy();
      return row!;
    });
    const tpl = await getTemplate(saved.id);
    expect(tpl.engine).toBe("template");
    expect(tpl.code).toContain("{{#each rows}}");

    // ── Add to a new dashboard → a REAL dashboard.save with a `view:"template"` cell. ──
    await user.click(await screen.findByLabelText("Add to a new dashboard"));
    await screen.findByText(/Added to dashboard/i);
    const dashRow = await waitFor(async () => {
      const list = await listDashboards();
      const row = list.find((d) => d.title === "Hourly energy by site");
      expect(row).toBeTruthy();
      return row!;
    });
    const dash = await getDashboard(dashRow.id);
    expect(dash.cells).toHaveLength(1);
    expect(dash.cells[0].view).toBe("template");
    expect(dash.cells[0].sources?.[0].tool).toBe("federation.query");
  });

  it("CAP DENY: a session without dashboard.save sees no save controls and the host refuses template.save", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    // Query-only caps: can run, but cannot save a template/dashboard.
    const denyCaps = [CAP.datasourceList, "mcp:datasource.add:call", "mcp:federation.query:call"];
    await signInWithCaps("user:viewer", ws, denyCaps);
    renderWizard(ws, denyCaps);

    // Jump to the Save step (the rail lets a reached step be revisited; walk Continue through).
    await screen.findByText("Build a render-template widget");
    for (let i = 0; i < 5; i++) await user.click(screen.getByLabelText("Continue"));

    // The save controls are hidden (display gate) — and the host is the wall regardless.
    await screen.findByText(/needs widget-write access/i);
    expect(screen.queryByLabelText("Save the template")).toBeNull();
    // The gateway refuses `template.save` for this identity (opaque deny), proving the host backstop.
    const { saveTemplate } = await import("@/lib/dashboard/template.api");
    await expect(saveTemplate("render_template:x", "X", "template", "<div></div>")).rejects.toThrow();
  });

  it("WORKSPACE ISOLATION: a render_template saved in ws-A is invisible in ws-B", async () => {
    const wsA = nextWs();
    const wsB = nextWs();
    const { saveTemplate } = await import("@/lib/dashboard/template.api");

    await signInReal("user:ada", wsA);
    await saveTemplate("render_template:secret", "Secret", "template", "<div>{{rows.length}}</div>");
    // ws-A can read its own template back.
    expect((await getTemplate("render_template:secret")).title).toBe("Secret");

    // ws-B (re-derived from the token) cannot see ws-A's template.
    await signInReal("user:ada", wsB);
    await expect(getTemplate("render_template:secret")).rejects.toThrow();
    expect((await listTemplates()).some((t) => t.id === "render_template:secret")).toBe(false);
  });
});
