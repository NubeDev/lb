// Proves the LAZY per-section contract end-to-end against the real gateway: the explorer verbs
// (`listChannels`, `listSeries`, `listDatasources`, …) MUST NOT fire when SourcesPane mounts; they
// fire ONLY when the user expands the matching section in the CatalogExplorer tree. This is the
// "don't call the API on page load, only on toggle" contract — the test spies on `ipc.invoke` (the
// ONE seam every read goes through) and asserts the counts at each phase.

import { describe, expect, it, beforeAll, afterAll, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import { DataStudioView } from "@/features/data-studio/DataStudioView";
import { useRealGateway, signInReal, seedIotDemo } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";
import * as ipc from "@/lib/ipc/invoke";

beforeAll(() => useRealGateway());

const realGetRect = HTMLElement.prototype.getBoundingClientRect;
beforeAll(() => {
  HTMLElement.prototype.getBoundingClientRect = function () {
    return new DOMRect(0, 0, 1280, 800);
  };
});
afterAll(() => {
  HTMLElement.prototype.getBoundingClientRect = realGetRect;
});

/** Spy on `ipc.invoke` and count per command (`channel_list`, `series_list`, `datasource_list`,
 *  `read_schema`, `insight_list`, …) — the explorer verbs are direct invokes, not `mcp_call`, so we
 *  count by the COMMAND name. Delegates to the real transport (observe, never fake — rule 9). */
function spyInvoke() {
  const real = ipc.invoke;
  const byCmd = new Map<string, number>();
  const spy = vi
    .spyOn(ipc, "invoke")
    .mockImplementation(((cmd: string, args?: Record<string, unknown>) => {
      byCmd.set(cmd, (byCmd.get(cmd) ?? 0) + 1);
      // Also index mcp_call by its tool arg (for verbs routed through the MCP bridge).
      if (cmd === "mcp_call") {
        const tool = (args?.tool as string) ?? "?";
        byCmd.set(tool, (byCmd.get(tool) ?? 0) + 1);
      }
      return real(cmd, args);
    }) as typeof ipc.invoke);
  return {
    count: (key: string) => byCmd.get(key) ?? 0,
    restore: () => spy.mockRestore(),
  };
}

describe("Data Studio SourcesPane — lazy per-section API calls (real gateway)", () => {
  it("mounting the studio does NOT fire listChannels / listSeries / listDatasources", async () => {
    await signInReal("user:ada", "lazy-1");
    await seedIotDemo();

    const spy = spyInvoke();
    try {
      const s = getSession();
      render(
        <RoutingContextProvider
          value={{
            workspace: "lazy-1",
            principal: s?.principal ?? "",
            caps: s?.caps,
            allowed: ["data-studio"],
            extPages: [],
            extPagesLoading: false,
            onSignOut: () => {},
            switchWorkspace: () => {},
          }}
        >
          <DataStudioView ws="lazy-1" />
        </RoutingContextProvider>,
      );
      // The Sources rail tab mounts + the lazy CatalogExplorer loads + the section headers render.
      // Wait for the Channels section header to be present — that's the "page load" point.
      await waitFor(
        () => expect(screen.getByLabelText("toggle section Channels")).toBeInTheDocument(),
        { timeout: 8000 },
      );
      // The lazy contract: NO explorer verbs fired on mount. The sections are present (idle) but the
      // API calls haven't gone out — the user hasn't expanded anything yet.
      expect(spy.count("channel_list")).toBe(0);
      expect(spy.count("series_list")).toBe(0);
      expect(spy.count("datasource_list")).toBe(0);
      expect(spy.count("store.schema")).toBe(0);
      expect(spy.count("insight.list")).toBe(0);
    } finally {
      spy.restore();
    }
  }, 30000);

  it("expanding the Channels section fires `channel_list` exactly once; collapsing/re-expanding does NOT refire", async () => {
    await signInReal("user:ada", "lazy-2");
    await seedIotDemo();

    const spy = spyInvoke();
    try {
      const s = getSession();
      render(
        <RoutingContextProvider
          value={{
            workspace: "lazy-2",
            principal: s?.principal ?? "",
            caps: s?.caps,
            allowed: ["data-studio"],
            extPages: [],
            extPagesLoading: false,
            onSignOut: () => {},
            switchWorkspace: () => {},
          }}
        >
          <DataStudioView ws="lazy-2" />
        </RoutingContextProvider>,
      );
      // Wait for page load (Channels header present, no API call yet).
      const channelsToggle = await screen.findByLabelText("toggle section Channels", {}, { timeout: 8000 });
      expect(spy.count("channel_list")).toBe(0);
      // Toggle open — the loader fires once.
      fireEvent.click(channelsToggle);
      await waitFor(() => expect(spy.count("channel_list")).toBe(1));
      // Collapse (toggle again) — no extra call.
      fireEvent.click(screen.getByLabelText("toggle section Channels"));
      await new Promise((r) => setTimeout(r, 200));
      expect(spy.count("channel_list")).toBe(1);
      // Re-expand — the cached state persists, NO refire (idempotent).
      fireEvent.click(screen.getByLabelText("toggle section Channels"));
      await new Promise((r) => setTimeout(r, 300));
      expect(spy.count("channel_list")).toBe(1);
    } finally {
      spy.restore();
    }
  }, 30000);
});
