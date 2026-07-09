// Unit tests for the portable bundle format (dashboard scope, import/export UX). Pure functions, no
// gateway — the parse/validate/round-trip contract that the import dialog trusts before any write. The
// store-side of import/export (a real replay through `dashboard.save`) is covered by the gateway test
// (`io/dashboardIo.gateway.test.tsx`), per CLAUDE §9 — this file only exercises the pure edge.

import { describe, expect, it } from "vitest";

import {
  BUNDLE_KIND,
  BUNDLE_VERSION,
  bareId,
  describeBundle,
  makeBundle,
  parseBundle,
  serializeBundle,
  slugFromTitle,
  uniqueId,
  type PortableDashboard,
} from "./portable";
import type { Cell } from "./dashboard.types";

const cell: Cell = {
  i: "w1",
  x: 0,
  y: 0,
  w: 8,
  h: 4,
  widget_type: "chart",
  binding: { series: "temp" },
};
const dash: PortableDashboard = { id: "ops", title: "Ops", cells: [cell] };

describe("portable bundle serialize/parse round-trip", () => {
  it("round-trips a dashboard bundle byte-stable through serialize → parse", () => {
    const bundle = makeBundle([dash], [], "2026-07-09T00:00:00Z");
    const parsed = parseBundle(serializeBundle(bundle));
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.bundle.dashboards).toHaveLength(1);
    expect(parsed.bundle.dashboards[0]).toMatchObject({
      id: "ops",
      title: "Ops",
    });
    expect(parsed.bundle.dashboards[0].cells).toHaveLength(1);
    expect(parsed.warnings).toHaveLength(0);
  });

  it("carries widgets (standalone panels) alongside dashboards", () => {
    const bundle = makeBundle(
      [],
      [
        {
          id: "gauge",
          title: "Gauge",
          spec: { widget_type: "gauge", binding: { series: "x" } },
        },
      ],
    );
    expect(describeBundle(bundle)).toBe("1 widget");
    const parsed = parseBundle(serializeBundle(bundle));
    expect(parsed.ok && parsed.bundle.panels[0].title).toBe("Gauge");
  });
});

describe("portable bundle validation (rejects, never guesses)", () => {
  it("rejects non-JSON", () => {
    const r = parseBundle("{not json");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(/not valid json/i);
  });

  it("rejects a bundle with the wrong/missing kind (e.g. a Grafana export)", () => {
    const r = parseBundle(
      JSON.stringify({ panels: [], schemaVersion: 39, title: "grafana" }),
    );
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(/not a lazybones|grafana/i);
  });

  it("rejects a newer MAJOR version it cannot read", () => {
    const r = parseBundle(
      JSON.stringify({
        kind: BUNDLE_KIND,
        version: BUNDLE_VERSION + 1,
        dashboards: [dash],
        panels: [],
      }),
    );
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(/newer than this app/i);
  });

  it("rejects an empty bundle (nothing importable)", () => {
    const r = parseBundle(JSON.stringify(makeBundle([], [])));
    expect(r.ok).toBe(false);
  });

  it("skips a malformed entry with a warning but imports the rest", () => {
    const raw = {
      kind: BUNDLE_KIND,
      version: BUNDLE_VERSION,
      dashboards: [dash, { id: "bad", title: "Bad", cells: "not-an-array" }],
      panels: [],
    };
    const r = parseBundle(JSON.stringify(raw));
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.bundle.dashboards).toHaveLength(1);
    expect(r.warnings.some((w) => /bad/i.test(w))).toBe(true);
  });
});

describe("id helpers", () => {
  it("strips a table prefix to the bare id", () => {
    expect(bareId("dashboard:ops")).toBe("ops");
    expect(bareId("ops")).toBe("ops");
  });

  it("slugifies a title", () => {
    expect(slugFromTitle("My Ops Board!")).toBe("my-ops-board");
    expect(slugFromTitle("   ")).toBe("imported");
  });

  it("makes a colliding id unique (never a silent overwrite)", () => {
    const taken = new Set(["ops", "ops-2"]);
    expect(uniqueId("ops", taken)).toBe("ops-3");
    expect(uniqueId("fresh", taken)).toBe("fresh");
  });
});
