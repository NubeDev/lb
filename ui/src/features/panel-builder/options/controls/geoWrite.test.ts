// The geo-place write (weather scope): picking a searched city writes label + lat/lon as ONE options
// patch. Pure — no network, no React (the live geocoding fetch is exercised in the browser, not here;
// it's the one sanctioned external boundary, rule 9). One responsibility: the writeGeoPlace contract.

import { describe, expect, it } from "vitest";
import { writeGeoPlace } from "../binding";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";

const base = { options: {} } as unknown as EditorState;

describe("writeGeoPlace", () => {
  it("sets label + lat + lon from a picked place in one patch", () => {
    const patch = writeGeoPlace(base, { label: "Brisbane, Queensland, AU", lat: -27.47, lon: 153.02 });
    expect(patch.options).toMatchObject({ label: "Brisbane, Queensland, AU", lat: -27.47, lon: 153.02 });
  });

  it("overwrites a prior location wholesale (new pick replaces old coordinates)", () => {
    const prev = { options: { label: "Brisbane, Queensland, AU", lat: -27.47, lon: 153.02 } } as unknown as EditorState;
    const patch = writeGeoPlace(prev, { label: "London, England, GB", lat: 51.5, lon: -0.13 });
    expect(patch.options).toMatchObject({ label: "London, England, GB", lat: 51.5, lon: -0.13 });
  });

  it("clears the label when a place has an empty label but keeps its coordinates", () => {
    const patch = writeGeoPlace(base, { label: "", lat: 10, lon: 20 });
    const opts = patch.options as Record<string, unknown>;
    expect(opts.label).toBeUndefined();
    expect(opts.lat).toBe(10);
    expect(opts.lon).toBe(20);
  });
});
