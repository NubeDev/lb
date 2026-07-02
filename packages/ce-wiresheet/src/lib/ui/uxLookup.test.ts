import { describe, it, expect } from "vitest";
import { findComponentUx } from "./uxLookup";
import type { ExtensionUi } from "./types";

const EXTS: ExtensionUi[] = [
  {
    id: "ce", label: "CE", uis: [
      { id: "table", label: "Table", selection: "sync", view: { type: "collection", source: "components" } },
    ],
  },
  {
    id: "scheduler", label: "Scheduler", uis: [
      { id: "schedule", label: "Schedule", selection: "ignore", view: { type: "layout", children: [{ type: "tabbedEditor", fullType: "NubeIO-schedule::schedule" }] } },
      { id: "cron", label: "Cron", selection: "ignore", view: { type: "layout", children: [{ type: "tabbedEditor", fullType: "NubeIO-schedule::cron" }] } },
    ],
  },
  {
    id: "js", label: "JS", uis: [
      { id: "js-components", label: "Components", selection: "ignore", view: { type: "layout", children: [{ type: "jsComponents", fullType: "NubeIO-js::jsLogic" }] } },
      { id: "js-scripts", label: "Scripts", selection: "ignore", view: { type: "layout", children: [{ type: "jsScripts", fullType: "NubeIO-js::jsLogic" }] } },
    ],
  },
];

describe("findComponentUx", () => {
  it("matches a scheduler component to its tabbedEditor UI (exact full type)", () => {
    expect(findComponentUx(EXTS, "NubeIO-schedule::schedule")).toEqual({ extId: "scheduler", uiId: "schedule" });
    expect(findComponentUx(EXTS, "NubeIO-schedule::cron")).toEqual({ extId: "scheduler", uiId: "cron" });
  });
  it("matches a jsLogic component to the Components UI, not Scripts (per-component widget only)", () => {
    expect(findComponentUx(EXTS, "NubeIO-js::jsLogic")).toEqual({ extId: "js", uiId: "js-components" });
  });
  it("matches by last type segment when the vendor prefix differs", () => {
    expect(findComponentUx(EXTS, "Other-pkg::cron")).toEqual({ extId: "scheduler", uiId: "cron" });
  });
  it("returns null for a type with no per-component UI", () => {
    expect(findComponentUx(EXTS, "NubeIO-math::add")).toBeNull();
    expect(findComponentUx(EXTS, undefined)).toBeNull();
  });
  it("ignores type-agnostic views (collection/table)", () => {
    const onlyTable: ExtensionUi[] = [{ id: "ce", label: "CE", uis: [EXTS[0].uis[0]] }];
    expect(findComponentUx(onlyTable, "anything::x")).toBeNull();
  });
});
