// Extension pages in the shell (ui-federation scope). Two guarantees:
//   1. an installed extension that declares a `[ui]` page shows a cap-gated nav slot (the shell builds
//      it from `ext.list`), so a real page becomes reachable from the sidebar;
//   2. the host-mediated bridge forwards ONLY the extension's granted read-only tools and rejects
//      anything out of scope — the page is a gated caller, never a trusted decider.
// The bundle dynamic-import itself is exercised in the browser/gateway path; jsdom can't load a remote
// ESM, so here we assert the slot + the host container render and the bridge's scope filter.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { App } from "../../App";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { setSession } from "@/lib/session/session.store";
import { __resetExtFake, __seedExt } from "@/lib/ipc/ext.fake";
import { makeBridge } from "./bridge";

const HELLO_UI = {
  ext: "hello-ui",
  version: "v1",
  tier: "wasm" as const,
  enabled: true,
  ui: { entry: "entry.mjs", label: "Hello UI", icon: "puzzle", scope: ["series.find"] },
};

beforeEach(() => {
  setSession(null);
  __resetExtFake();
});
afterEach(() => {
  setSession(null);
  __resetExtFake();
});

describe("extension pages (ui-federation)", () => {
  it("shows a sidebar slot for an extension that declares a [ui] page", async () => {
    setSession({ token: "t", principal: "user:ada", workspace: "acme", caps: ADMIN_CAPS });
    __seedExt(HELLO_UI);
    render(<App />);
    // The page's label becomes a cap-gated nav slot.
    expect(await screen.findByLabelText("Hello UI")).toBeInTheDocument();
  });

  it("does NOT show a slot for an extension with no [ui] page", async () => {
    setSession({ token: "t", principal: "user:ada", workspace: "acme", caps: ADMIN_CAPS });
    __seedExt({ ext: "hello", version: "v2", tier: "wasm", enabled: true }); // no ui
    render(<App />);
    expect(await screen.findByLabelText("Channels")).toBeInTheDocument();
    expect(screen.queryByLabelText("hello")).not.toBeInTheDocument();
  });
});

describe("the host-mediated bridge", () => {
  it("forwards an in-scope tool and rejects an out-of-scope one", async () => {
    const bridge = makeBridge(["series.find"]);
    // In scope → forwards (the test-mode fake answers series.* with [] — honest 'no data', not a mock).
    await expect(bridge.call("series.find", { tags: [] })).resolves.toEqual([]);
    // Out of scope → rejected locally (the host would deny it too).
    await expect(bridge.call("series.delete", {})).rejects.toThrow(/out_of_scope/);
    await expect(bridge.call("dashboard.delete", {})).rejects.toThrow(/out_of_scope/);
  });
});
