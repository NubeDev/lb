import { describe, expect, it } from "vitest";

import { mount } from "@/mount";
import { stubBridge } from "@/test/bridge.stub";

// The page's appliance.list runs in an effect → a microtask → a setState; under React 18 createRoot +
// StrictMode (double-invoke) that settles over a few macrotask turns. Flush a handful so the async
// state has landed before we assert (no fake timers — the bridge stub resolves for real).
async function flush() {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve();
    await new Promise((r) => setTimeout(r, 0));
  }
}

describe("mount", () => {
  it("renders the page + appliance picker when appliance.list returns one appliance", async () => {
    const el = document.createElement("div");
    document.body.appendChild(el);
    const bridge = stubBridge({
      "control-engine.appliance.list": () => ({
        appliances: [{ id: "ce-studio", name: "CE Studio", base: "http://127.0.0.1:7979" }],
      }),
    });

    const unmount = mount(el, { workspace: "acme" }, bridge);
    await flush();

    expect(el.textContent).toContain("Control Engine");
    expect(el.textContent).toContain("acme"); // the tenant wall reached the remote
    // The picker rendered the appliance, and the (stubbed) editor mounted for it.
    expect(el.querySelector('select[aria-label="appliance"]')).not.toBeNull();
    expect(el.querySelector('[data-testid="ce-editor"]')).not.toBeNull();

    unmount();
    await flush();
    expect(el.childNodes.length).toBe(0);
    el.remove();
  });

  it("shows the add-appliance empty state when the list is empty", async () => {
    const el = document.createElement("div");
    document.body.appendChild(el);
    const bridge = stubBridge({ "control-engine.appliance.list": () => ({ appliances: [] }) });

    const unmount = mount(el, { workspace: "acme" }, bridge);
    await flush();

    expect(el.querySelector('form[aria-label="add appliance"]')).not.toBeNull();
    expect(el.textContent).toContain("No control engines yet");
    // No editor mounts without an appliance.
    expect(el.querySelector('[data-testid="ce-editor"]')).toBeNull();

    unmount();
    await flush();
    el.remove();
  });
});
