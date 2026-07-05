// `useCatalog` hook tests with an INJECTED fake loader object (a pure function seam — permitted;
// NOT a fake backend, system-catalog scope testing plan). Proves the per-section contract:
//   - a denied loader → `{status:"denied"}` for that section (NOT empty-ready);
//   - an absent loader → the section is `undefined` (absent section, not denied);
//   - a ready loader → `{status:"ready", data}`;
//   - re-key on workspace (`ws`).
// Plus the projection invariant: the picker's deny→empty collapse is consistent with the catalog's
// visible tri-state (one orchestration).

import { describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { useCatalog } from "./useCatalog";
import { loadCatalog } from "./loadCatalog";
import { loadSourcePicker } from "./loadSourcePicker";
import type { SourceLoaders } from "./types";

const full: SourceLoaders = {
  listSeries: async () => ["a.b"],
  listExtensions: async () => [
    { ext: "p", enabled: true, widgets: [{ entry: "r.js", label: "Tile", icon: "x", scope: ["s.latest"] }] },
  ],
  listFlows: async () => [{ id: "f1", name: "F1" }],
  getFlow: async (id) => ({ id, name: "F1", nodes: [{ id: "n", type: "t" }] }),
  listFlowNodes: async () => [{ type: "t", outputs: ["state"] }],
  listDatasources: async () => [{ name: "pg", kind: "postgres", endpoint: "db:5432" }],
  listRules: async () => [{ id: "r1", name: "Hourly mean" }],
  readSchema: async () => ({ tables: [{ name: "device", columns: [{ name: "id", type: "string" }] }] }),
  listChannels: async () => [{ id: "general" }],
  listInsights: async () => [{ id: "i1", title: "AHU 2 anomaly", severity: "warning", status: "open" }],
  listInbox: async () => [{ id: "x1", channel: "general" }],
};

describe("loadCatalog", () => {
  it("every wired loader resolves to ready; absent loaders stay absent", async () => {
    const cat = await loadCatalog(full);
    expect(cat.series?.status).toBe("ready");
    expect(cat.series).toEqual({ status: "ready", data: ["a.b"] });
    expect(cat.datasources).toEqual({ status: "ready", data: [{ name: "pg", kind: "postgres", endpoint: "db:5432" }] });
    expect(cat.schema).toEqual({
      status: "ready",
      data: { tables: [{ name: "device", columns: [{ name: "id", type: "string" }] }] },
    });
    expect(cat.channels).toEqual({ status: "ready", data: [{ id: "general" }] });
    expect(cat.insights).toEqual({
      status: "ready",
      data: [{ id: "i1", title: "AHU 2 anomaly", severity: "warning", status: "open" }],
    });
    expect(cat.inbox).toEqual({ status: "ready", data: [{ id: "x1", channel: "general" }] });
  });

  it("a denied loader yields {status:'denied'} — never a fabricated roster", async () => {
    const loaders: SourceLoaders = {
      ...full,
      listChannels: async () => {
        throw new Error("denied");
      },
    };
    const cat = await loadCatalog(loaders);
    expect(cat.channels?.status).toBe("denied");
    // Other sections still resolve normally.
    expect(cat.series?.status).toBe("ready");
    expect(cat.insights?.status).toBe("ready");
  });

  it("an absent loader yields an absent (undefined) section, not denied", async () => {
    const cat = await loadCatalog({ readSchema: full.readSchema! });
    expect(cat.schema?.status).toBe("ready");
    expect(cat.series).toBeUndefined();
    expect(cat.channels).toBeUndefined();
  });

  it("each new loader surfaces its row shape verbatim (the host's row typing is the contract)", async () => {
    const cat = await loadCatalog({
      readSchema: async () => ({ tables: [{ name: "t", columns: [{ name: "c", type: "string" }] }] }),
      listChannels: async () => [{ id: "ops" }, { id: "alerts" }],
      listInsights: async () => [{ id: "i", title: "T", severity: "info" }],
      listInbox: async () => [{ id: "m1", channel: "ops" }],
    });
    expect(cat.schema?.status).toBe("ready");
    expect(cat.channels).toEqual({ status: "ready", data: [{ id: "ops" }, { id: "alerts" }] });
    expect(cat.insights).toEqual({ status: "ready", data: [{ id: "i", title: "T", severity: "info" }] });
    expect(cat.inbox).toEqual({ status: "ready", data: [{ id: "m1", channel: "ops" }] });
    // Absent (no listSeries) ⇒ undefined, not denied — the host didn't wire that section.
    expect(cat.series).toBeUndefined();
  });

  it("each new section's deny is independent (one denied ⇒ only that section denies)", async () => {
    const cat = await loadCatalog({
      readSchema: async () => {
        throw new Error("denied");
      },
      listChannels: async () => {
        throw new Error("denied");
      },
      listInsights: async () => [{ id: "i", title: "T" }],
      listInbox: async () => [{ id: "m1", channel: "ops" }],
    });
    expect(cat.schema?.status).toBe("denied");
    expect(cat.channels?.status).toBe("denied");
    expect(cat.insights?.status).toBe("ready");
    expect(cat.inbox?.status).toBe("ready");
  });
});

describe("useCatalog (lazy per-section loads)", () => {
  it("starts every wired section as `idle` — no loader fires on mount (the lazy contract)", async () => {
    const loaders: SourceLoaders = { listSeries: async () => [`a.b`] };
    const { result } = renderHook(() => useCatalog(loaders, "acme"));
    // The Series section exists (loader wired) but is `idle` — no API call went out yet.
    expect(result.current.sections.series?.status).toBe("idle");
    // Sections the host didn't wire stay absent (undefined), not idle.
    expect(result.current.sections.channels).toBeUndefined();
  });

  it("loadSection(kind) fires the section's loader and resolves to ready", async () => {
    const listChannels = vi.fn(async () => [{ id: "general" }]);
    const loaders: SourceLoaders = { listChannels };
    const { result } = renderHook(() => useCatalog(loaders, "acme"));
    expect(result.current.sections.channels?.status).toBe("idle");
    expect(listChannels).not.toHaveBeenCalled(); // not fired on mount
    // The user expands the Channels section.
    act(() => result.current.loadSection("channels"));
    expect(result.current.sections.channels?.status).toBe("loading");
    expect(listChannels).toHaveBeenCalledTimes(1); // fired on expand
    await waitFor(() => expect(result.current.sections.channels?.status).toBe("ready"));
    expect(result.current.sections.channels).toEqual({ status: "ready", data: [{ id: "general" }] });
  });

  it("loadSection is idempotent — re-calling on a loaded section does not refire the loader", async () => {
    const listSeries = vi.fn(async () => [`a.b`]);
    const { result } = renderHook(() => useCatalog({ listSeries }, "acme"));
    act(() => result.current.loadSection("series"));
    await waitFor(() => expect(result.current.sections.series?.status).toBe("ready"));
    expect(listSeries).toHaveBeenCalledTimes(1);
    // Re-call (e.g. a collapse/re-expand) — the cached state persists, no second API call.
    act(() => result.current.loadSection("series"));
    expect(listSeries).toHaveBeenCalledTimes(1);
  });

  it("a denied loader surfaces {status:'denied'} on expand (the picker collapses this to empty)", async () => {
    const listChannels = vi.fn(async () => {
      throw new Error("denied");
    });
    const { result } = renderHook(() => useCatalog({ listChannels }, "acme"));
    act(() => result.current.loadSection("channels"));
    await waitFor(() => expect(result.current.sections.channels?.status).toBe("denied"));
    // The picker projection of the same state:
    const picker = await loadSourcePicker({ ...full, listChannels });
    expect(picker.entries.some((e) => e.group === "series")).toBe(true);
  });

  it("re-keys when the workspace changes — ws switch resets every section to `idle`", async () => {
    const listSeries = vi.fn(async () => [`ws:hook`]);
    const { result, rerender } = renderHook(({ w }) => useCatalog({ listSeries }, w), {
      initialProps: { w: "a" },
    });
    // Load under ws-A.
    act(() => result.current.loadSection("series"));
    await waitFor(() => expect(result.current.sections.series?.status).toBe("ready"));
    expect(listSeries).toHaveBeenCalledTimes(1);
    // Switch to ws-B — the prior data is dropped (every section back to `idle`).
    rerender({ w: "b" });
    await waitFor(() => expect(result.current.sections.series?.status).toBe("idle"));
    // Re-expand under ws-B fires the loader again (different workspace, fresh data).
    act(() => result.current.loadSection("series"));
    await waitFor(() => expect(result.current.sections.series?.status).toBe("ready"));
    expect(listSeries).toHaveBeenCalledTimes(2);
  });
});
