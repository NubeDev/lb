import { describe, expect, it } from "vitest";

import { mount } from "@/mount";
import { stubBridge } from "@/test/bridge.stub";

describe("mount", () => {
  it("renders into the element with the host ctx and returns an unmount fn that clears it", async () => {
    const el = document.createElement("div");
    document.body.appendChild(el);
    const bridge = stubBridge({ "series.find": () => [] });

    const unmount = mount(el, { workspace: "acme" }, bridge);
    // React 18 createRoot renders asynchronously; flush the microtask + macrotask queues.
    await Promise.resolve();
    await new Promise((r) => setTimeout(r, 0));

    expect(typeof unmount).toBe("function");
    expect(el.textContent).toContain("Proof Panel");
    expect(el.textContent).toContain("acme"); // the host ctx (tenant wall) reached the remote

    unmount();
    await new Promise((r) => setTimeout(r, 0));
    expect(el.childNodes.length).toBe(0);

    el.remove();
  });
});
