// Contract-mirror lockstep guard (theme-appearance scope slice 6; updated by ext-out-of-tree slice 2).
// The widget ctx contract used to live in THREE hand-kept mirrors. After the SDK extraction it lives in
// ONE authoritative source — the standalone `@nube/ext-ui-sdk` package — and the former host mirror now
// IMPORTS from it. `ctx.theme` (v4) must still be present in the authoritative source, and the host must
// resolve to it (import) rather than redefine it. jsdom can't build the extensions, so this asserts on
// the SOURCE of each place:
//
//   1. authoritative — lb-ext-ui-sdk/src/widget.ts (WidgetCtx.theme + WidgetTheme, version 4)
//   2. host type      — federationWidget.ts imports from the package (the mirror collapsed — no redefine)
//   3. host builder   — ExtWidget.tsx pins WIDGET_CTX_V = 4
//   4. devkit template — rust/crates/devkit/templates/ui/src_contract.ts.tmpl (WidgetCtx.theme)
// Extension copies (echarts ChartCtx.theme, thecrew WidgetCtx.theme) still carry the v4 field until they
// migrate to the package import as they move to lb-extensions.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const UI = join(__dirname, "..", "..", "..", "..");
const REPO = join(UI, "..");
// The standalone UI-SDK repo is a sibling of `lb` (the family lives together under rust/).
const UI_SDK = join(REPO, "..", "lb-ext-ui-sdk");

const read = (p: string) => readFileSync(p, "utf8");

describe("widget ctx contract — v4 theme, single authoritative source", () => {
  it("authoritative @nube/ext-ui-sdk carries WidgetCtx.theme + a WidgetTheme shape at v4", () => {
    const src = read(join(UI_SDK, "src", "widget.ts"));
    expect(src).toMatch(/theme\?:\s*WidgetTheme/);
    expect(src).toMatch(/interface WidgetTheme/);
    expect(src).toMatch(/WIDGET_CONTRACT_VERSION\s*=\s*4\b/);
  });

  it("host type imports the contract from the package (the mirror collapsed)", () => {
    const src = read(join(__dirname, "federationWidget.ts"));
    expect(src).toMatch(/from ["']@nube\/ext-ui-sdk["']/);
    expect(src).toMatch(/WidgetCtx/);
    // It must NOT redefine the shape — that would resurrect the mirror it just killed.
    expect(src).not.toMatch(/interface WidgetTheme\s*\{/);
  });

  it("host builder pins WIDGET_CTX_V = 4", () => {
    const src = read(join(__dirname, "ExtWidget.tsx"));
    expect(src).toMatch(/WIDGET_CTX_V\s*=\s*4\b/);
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
