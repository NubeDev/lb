// ExtWidget ctx forwarding (thecrew finding 8). The renderer used to hardcode `options:{}`/`binding:{}`
// on the widget ctx, so a cell's `options.sceneId` never reached the tile (the extension worked around
// it via `ctx.vars`). This pins the fix: the cell's author-set `options`/`binding` are forwarded verbatim
// as `ctx.options`/`ctx.binding`. The remote mount is mocked (no gateway) to capture the ctx it receives;
// the WidgetBridge is real (the tile still reaches data only through it — options is inert config).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { ExtWidget } from "./ExtWidget";
import type { ExtRow } from "@/lib/ext/ext.api";

let captured: { workspace: string; options: unknown; binding: unknown; widgetId: string; el: HTMLElement } | null = null;
// Records how the tile's unmount was invoked. The nav-breaks bug was `ExtWidget` mounting the tile into
// the SAME container every effect-run and clearing it with `replaceChildren()`, so a StrictMode
// double-invoke / dep-change orphaned the prior React root's DOM → "removeChild: not a child" inside the
// shell's commit → nav wedged. We now assert the tile mounts into a private child SLOT and tears down once.
let unmountCalls = 0;
// A one-shot hook: when set, the NEXT mock mount calls it AFTER capturing but BEFORE returning its
// teardown — used to simulate a cleanup that fired while the async `mount()` was still awaiting.
let onMountEnter: (() => void) | null = null;

vi.mock("./federationWidget", () => ({
  loadRemoteWidgetMount: async () => {
    // A fake remote mount that records the ctx + the exact node it was handed (the assertion target).
    return (el: HTMLElement, ctx: { workspace: string; options: unknown; binding: unknown }, _bridge: unknown, widgetId: string) => {
      captured = { workspace: ctx.workspace, options: ctx.options, binding: ctx.binding, widgetId, el };
      el.textContent = "mounted";
      onMountEnter?.();
      return () => {
        unmountCalls += 1;
      };
    };
  },
}));

const thecrew: ExtRow = {
  ext: "thecrew",
  version: "0.1.0",
  tier: "wasm",
  enabled: true,
  running: true,
  health: "ok",
  restart_count: 0,
  ui: { entry: "remoteEntry.js", label: "Graphics", icon: "box", scope: ["assets.get_doc"] },
  widgets: [{ entry: "remoteEntry.js", label: "Scene", icon: "box", scope: ["series.latest"] }],
};

beforeEach(() => {
  captured = null;
  unmountCalls = 0;
  onMountEnter = null;
});

describe("ExtWidget — cell options/binding reach ctx (finding 8)", () => {
  it("forwards the cell's options.sceneId to ctx.options (no ctx.vars workaround needed)", async () => {
    render(
      <ExtWidget
        viewKey="ext:thecrew/scene"
        installed={[thecrew]}
        workspace="acme"
        options={{ sceneId: "scene:ahu-1" }}
        binding={{ series: "ahu1.speed" }}
      />,
    );
    await waitFor(() => expect(captured).not.toBeNull());
    expect(captured?.options).toEqual({ sceneId: "scene:ahu-1" });
    expect(captured?.binding).toEqual({ series: "ahu1.speed" });
    expect(captured?.widgetId).toBe("scene");
  });

  it("defaults to empty options/binding when the cell carries none", async () => {
    render(<ExtWidget viewKey="ext:thecrew/scene" installed={[thecrew]} workspace="acme" />);
    await waitFor(() => expect(captured).not.toBeNull());
    expect(captured?.options).toEqual({});
    expect(captured?.binding).toEqual({});
  });

  // Regression (thecrew graphics-canvas nav-breaks): the tile owns its own React root. ExtWidget must
  // mount it into a PRIVATE child slot (not the shared `data-ext-widget` container it also clears), so a
  // StrictMode double-invoke / dep-change remount can't orphan a live root's DOM — the orphaned root's
  // later `unmount()` was what threw "removeChild: not a child" inside the shell's commit and wedged nav.
  it("mounts the tile into a private child slot, not the shared container", async () => {
    const { container } = render(
      <ExtWidget viewKey="ext:thecrew/scene" installed={[thecrew]} workspace="acme" />,
    );
    await waitFor(() => expect(captured).not.toBeNull());

    const host = container.querySelector("[data-ext-widget]")?.querySelector("div");
    // The node handed to the tile's mount is a child OF the ref'd container, never the container itself —
    // that isolation is what stops one effect-run from wiping another run's live root.
    expect(captured?.el).toBeTruthy();
    expect(captured?.el).not.toBe(host);
    expect(host?.contains(captured!.el)).toBe(true);
  });

  it("tears the tile down exactly once on unmount", async () => {
    const { unmount } = render(
      <ExtWidget viewKey="ext:thecrew/scene" installed={[thecrew]} workspace="acme" />,
    );
    await waitFor(() => expect(captured).not.toBeNull());
    unmount();
    await waitFor(() => expect(unmountCalls).toBe(1));
    expect(unmountCalls).toBe(1); // never double-unmounted
  });

  // The async-mount-after-cleanup leak: if cleanup fires while `mount()` is still awaiting its remote,
  // the resolved root must still be torn down (else it leaks a live root whose later unmount throws).
  it("still tears down a tile whose mount resolves after cleanup already ran", async () => {
    // Unmount synchronously the moment the mock mount is entered — simulating cleanup racing the await.
    onMountEnter = () => result.unmount();
    const result = render(
      <ExtWidget viewKey="ext:thecrew/scene" installed={[thecrew]} workspace="acme" />,
    );
    await waitFor(() => expect(captured).not.toBeNull());
    // The root created during that racing mount is torn down immediately (alive===false branch), once.
    await waitFor(() => expect(unmountCalls).toBe(1));
    expect(unmountCalls).toBe(1);
  });
});
