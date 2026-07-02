// @vitest-environment happy-dom
// Bind-via-picker, driven through the REAL mount (source-picker-package-scope.md, thecrew consumer).
// A scene shape's bind slot now uses the reusable @nube/source-picker, discovering workspace series
// through the bridge (`series.list`) instead of only already-bound channels. We drive the whole wired
// path — mountPage → ScenePage (injects the bridge-backed loaders) → App → PropertyRail — with a stub
// bridge answering `series.list`, so this proves the loaders context + the bridge call + the
// selection→channel mapping together (no RTL dep; the raw-DOM style of mount.test.tsx).

import { describe, expect, it, beforeEach } from "vitest";
import { mountPage } from "../remoteEntry";
import { stubBridge } from "../bridge/bridge.stub";
import { useSceneStore } from "../state/scene-store";

const store = () => useSceneStore.getState();

/** Flush React 18 async render + effects + the bridge's async loader promises (several turns). */
async function flush() {
  for (let i = 0; i < 8; i++) {
    await new Promise((r) => setTimeout(r, 0));
    await Promise.resolve();
  }
}

/** A page bridge that answers the picker's `series.list` (+ the inert reads the page makes on mount). */
function pageBridge() {
  return stubBridge({
    "series.list": () => ({ series: ["ahu1.sf1.speed", "ahu1.rat"] }),
    "assets.list_docs": () => ({ docs: [] }),
    "series.latest": () => ({ sample: null }),
  });
}

beforeEach(() => {
  store().loadDemo("blank");
});

describe("PropertyRail — bind via the reusable source picker (wired through mountPage)", () => {
  it("a selected shape's bind slot lists the workspace series discovered over the bridge", async () => {
    const id = store().placeShape("hvac.fan", 0, 0); // fan slots: running, speed, fault
    store().select([id], false);
    const el = document.createElement("div");
    const unmount = mountPage(el, { workspace: "acme" }, pageBridge());
    await flush();
    const speed = el.querySelector('select[aria-label="bind speed"]') as HTMLSelectElement | null;
    expect(speed).toBeTruthy();
    expect(speed!.textContent).toContain("ahu1.sf1.speed");
    unmount();
  });

  it("picking a series sets the bind CHANNEL on the shape (bind stays {channel})", async () => {
    const id = store().placeShape("hvac.fan", 0, 0);
    store().select([id], false);
    const el = document.createElement("div");
    const unmount = mountPage(el, { workspace: "acme" }, pageBridge());
    await flush();
    const speed = el.querySelector('select[aria-label="bind speed"]') as HTMLSelectElement;
    speed.value = "series:ahu1.sf1.speed";
    speed.dispatchEvent(new Event("change", { bubbles: true }));
    await flush();
    expect(store().doc.shapes[id].bind).toEqual({ speed: { channel: "ahu1.sf1.speed" } });
    unmount();
  });

  it("a denied series.list leaves the rail usable (empty picker), never a crash", async () => {
    const id = store().placeShape("hvac.fan", 0, 0);
    store().select([id], false);
    const el = document.createElement("div");
    // series.list absent from the table → the stub throws out_of_scope; the loader catch → empty group.
    const unmount = mountPage(el, { workspace: "acme" }, stubBridge({ "assets.list_docs": () => ({ docs: [] }) }));
    await flush();
    const speed = el.querySelector('select[aria-label="bind speed"]') as HTMLSelectElement | null;
    expect(speed).toBeTruthy(); // the picker still renders (just no series options)
    expect(speed!.textContent).not.toContain("ahu1.sf1.speed");
    unmount();
  });
});
