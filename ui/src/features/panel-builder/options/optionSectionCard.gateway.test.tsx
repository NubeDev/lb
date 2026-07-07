// The OptionSectionCard contract (panel-wizard scope, redesigned per resolved decision #3): the reusable
// per-option ROW composes the registry `Control` over the REAL `EditorState`, renders NO chart of its own
// (the ONE pinned OptionFocusPreview is the only render surface), reports focus upward via `onFocus`, and
// a DEAD option (per `optionLiveness`) shows the honest "renderer pending" note.
//
// Real gateway session (rule 9) — the row itself renders no data, but it runs in the same real-session
// harness as the rest of the wizard suite (no fakes, no parallel test stack).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, cleanup, fireEvent } from "@testing-library/react";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import type { Cell, View } from "@/lib/dashboard";
import { cellToEditorState } from "@/lib/panel-kit/cellEditorState";
import { optionById } from "@/features/panel-builder/options/registry";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { OptionSectionCard } from "@/features/panel-builder/options/OptionSectionCard";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `sectcard-${n++}`;

function baseCell(view: View, series: string): Cell {
  return {
    i: "c",
    x: 0,
    y: 0,
    w: 6,
    h: 4,
    v: 3,
    widget_type: view === "timeseries" ? "chart" : "stat",
    view,
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options: view === "stat"
      ? { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" }
      : {},
  };
}

describe("OptionSectionCard — one option row over the real EditorState (no chart of its own)", () => {
  it("renders the control + LIVE classification, and mounts NO preview (one-preview design)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const state = cellToEditorState(baseCell("stat", "s.live"));
    const def = optionById("decimals")!;
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionSectionCard def={def} view="stat" state={state} patch={() => {}} />
      </WithDashboardCache>,
    );
    expect(screen.getByLabelText("Decimals")).toBeInTheDocument();
    const row = container.querySelector('[data-option-id="decimals"]');
    expect(row?.getAttribute("data-live")).toBe("true");
    // The headline of the redesign: the row renders NO chart — no option-focus preview, no widget view.
    expect(container.querySelector(".option-focus-preview")).toBeNull();
    expect(container.querySelector("svg.recharts-surface")).toBeNull();
    // No dead-option note for a LIVE option.
    expect(container.querySelector('[role="note"]')).toBeNull();
    unmount();
    cleanup();
  });

  it("DEAD option (custom.spanNulls on timeseries): the row shows the 'renderer pending' note", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const state = cellToEditorState(baseCell("timeseries", "s.dead"));
    const def = optionById("custom.spanNulls")!;
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionSectionCard def={def} view="timeseries" state={state} patch={() => {}} />
      </WithDashboardCache>,
    );
    expect(container.querySelector('[data-option-id="custom.spanNulls"]')?.getAttribute("data-live")).toBe("false");
    expect(container.querySelector('[role="note"]')?.textContent).toMatch(/no visible effect/i);
    unmount();
    cleanup();
  });

  it("writing through the row's Control updates the EditorState via writeOption (no row-local state)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const state = cellToEditorState(baseCell("stat", "s.write"));
    const def = optionById("decimals")!;
    let captured: EditorState | undefined;
    const { unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionSectionCard
          def={def}
          view="stat"
          state={state}
          patch={(next) => {
            captured = { ...state, ...next };
          }}
        />
      </WithDashboardCache>,
    );
    const input = screen.getByLabelText("Decimals") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "3" } });
    expect(captured?.fieldConfig?.defaults?.decimals).toBe(3);
    unmount();
    cleanup();
  });

  it("focusing the row's control reports the option upward via onFocus (the pinned-preview seam)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const state = cellToEditorState(baseCell("stat", "s.focus"));
    const def = optionById("decimals")!;
    let reported: string | undefined;
    const { unmount } = render(
      <WithDashboardCache ws={ws}>
        <OptionSectionCard
          def={def}
          view="stat"
          state={state}
          patch={() => {}}
          onFocus={(id) => {
            reported = id;
          }}
        />
      </WithDashboardCache>,
    );
    fireEvent.focus(screen.getByLabelText("Decimals"));
    expect(reported).toBe("decimals");
    unmount();
    cleanup();
  });
});
