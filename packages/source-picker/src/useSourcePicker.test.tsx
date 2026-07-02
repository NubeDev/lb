// Loader-hook tests with an INJECTED fake loader object (a pure function seam — permitted; NOT a fake
// backend). Proves: entries assemble from the loaders; a rejected loader → that group is empty
// (deny-tolerant); a host that omits a loader gets that group absent; the hook re-keys on `ws`.

import { describe, expect, it } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useSourcePicker } from "./useSourcePicker";
import type { SourceLoaders } from "./types";

const full: SourceLoaders = {
  listSeries: async () => ["a.b"],
  listExtensions: async () => [
    { ext: "p", enabled: true, widgets: [{ entry: "r.js", label: "Tile", icon: "x", scope: ["s.latest"] }] },
  ],
  listFlows: async () => [{ id: "f1", name: "F1" }],
  getFlow: async (id) => ({ id, name: "F1", nodes: [{ id: "n", type: "t" }] }),
  listFlowNodes: async () => [{ type: "t", outputs: ["state"] }],
  listDatasources: async () => [{ name: "pg", kind: "postgres" }],
};

describe("useSourcePicker", () => {
  it("assembles entries from every loader", async () => {
    const { result } = renderHook(() => useSourcePicker(full, "acme"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const groups = new Set(result.current.entries.map((e) => e.group));
    expect(groups).toContain("series");
    expect(groups).toContain("live");
    expect(groups).toContain("widget"); // the packaged tile
    expect(groups).toContain("flows"); // the output port
    expect(groups).toContain("sql"); // always offered
    expect(result.current.installed).toHaveLength(1);
  });

  it("a rejected loader yields an empty group, never a crash (deny-tolerant)", async () => {
    const loaders: SourceLoaders = {
      ...full,
      listSeries: async () => {
        throw new Error("denied");
      },
    };
    const { result } = renderHook(() => useSourcePicker(loaders, "acme"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.entries.some((e) => e.group === "series")).toBe(false);
    // other groups still present
    expect(result.current.entries.some((e) => e.group === "flows")).toBe(true);
  });

  it("an omitted loader yields that group absent (no getFlow → no flows)", async () => {
    const { result } = renderHook(() => useSourcePicker({ listSeries: async () => ["x"] }, "acme"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.entries.some((e) => e.group === "flows")).toBe(false);
    expect(result.current.entries.some((e) => e.group === "series")).toBe(true);
  });

  it("re-loads when the workspace changes", async () => {
    // A ws-derived series makes the re-key observable via `result.current`. The loader reads the CURRENT
    // `w` via a closure the host re-supplies each render — the hook keys on `ws`, reads loaders via a
    // ref, so a fresh loaders object per render does NOT loop (the host-stability guarantee is soft).
    const { result, rerender } = renderHook(({ w }) => useSourcePicker({ listSeries: async () => [`ws:${w}`] }, w), {
      initialProps: { w: "a" },
    });
    await waitFor(() => expect(result.current.entries.some((e) => e.label === "ws:a")).toBe(true));
    rerender({ w: "b" });
    await waitFor(() => expect(result.current.entries.some((e) => e.label === "ws:b")).toBe(true));
  });
});
