// Live re-theme fan-out (theme-appearance scope, slice 6). A canvas-style widget that returns an
// `{ update, teardown }` handle must receive `update(ctx)` with a FRESH `ctx.theme` on a theme change —
// no re-mount (mount called once). This is the whole point of ctx.theme v4: an ECharts tile recolors in
// place. Driven through the REAL ThemeProvider + the REAL emitter (no fake theme layer).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor, act } from "@testing-library/react";

import { ExtWidget } from "./ExtWidget";
import { ThemeProvider, useTheme } from "@/lib/theme";
import type { ExtRow } from "@/lib/ext/ext.api";

let mountCount = 0;
const updates: Array<Record<string, unknown> | undefined> = [];

vi.mock("./federationWidget", () => ({
  loadRemoteWidgetMount: async () => {
    // A v4 canvas tile: returns an update handle (like echarts' mountChart) that records each ctx.theme.
    return (el: HTMLElement, ctx: { theme?: Record<string, unknown> }) => {
      mountCount += 1;
      updates.push(ctx.theme);
      el.textContent = "chart";
      return {
        update: (next: { theme?: Record<string, unknown> }) => updates.push(next.theme),
        teardown: () => {},
      };
    };
  },
}));

const ext: ExtRow = {
  ext: "echarts-panel",
  version: "0.1.0",
  tier: "wasm",
  enabled: true,
  running: true,
  health: "ok",
  restart_count: 0,
  ui: { entry: "remoteEntry.js", label: "Chart", icon: "chart", scope: [] },
  widgets: [{ entry: "remoteEntry.js", label: "Chart", icon: "chart", scope: [], data: true }],
};

// A control that changes an INLINE-written token (radius) so the applied theme — and thus ctx.theme —
// visibly changes in jsdom. (Built-in preset colors come from the globals.css `.dark` block, which jsdom
// doesn't load; radius is written inline by theme-dom, so it's the reliable observable here. The color
// path is covered against inline custom palettes in resolve-theme-tokens.test.ts.)
function ThemeDriver() {
  const { setRadius } = useTheme();
  return (
    <button type="button" onClick={() => setRadius("1rem")}>
      grow
    </button>
  );
}

beforeEach(() => {
  mountCount = 0;
  updates.length = 0;
});

describe("ExtWidget — live re-theme via ctx.theme (v4)", () => {
  it("pushes a fresh ctx.theme through update() on a theme change, without re-mounting", async () => {
    const { getByText } = render(
      <ThemeProvider>
        <ThemeDriver />
        <ExtWidget viewKey="ext:echarts-panel/chart" installed={[ext]} workspace="acme" />
      </ThemeProvider>,
    );

    await waitFor(() => expect(mountCount).toBe(1));
    const initialRadius = updates[0]?.radius;
    expect(initialRadius).toBe("0.5rem"); // DEFAULT_THEME radius, written inline

    // Change radius → theme-dom re-applies + emits → useThemeTokens re-resolves → ctx changes → update.
    await act(async () => {
      getByText("grow").click();
    });

    await waitFor(() => expect(updates.length).toBeGreaterThan(1));
    // The tile was mounted exactly once — the re-theme rode `update`, not a re-mount.
    expect(mountCount).toBe(1);
    // The latest ctx.theme reflects the new radius, different from the initial.
    expect(updates[updates.length - 1]?.radius).toBe("1rem");
    expect(updates[updates.length - 1]?.radius).not.toBe(initialRadius);
  });
});
