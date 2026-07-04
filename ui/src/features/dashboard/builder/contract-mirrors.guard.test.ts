// Contract-mirror lockstep guard (theme-appearance scope, slice 6). The widget ctx contract lives in
// THREE mirrors that MUST move together; `ctx.theme` (v4) must be present in all of them or a
// devkit-scaffolded / extension-copied widget drifts from the host. jsdom can't build the extensions, so
// this asserts on the SOURCE of each mirror: the version is 4 and each carries a `theme` field.
//
//   1. host type      — ui/src/features/dashboard/builder/federationWidget.ts (WidgetCtx.theme)
//   2. host builder    — ui/src/features/dashboard/builder/ExtWidget.tsx (WIDGET_CTX_V = 4)
//   3. devkit template — rust/crates/devkit/templates/ui/src_contract.ts.tmpl (WidgetCtx.theme)
// Extension copies (echarts ChartCtx.theme, thecrew WidgetCtx.theme) are checked too.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const UI = join(__dirname, "..", "..", "..", "..");
const REPO = join(UI, "..");

const read = (p: string) => readFileSync(p, "utf8");

describe("widget ctx contract — v4 theme mirror lockstep", () => {
  it("host builder pins WIDGET_CTX_V = 4", () => {
    const src = read(join(__dirname, "ExtWidget.tsx"));
    expect(src).toMatch(/WIDGET_CTX_V\s*=\s*4\b/);
  });

  it("host type carries WidgetCtx.theme + a WidgetTheme shape", () => {
    const src = read(join(__dirname, "federationWidget.ts"));
    expect(src).toMatch(/theme\?:\s*WidgetTheme/);
    expect(src).toMatch(/interface WidgetTheme/);
  });

  it("devkit template carries WidgetCtx.theme (v4)", () => {
    const src = read(join(REPO, "rust/crates/devkit/templates/ui/src_contract.ts.tmpl"));
    expect(src).toMatch(/theme\?:\s*WidgetTheme/);
    expect(src).toMatch(/`4`/); // the version note
  });

  it("echarts-panel copy consumes ctx.theme (ChartCtx.theme + ChartTheme)", () => {
    const src = read(join(REPO, "rust/extensions/echarts-panel/ui/src/chart/mountChart.ts"));
    expect(src).toMatch(/theme\?:\s*ChartTheme/);
    expect(src).toMatch(/interface ChartTheme/);
  });

  it("thecrew copy carries the v4 theme field (mirror stays in lockstep)", () => {
    const src = read(join(REPO, "rust/extensions/thecrew/ui/src/bridge/contract.ts"));
    expect(src).toMatch(/theme\?:\s*\{/);
    expect(src).toMatch(/v\?:\s*number/);
  });
});
