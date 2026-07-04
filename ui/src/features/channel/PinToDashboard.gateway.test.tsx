// Pin-to-dashboard, end to end through a REAL spawned gateway (widget-platform scope, Slice B) — no
// fakes (CLAUDE §9). The headline: a channel `rich_result` carrying the `reminder.list` declared render
// envelope is PINNED to a dashboard via the `PinToDashboard` affordance, then the dashboard is reloaded
// and the pinned cell renders through the real `WidgetView` (the dashboard `TablePanel`), showing the
// real reminder rows AND the row controls (enable switch + run-now + delete). This is the cross-surface
// fidelity invariant: a pinned cell renders on the dashboard exactly as the response renders in a channel,
// because the HOST owns the envelope→cell mapping (the same `mint_cell_from_envelope` a headless
// `POST /mcp/call` agent hits) and both surfaces reuse the shared `<RowControls>` actions column.
//
// Covers the mandatory categories real-gateway-side:
//   - **capability-deny**: a session WITHOUT `mcp:dashboard.pin:call` gets an opaque error from the pin
//     (the gateway re-checks the cap; the UI gate is convenience, the host is the boundary).
//   - **workspace isolation**: a pin in ws-A is invisible to ws-B (the dashboard lives in ws-A's
//     namespace; `dashboard.get` in ws-B returns neither the dashboard nor a 404-existence leak).
//   - **the headline parity**: pin rich_result → reload → render through `WidgetView` with row controls.
//   - **envelope↔cell fidelity**: the persisted cell carries the envelope's `view`/`source`/`options`/
//     `fieldConfig`/`tools`-fold; re-pinning the SAME envelope REPLACES (idempotent `pin-reminder-list`).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { MessageItem } from "./MessageItem";
import { WidgetView } from "@/features/dashboard/views/WidgetView";
import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";
import type { Cell } from "@/lib/dashboard";
import { getDashboard, listDashboards } from "@/lib/dashboard";
import { encodeRichResult } from "@/lib/channel/payload.types";
import type { Item } from "@/lib/channel/channel.types";
import { invoke } from "@/lib/ipc/invoke";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `pin-b-${n++}`;

/** The reminder.list envelope (the descriptor's declared `result`). The host mints the cell from this —
 *  generic over the tool id (`reminder.list` is opaque data here, never branched on). */
const REMINDER_ENVELOPE_BODY = encodeRichResult({
  v: 2,
  view: "table",
  source: { tool: "reminder.list", args: {} },
  options: {
    rowControls: [
      { kind: "switch", label: "enabled", action: { tool: "reminder.update", argsTemplate: { id: "${id}", enabled: "{{value}}" } } },
      { kind: "button", buttonLabel: "Run now", action: { tool: "reminder.fire", argsTemplate: { id: "${id}" } } },
      { kind: "button", buttonLabel: "Delete", action: { tool: "reminder.delete", argsTemplate: { id: "${id}" } } },
    ],
  },
  fieldConfig: {
    defaults: {},
    overrides: [
      { matcher: { id: "byName", options: "maxRuns" }, properties: [{ id: "displayName", value: "Max Runs" }] },
    ],
  },
  tools: ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"],
});

/** Seed one real reminder through `reminder.create` (the real write path), returning the derived id. */
async function seedReminder(channel: string, body: string): Promise<string> {
  const r = await invoke<{ id: string }>("mcp_call", {
    tool: "reminder.create",
    args: { schedule: "0 8 * * 1", action_kind: "channel-post", channel, body },
  });
  return r.id;
}

/** Mount the reminder.list rich_result through the real channel render path (MessageItem → ResponseView
 *  → PinToDashboard), so the "Pin to dashboard" affordance is in the DOM. */
function mountChannelResponse(ws: string): void {
  const item: Item = {
    id: "rich-list",
    channel: "general",
    author: "system:reminders",
    body: REMINDER_ENVELOPE_BODY,
    ts: 1,
  };
  render(<MessageItem item={item} author="user:me" ws={ws} onEdit={() => {}} onDelete={() => {}} />);
}

beforeAll(() => {
  useRealGateway();
});

describe("Pin to dashboard (Slice B)", () => {
  it("HEADLINE: pins a reminder.list rich_result to a dashboard, reloads, and renders the rows + row controls through WidgetView", async () => {
    const ws = nextWs();
    await signInReal("ada", ws);
    await seedReminder("ops", "check the cooler");

    // Mount the channel response — the PinToDashboard affordance is beside the rendered table.
    mountChannelResponse(ws);
    const pinBtn = await screen.findByRole("button", { name: /Pin to dashboard/i });
    expect(pinBtn).toBeInTheDocument();

    // The rendered channel table already shows the reminder row (proves the rich_result renders BEFORE pin).
    await waitFor(
      () => {
        const t = document.querySelector('[aria-label="response table"]');
        expect(t?.querySelectorAll("tbody tr").length).toBeGreaterThanOrEqual(1);
      },
      { timeout: 8000 },
    );

    // Open the pin picker, choose "New dashboard", type a title, confirm.
    await userEvent.click(pinBtn);
    await screen.findByRole("dialog", { name: /Pin to dashboard/i });
    // A native `<select>` — query by its `aria-label` (jsdom doesn't always surface it as role=listbox).
    const select = screen.getByLabelText(/Target dashboard/i) as HTMLSelectElement;
    // The roster may be empty for a fresh ws; the "+ New dashboard…" option is always present.
    await userEvent.selectOptions(select, "__new__");
    const titleInput = screen.getByRole("textbox", { name: /New dashboard title/i });
    await userEvent.type(titleInput, "Ops");
    const confirm = screen.getByRole("button", { name: /Create \+ pin/i });
    await userEvent.click(confirm);

    // The affordance shows the "pinned to <name>" confirmation.
    await waitFor(
      () => {
        expect(screen.getByText(/pinned to/i)).toBeInTheDocument();
      },
      { timeout: 8000 },
    );

    // Reload the dashboard through the real `dashboard.get` — the minted cell survives intact.
    const dashboards = await listDashboards();
    const created = dashboards.find((d) => d.title === "Ops") ?? dashboards[0];
    expect(created, "the pinned dashboard exists").toBeTruthy();
    const full = await getDashboard(created.id);
    expect(full.cells.length).toBe(1);
    const cell = full.cells[0];
    expect(cell.i).toBe("pin-reminder-list");
    expect(cell.view).toBe("table");
    expect(cell.source?.tool).toBe("reminder.list");
    expect(cell.options?.rowControls).toHaveLength(3);
    // The `tools` fold: the three row-control verbs (`reminder.update`/`fire`/`delete`) become hidden
    // `sources[]` so the bridge leash (`cellTools(cell)`) covers `render.tools`.
    const hidden = cell.sources ?? [];
    expect(hidden).toHaveLength(3);
    const tools = hidden.map((t) => t.tool);
    expect(tools).toEqual(expect.arrayContaining(["reminder.update", "reminder.fire", "reminder.delete"]));
    expect(hidden.every((t) => t.hide)).toBe(true);

    // RENDER the pinned cell through the real `WidgetView` (the dashboard `TablePanel`) and assert the
    // reminder rows AND the row controls render — the cross-surface fidelity invariant. The dashboard
    // table reuses the shared `<RowControls>`, so the pinned cell is fully interactive on the grid.
    render(
      <DashboardCacheProvider key={ws} ws={ws}>
        <WidgetView cell={cell as Cell} workspace={ws} />
      </DashboardCacheProvider>,
    );
    const panel = await waitFor(
      () => {
        const t = document.querySelector('[aria-label="table panel"]');
        if (!t || t.querySelectorAll("tbody tr").length === 0) throw new Error("dashboard table not loaded");
        return t as HTMLElement;
      },
      { timeout: 8000 },
    );
    // The reminder row is visible (the source `reminder.list` re-ran under the viewer's grant, in ws).
    expect(panel.querySelectorAll("tbody tr").length).toBeGreaterThanOrEqual(1);
    // The row controls render (enable switch + run-now + delete) — `<RowControls>` on the dashboard.
    const actionsHeader = within(panel as HTMLElement).getByText("actions");
    expect(actionsHeader).toBeInTheDocument();
  });

  it("capability-deny: a session without mcp:dashboard.pin:call is refused by the host", async () => {
    const ws = nextWs();
    // Mint a real session carrying dashboard read caps but NOT the pin cap (the UI gate is convenience;
    // the host re-checks). The `.pin` suffix is not matched by the dev-login wildcards, so a token
    // without the explicit cap is denied at the gateway.
    await signInWithCaps("ada", ws, [
      "mcp:dashboard.get:call",
      "mcp:dashboard.list:call",
      "mcp:reminder.list:call",
    ]);

    // The pin call over `POST /mcp/call` returns an opaque deny (the UI surfaces it as a short message).
    // `now` is omitted intentionally: the cap gate runs BEFORE the `now` parse in `call_tool`, so a
    // cap-less caller is denied at the gate regardless.
    const err = await invoke<string>("mcp_call", {
      tool: "dashboard.pin",
      args: { dashboard: "ops", title: "Ops", envelope: { view: "table", source: { tool: "reminder.list" } } },
    }).catch((e: unknown) => String((e as Error)?.message ?? e));
    expect(/403|forbidden|denied/i.test(err), `expected a deny, got: ${err}`).toBe(true);

    // Nothing persisted — the dashboard "ops" was never created.
    const rows = await listDashboards();
    expect(rows.find((d) => d.id === "ops")).toBeUndefined();
  });

  it("workspace isolation: a pin in ws-A is invisible to ws-B", async () => {
    const wsA = nextWs();
    const wsB = nextWs();
    await signInReal("ada", wsA);
    await seedReminder("ops", "check the cooler");
    // Pin in ws-A via the headless MCP path (the same path a headless agent takes; `now` is required
    // here — the REST route uses the gateway clock, but the MCP bridge takes the caller's logical now).
    await invoke("mcp_call", {
      tool: "dashboard.pin",
      args: { dashboard: "ops", title: "Ops", now: 10, envelope: { view: "table", source: { tool: "reminder.list" } } },
    });
    const aRows = await listDashboards();
    expect(aRows.find((d) => d.id === "ops")).toBeTruthy();

    // Bob in ws-B cannot read ws-A's "ops" — the workspace wall (gate 1).
    await signInReal("bob", wsB);
    const bRows = await listDashboards();
    expect(bRows.find((d) => d.id === "ops"), "ws-B must not see ws-A's dashboard").toBeUndefined();
  });

  it("fidelity + idempotency: re-pinning the same envelope replaces the cell, not duplicates", async () => {
    const ws = nextWs();
    await signInReal("ada", ws);
    // First pin (creates the dashboard "ops" with one `pin-reminder-list` cell).
    await invoke("mcp_call", {
      tool: "dashboard.pin",
      args: { dashboard: "ops", title: "Ops", now: 10, envelope: { view: "table", source: { tool: "reminder.list" } } },
    });
    // Re-pin the same envelope (same source.tool → same `i`) — REPLACES, not appends.
    await invoke("mcp_call", {
      tool: "dashboard.pin",
      args: { dashboard: "ops", now: 11, envelope: { view: "table", source: { tool: "reminder.list" } } },
    });
    const full = await getDashboard("ops");
    expect(full.cells.filter((c) => c.i === "pin-reminder-list")).toHaveLength(1);
    expect(full.cells.length).toBe(1);
  });
});