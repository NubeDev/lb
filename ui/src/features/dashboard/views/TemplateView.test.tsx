// TemplateView unit tests — the in-process render + the leashed `[data-call]` wiring, against a thin
// IPC stub (rule 9: a transport shim, NOT a fake backend). These cover the render-tier specifics from the
// scope's Testing plan that don't need the real gateway: rows from a seeded usePanelData, the data-call
// leash (in-leash forwarded, out-of-leash rejected with NO invoke), the denied-source panel, and the
// assert that NO WidgetIframe is mounted for the template view. The any-source (incl. rules), capability-
// deny-at-host, and workspace-isolation paths run against the REAL gateway in the gateway test.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor, fireEvent, cleanup } from "@testing-library/react";
import type { Cell } from "@/lib/dashboard";

// Stub the ONE ipc seam so usePanelData resolves rows without a gateway (a thin stub, not a fake node —
// the sanctioned pattern, see channel/ResponseView.test.tsx). Routes by tool: `viz.query` → seeded rows;
// an in-leash write tool (e.g. rules.run) → a captured echo; anything else → empty. Typed loose so per-
// test `mockImplementation` overrides can return the shape each case needs without narrowing the union.
const invokeMock = vi.fn(
  async (_channel: string, args: { tool?: string }): Promise<Record<string, unknown>> => {
    const tool = args?.tool ?? "";
    if (tool === "viz.query") {
      return { rows: [{ hour: 0, mean: 10 }, { hour: 1, mean: 20 }, { hour: 2, mean: 30 }] };
    }
    if (tool === "rules.run") return { ok: true };
    return {};
  },
);
vi.mock("@/lib/ipc/invoke", () => ({
  invoke: (c: string, a: { tool?: string }) => invokeMock(c, a) as Promise<unknown>,
}));

import { TemplateView } from "./TemplateView";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

/** A template cell whose inline body lists the rows + a `rules.run` data-call button. `rules.run` is in
 *  the cell's tool set (the leash); the source is `rules.run` too so usePanelData fetches via viz.query. */
function templateCell(overrides: Partial<Cell> = {}): Cell {
  return {
    i: "t1",
    x: 0, y: 0, w: 6, h: 4, v: 3,
    widget_type: "chart",
    view: "template",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "rules.run", args: { rule_id: "r1" }, datasource: { type: "surreal" } }],
    options: {
      code: `<ul>{{#each rows}}<li>hour {{hour}}: {{mean}}</li>{{/each}}</ul>
<button data-call="rules.run" data-args='{"rule_id":"r1"}'>Recompute</button>`,
    },
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockClear();
  cleanup();
});

describe("TemplateView — in-process render + leashed bridge", () => {
  it("renders rows from usePanelData (in-process, no iframe)", async () => {
    const cell = templateCell();
    const { container } = render(
      <WithDashboardCache ws="ws-tpl">
        <TemplateView cell={cell} />
      </WithDashboardCache>,
    );
    // The rows arrive via viz.query → interpolateTemplate → the rendered <li> text.
    await waitFor(() => {
      expect(container.querySelectorAll("li")).toHaveLength(3);
    });
    expect(container.textContent).toContain("hour 0: 10");
    expect(container.textContent).toContain("hour 2: 30");
    // CRITICAL: a template view mounts NO iframe (the sandbox is gone for template).
    expect(container.querySelector("iframe")).toBeNull();
    // The data-view marker the post-commit wiring reads.
    expect(container.querySelector('[data-view="template"]')).not.toBeNull();
  });

  it("forwards an in-leash [data-call] click through the bridge (rules.run IS in cell.tools)", async () => {
    const cell = templateCell();
    const { container } = render(
      <WithDashboardCache ws="ws-tpl">
        <TemplateView cell={cell} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-call]")).not.toBeNull());
    const btn = container.querySelector<HTMLButtonElement>("[data-call]")!;
    fireEvent.click(btn);
    await waitFor(() => expect(btn.getAttribute("data-called")).toBe("ok"));
    // The write went through the bridge → invoke saw an mcp_call for rules.run.
    const calls = invokeMock.mock.calls.filter((c) => c[1]?.tool === "rules.run");
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[0][1]).toMatchObject({ tool: "rules.run", args: { rule_id: "r1" } });
  });

  it("REJECTS a [data-call] outside cell.tools: NO invoke, data-called=\"err\" (the leash)", async () => {
    // The button names a tool NOT in the cell's tool set. The bridge's local gate fires before invoke.
    const cell = templateCell({
      options: {
        code: `<button data-call="store.delete" data-args='{"id":"x"}'>Hostile</button>`,
      },
    });
    const { container } = render(
      <WithDashboardCache ws="ws-tpl">
        <TemplateView cell={cell} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-call]")).not.toBeNull());
    const btn = container.querySelector<HTMLButtonElement>("[data-call]")!;
    fireEvent.click(btn);
    await waitFor(() => expect(btn.getAttribute("data-called")).toBe("err"));
    // The local leash rejected the call BEFORE invoking — the host never sees store.delete from this click.
    const hostile = invokeMock.mock.calls.filter((c) => c[1]?.tool === "store.delete");
    expect(hostile).toHaveLength(0);
  });

  it("stamps data-called=\"err\" on a malformed data-args JSON (degrades honestly, never crashes)", async () => {
    const cell = templateCell({
      options: { code: `<button data-call="rules.run" data-args='not-json'>X</button>` },
    });
    const { container } = render(
      <WithDashboardCache ws="ws-tpl">
        <TemplateView cell={cell} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(container.querySelector("[data-call]")).not.toBeNull());
    const btn = container.querySelector<HTMLButtonElement>("[data-call]")!;
    fireEvent.click(btn);
    await waitFor(() => expect(btn.getAttribute("data-called")).toBe("err"));
    // No write fired for the malformed blob.
    expect(invokeMock.mock.calls.filter((c) => c[1]?.tool === "rules.run")).toHaveLength(0);
  });

  it("renders the standard denied panel when the source is denied (parity with every other view)", async () => {
    // No resolvable target → useVizQuery returns denied=true (the honest "no source" state).
    const cell: Cell = {
      ...templateCell(),
      sources: [{ refId: "A", tool: "", args: {}, datasource: { type: "surreal" } }],
      options: { code: "<p>you should not see the rows</p>" },
    };
    const { container } = render(
      <WithDashboardCache ws="ws-tpl">
        <TemplateView cell={cell} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(container.textContent).toMatch(/no access to this source/i));
    expect(container.querySelector("iframe")).toBeNull();
  });

  it("sanitizes author markup: a hostile onerror handler does NOT survive into the DOM", async () => {
    // Belt-and-braces at the view level: even though sanitizeTemplateHtml is unit-tested directly, this
    // proves the FULL pipeline (interpolate → sanitize → dangerouslySetInnerHTML) drops the handler.
    const cell = templateCell({
      options: { code: `<img src=x onerror="alert(1)"><p>{{rows.length}} rows</p>` },
    });
    const { container } = render(
      <WithDashboardCache ws="ws-tpl">
        <TemplateView cell={cell} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(container.textContent).toContain("rows"));
    const html = container.innerHTML.toLowerCase();
    expect(html).not.toContain("onerror");
    expect(html).not.toContain("alert(1)");
  });

  it("renders a Saved (templateId) template after the fetch resolves (Inline↔Saved both reach the DOM)", async () => {
    // Saved mode: no inline code, the body comes from `template.get`. The invoke stub returns it.
    invokeMock.mockImplementation(async (_c: string, args: { tool?: string }) => {
      const tool = args?.tool ?? "";
      if (tool === "template.get") return { id: "saved1", code: "<p>saved-body</p>" };
      if (tool === "viz.query") return { rows: [{ x: 1 }] };
      return {};
    });
    const cell = templateCell({
      options: { templateId: "saved1" },
    });
    // A Saved cell has no inline `code` key.
    (cell.options as Record<string, unknown>).code = undefined;
    const { container } = render(
      <WithDashboardCache ws="ws-tpl">
        <TemplateView cell={cell} />
      </WithDashboardCache>,
    );
    await waitFor(() => expect(container.textContent).toContain("saved-body"));
    expect(container.querySelector("iframe")).toBeNull();
  });
});
