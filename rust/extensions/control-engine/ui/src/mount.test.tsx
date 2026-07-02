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
  it("mounts the editor with NO page header/picker when there is a single appliance", async () => {
    const el = document.createElement("div");
    document.body.appendChild(el);
    const bridge = stubBridge({
      "control-engine.appliance.list": () => ({
        appliances: [{ id: "ce-studio", name: "CE Studio", base: "http://127.0.0.1:7979" }],
      }),
    });

    const unmount = mount(el, { workspace: "acme" }, bridge);
    await flush();

    // Issue #2: the HOST header owns the title + workspace, so the page renders NO header of its
    // own (no "Control Engine · acme" bar) — that clashed with the host's. A single appliance has
    // nothing to pick, so no picker either; the editor just mounts for it.
    expect(el.textContent).not.toContain("Control Engine");
    expect(el.querySelector('select[aria-label="appliance"]')).toBeNull();
    expect(el.querySelector('[data-testid="ce-editor"]')).not.toBeNull();

    unmount();
    await flush();
    expect(el.childNodes.length).toBe(0);
    el.remove();
  });

  it("shows the appliance picker only when there is a real choice (≥2 appliances)", async () => {
    const el = document.createElement("div");
    document.body.appendChild(el);
    const bridge = stubBridge({
      "control-engine.appliance.list": () => ({
        appliances: [
          { id: "ce-a", name: "CE A", base: "http://127.0.0.1:7979" },
          { id: "ce-b", name: "CE B", base: "http://127.0.0.1:7989" },
        ],
      }),
    });

    const unmount = mount(el, { workspace: "acme" }, bridge);
    await flush();

    const picker = el.querySelector('select[aria-label="appliance"]');
    expect(picker).not.toBeNull();
    expect(picker?.querySelectorAll("option").length).toBe(2);
    // Still no page title/workspace chrome — only the picker.
    expect(el.textContent).not.toContain("Control Engine");
    expect(el.querySelector('[data-testid="ce-editor"]')).not.toBeNull();

    unmount();
    await flush();
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
