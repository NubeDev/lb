// The new-panel wizard's step-1 rule discoverability (panel-wizard-source-discoverability scope) —
// gateway test against a REAL spawned gateway + REAL seeded rule (rule 9 — no `*.fake.ts`). The picker +
// render halves shipped (rules-as-source / rules-for-widgets); this proves the DISCOVERABILITY fix: from
// step 1 a user reaches the rule in ≤2 clicks, the "Rules" group LEADS the workspace combobox (not
// buried seventh), and the emitted source is the shipped `{tool:"rules.run", args:{rule_id, route:false}}`.
//
// It drives the SAME wizard shell the editor mounts (no wizard-only state) over the real `rules.*` verbs +
// real store, so the ws-wall + deny-tolerance land here where a node is in-process.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, within, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { saveRule } from "@/lib/rules";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { PanelWizard } from "@/features/panel-builder/wizard/PanelWizard";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `pwiz-rule-${n++}`;

// A records-returning rule — the chart-ready output a data panel draws (mirrors rulesSource.gateway).
const DATA_RULE_BODY = `let rows = [#{ h: 0, v: 1 }, #{ h: 1, v: 2 }]; rows`;

// The narrow cap set for the deny/empty-state path: save (to plant a rule) + store, but NO
// `mcp:rules.list:call` — the picker's Rules group must come back empty, not crash.
const NO_LIST_CAPS = ["mcp:rules.save:call", "mcp:rules.run:call", "store:rule:read", "store:rule:write"];

function mountWizard(ws: string) {
  return render(
    <WithDashboardCache ws={ws}>
      <PanelWizard ws={ws} dashboardId="d-any" onExit={() => {}} />
    </WithDashboardCache>,
  );
}

describe("PanelWizard step 1 — rule discoverability (real gateway)", () => {
  it("reaches the seeded rule in ≤2 clicks with the Rules group LEADING the combobox", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveRule({ id: "hourly", name: "Hourly mean", body: DATA_RULE_BODY });
    const user = userEvent.setup();
    mountWizard(ws);

    expect(screen.getByLabelText("wizard source step")).toBeDefined();
    // Click 1 — the Workspace-source card. Click 2 — open the source combobox. No third click.
    await user.click(await screen.findByLabelText("source track workspace"));
    await user.click(screen.getByLabelText("wizard source"));

    // The seeded rule is offered under a VISIBLE "Rules" heading…
    const list = await screen.findByRole("listbox", { name: "wizard source" });
    expect(await screen.findByRole("option", { name: "Hourly mean" })).toBeDefined();
    // …and "Rules" is the FIRST group heading — before "Series"/"Saved queries" (the reorder is the fix).
    const headings = within(list)
      .getAllByText(/^(Rules|Series|Saved queries|Direct SurrealDB|Live \(Zenoh\)|Installed extension|Extension widgets)$/)
      .map((el) => el.textContent);
    expect(headings[0]).toBe("Rules");
    cleanup();
  });

  it("adopts the shipped source shape {tool:'rules.run', args:{rule_id, route:false}} when the rule is picked", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveRule({ id: "hourly", name: "Hourly mean", body: DATA_RULE_BODY });
    const user = userEvent.setup();
    mountWizard(ws);

    await user.click(await screen.findByLabelText("source track workspace"));
    await user.click(screen.getByLabelText("wizard source"));
    await user.click(await screen.findByRole("option", { name: "Hourly mean" }));

    // The bound-source readout reflects the adopted target — the render half's contract, unchanged.
    await waitFor(() =>
      expect(screen.getByLabelText("wizard source picked").textContent).toContain("rules.run"),
    );
    cleanup();
  });

  it("proves the rule works: Run renders the returned rows in the result grid, before binding", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveRule({ id: "hourly", name: "Hourly mean", body: DATA_RULE_BODY });
    const user = userEvent.setup();
    mountWizard(ws);

    await user.click(await screen.findByLabelText("source track workspace"));
    await user.click(screen.getByLabelText("wizard source"));
    await user.click(await screen.findByRole("option", { name: "Hourly mean" }));

    // The prove-it workbench appears with a Run — the parity twin of the datasource track.
    const run = await screen.findByLabelText("run rule");
    await user.click(run);
    // The real `rules.run` executes and the shipped RunResult pane renders the returned rows (the rule
    // is proven BEFORE binding). Its two values (v: 1, v: 2) appear in the result.
    const result = await screen.findByLabelText("run result");
    await waitFor(() => expect(result.textContent).toMatch(/1[\s\S]*2/));
    cleanup();
  });

  it("runs a PARAM-driven rule from the wizard: the filled param reaches the result", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Echoes its `site` param into a row — proves the wizard's params form feeds `rules.run args.params`.
    await saveRule({
      id: "echo",
      name: "Echo site",
      body: `[#{ site: param("site") }]`,
      params: [{ name: "site", label: "Site" }],
    });
    const user = userEvent.setup();
    mountWizard(ws);

    await user.click(await screen.findByLabelText("source track workspace"));
    await user.click(screen.getByLabelText("wizard source"));
    await user.click(await screen.findByRole("option", { name: "Echo site" }));

    // The params form is present (the rule declared `site`); fill it, then Run.
    const siteInput = await screen.findByLabelText("rule param site");
    await user.type(siteInput, "acme-hq");
    await user.click(screen.getByLabelText("run rule"));
    // The param value rides through `rules.run` into the returned row — rendered in the result pane.
    await waitFor(() =>
      expect(screen.getByLabelText("run result").textContent).toContain("acme-hq"),
    );
    cleanup();
  });

  it("front-loads 'rule' in the Workspace-source card (scent regression guard)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    mountWizard(ws);
    // The card's accessible text names 'rule' — guards a future subtitle edit from re-burying it.
    const card = await screen.findByLabelText("source track workspace");
    expect(card.textContent?.toLowerCase()).toContain("rule");
    cleanup();
  });

  it("shows an honest empty-Rules line (no saved rules) so the path is discoverable before the first rule", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws); // no rule seeded
    const user = userEvent.setup();
    mountWizard(ws);

    await user.click(await screen.findByLabelText("source track workspace"));
    await waitFor(() =>
      expect(screen.getByLabelText("wizard no rules").textContent).toContain("No saved rules yet"),
    );
    cleanup();
  });

  it("CAPABILITY-DENY: without mcp:rules.list:call the Rules group is empty + the honest line shows (no crash)", async () => {
    const ws = nextWs();
    // Plant a rule WITH save, then re-sign WITHOUT the list cap — the picker must not surface it.
    await signInWithCaps("user:ada", ws, ["mcp:rules.save:call", "store:rule:read", "store:rule:write"]);
    await saveRule({ id: "present", name: "Present", body: DATA_RULE_BODY });
    await signInWithCaps("user:ada", ws, NO_LIST_CAPS);
    const user = userEvent.setup();
    mountWizard(ws);

    await user.click(await screen.findByLabelText("source track workspace"));
    await user.click(screen.getByLabelText("wizard source"));
    // Denied read → empty Rules group → the honest line, never the seeded rule, never a crash.
    await waitFor(() =>
      expect(screen.getByLabelText("wizard no rules").textContent).toContain("No saved rules yet"),
    );
    expect(screen.queryByRole("option", { name: "Present" })).toBeNull();
    cleanup();
  });

  it("WORKSPACE ISOLATION: a rule saved in workspace A is not offered in the wizard opened in B", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveRule({ id: "secret", name: "Acme only", body: DATA_RULE_BODY });

    const wsB = nextWs();
    await signInReal("user:bob", wsB); // a different tenant — the hard wall
    const user = userEvent.setup();
    mountWizard(wsB);

    await user.click(await screen.findByLabelText("source track workspace"));
    await user.click(screen.getByLabelText("wizard source"));
    // B has no rules of its own → the honest line, and A's rule is never offered.
    await waitFor(() =>
      expect(screen.getByLabelText("wizard no rules").textContent).toContain("No saved rules yet"),
    );
    expect(screen.queryByRole("option", { name: "Acme only" })).toBeNull();
    cleanup();
  });
});
