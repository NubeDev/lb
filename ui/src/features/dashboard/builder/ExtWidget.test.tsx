// ExtWidget ctx forwarding (thecrew finding 8). The renderer used to hardcode `options:{}`/`binding:{}`
// on the widget ctx, so a cell's `options.sceneId` never reached the tile (the extension worked around
// it via `ctx.vars`). This pins the fix: the cell's author-set `options`/`binding` are forwarded verbatim
// as `ctx.options`/`ctx.binding`. The remote mount is mocked (no gateway) to capture the ctx it receives;
// the WidgetBridge is real (the tile still reaches data only through it — options is inert config).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { ExtWidget } from "./ExtWidget";
import type { ExtRow } from "@/lib/ext/ext.api";

let captured: { workspace: string; options: unknown; binding: unknown; widgetId: string } | null = null;

vi.mock("./federationWidget", () => ({
  loadRemoteWidgetMount: async () => {
    // A fake remote mount that records the ctx the renderer built (the assertion target).
    return (el: HTMLElement, ctx: { workspace: string; options: unknown; binding: unknown }, _bridge: unknown, widgetId: string) => {
      captured = { workspace: ctx.workspace, options: ctx.options, binding: ctx.binding, widgetId };
      el.textContent = "mounted";
      return () => {};
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
});
