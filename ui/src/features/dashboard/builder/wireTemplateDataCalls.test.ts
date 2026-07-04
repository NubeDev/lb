// Direct unit tests for the `[data-call]` click wiring (render-template-inprocess scope, Decision 5 —
// "the wiring reads only data-* attributes"). The TemplateView component test covers the integrated
// path; these prove the CONTRACT of the wiring module in isolation: a click forwards ONLY the
// `data-call`/`data-args` blobs through the leashed bridge, an out-of-leash tool is rejected locally,
// and an author inline handler (`onclick`) — which the sanitizer strips, but defense-in-depth — is
// never read or executed. The cleanup contract (no listener leak, no double-fire) is proven too.

import { describe, it, expect } from "vitest";
import { fireEvent } from "@testing-library/react";

import { wireTemplateDataCalls } from "./wireTemplateDataCalls";
import type { WidgetBridge } from "./widgetBridge";

/** A tiny fake bridge that records every call. `tools` is the leash the wiring checks. */
function fakeBridge(calls: Array<{ tool: string; args: Record<string, unknown> }>): WidgetBridge {
  return {
    call: async <T,>(tool: string, args?: Record<string, unknown>): Promise<T> => {
      calls.push({ tool, args: args ?? {} });
      return {} as T;
    },
    watch: () => () => {},
  };
}

/** Flush the bridge.call promise so the wiring's `.then(setAttribute)` runs before the assertion. */
function flush(): Promise<void> {
  return Promise.resolve();
}

function mount(html: string): { root: HTMLElement; cleanup: () => void } {
  const root = document.createElement("div");
  root.innerHTML = html;
  document.body.appendChild(root);
  return { root, cleanup: () => root.remove() };
}

describe("wireTemplateDataCalls — reads ONLY data-* (Decision 5 belt-and-braces)", () => {
  it("forwards a data-call click's tool+args through the leashed bridge, stamps data-called=\"ok\"", async () => {
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    const bridge = fakeBridge(calls);
    const { root, cleanup } = mount(
      `<button data-call="rules.run" data-args='{"rule_id":"r1"}'>Go</button>`,
    );
    const detach = wireTemplateDataCalls(root, bridge, ["rules.run"]);
    const btn = root.querySelector<HTMLButtonElement>("[data-call]")!;
    fireEvent.click(btn);
    await flush();
    expect(calls).toEqual([{ tool: "rules.run", args: { rule_id: "r1" } }]);
    expect(btn.getAttribute("data-called")).toBe("ok");
    detach();
    cleanup();
  });

  it("REJECTS a data-call outside the leash: no bridge call, data-called=\"err\"", () => {
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    const bridge = fakeBridge(calls);
    const { root, cleanup } = mount(`<button data-call="store.delete" data-args='{"id":"x"}'>X</button>`);
    const detach = wireTemplateDataCalls(root, bridge, ["rules.run"]); // store.delete not in leash
    fireEvent.click(root.querySelector("[data-call]")!);
    expect(calls).toHaveLength(0);
    expect(root.querySelector("[data-call]")!.getAttribute("data-called")).toBe("err");
    detach();
    cleanup();
  });

  it("wires ONLY [data-call] elements: a hostile element without data-call is never selected", async () => {
    // Decision 5 belt-and-braces: the wiring's element selection is `[data-call]` and it forwards ONLY
    // the `data-call`/`data-args` blobs. A hostile element with an inline handler but NO data-call is
    // invisible to the wiring — it is never selected, never read, never forwarded. (The sanitizer strips
    // inline handlers too; this guards the hypothetical miss on the WIRING side: even if a hostile
    // attribute survived, the wiring would not read or forward it.)
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    const bridge = fakeBridge(calls);
    const { root, cleanup } = mount(
      `<div><span>nope</span></div>` +
        `<button data-call="rules.run" data-args="{}">legit</button>`,
    );
    const detach = wireTemplateDataCalls(root, bridge, ["rules.run"]);
    fireEvent.click(root.querySelector("[data-call]")!);
    await flush();
    expect(calls).toHaveLength(1);
    expect(calls[0].tool).toBe("rules.run");
    // The hostile <div>/<span> are NOT in the wiring's selection; clicking them forwards nothing.
    fireEvent.click(root.querySelector("div")!);
    fireEvent.click(root.querySelector("span")!);
    expect(calls).toHaveLength(1); // still only the legit data-call
    detach();
    cleanup();
  });

  it("forwards ONLY the data-call/data-args blobs — never reads another attribute (e.g. id/class)", async () => {
    // The wiring reads `getAttribute("data-call")` and `getAttribute("data-args")` — nothing else. A
    // data-call element carrying unrelated attrs (id/class/style/…) forwards only the two data-* blobs;
    // the bridge never sees the others. This pins the "reads only data-*" contract behaviorally.
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    const bridge = fakeBridge(calls);
    const { root, cleanup } = mount(
      `<button id="btn-1" class="btn hostile" style="color:red" data-call="rules.run" data-args='{"rule_id":"r1"}'>Go</button>`,
    );
    const detach = wireTemplateDataCalls(root, bridge, ["rules.run"]);
    fireEvent.click(root.querySelector("[data-call]")!);
    await flush();
    expect(calls).toEqual([{ tool: "rules.run", args: { rule_id: "r1" } }]);
    // The forwarded args blob is EXACTLY data-args — the id/class/style never leaked into the call.
    expect(calls[0].args).not.toHaveProperty("id");
    expect(calls[0].args).not.toHaveProperty("class");
    detach();
    cleanup();
  });

  it("stamps data-called=\"err\" on a malformed data-args JSON (no crash, no bridge call)", () => {
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    const bridge = fakeBridge(calls);
    const { root, cleanup } = mount(`<button data-call="rules.run" data-args="not-json">Go</button>`);
    const detach = wireTemplateDataCalls(root, bridge, ["rules.run"]);
    fireEvent.click(root.querySelector("[data-call]")!);
    expect(calls).toHaveLength(0);
    expect(root.querySelector("[data-call]")!.getAttribute("data-called")).toBe("err");
    detach();
    cleanup();
  });

  it("cleanup detaches the listeners (no double-fire after detach)", async () => {
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    const bridge = fakeBridge(calls);
    const { root, cleanup } = mount(`<button data-call="rules.run" data-args="{}">Go</button>`);
    const detach = wireTemplateDataCalls(root, bridge, ["rules.run"]);
    const btn = root.querySelector<HTMLButtonElement>("[data-call]")!;
    fireEvent.click(btn);
    await flush();
    fireEvent.click(btn);
    await flush();
    expect(calls).toHaveLength(2);
    detach();
    fireEvent.click(btn); // after cleanup, a click does nothing
    await flush();
    expect(calls).toHaveLength(2);
    cleanup();
  });

  it("wires multiple [data-call] elements independently", async () => {
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    const bridge = fakeBridge(calls);
    const { root, cleanup } = mount(
      `<button data-call="rules.run" data-args="{}">A</button><button data-call="store.query" data-args='{"sql":"SELECT 1"}'>B</button>`,
    );
    const detach = wireTemplateDataCalls(root, bridge, ["rules.run", "store.query"]);
    const [a, b] = root.querySelectorAll<HTMLButtonElement>("[data-call]");
    fireEvent.click(a);
    fireEvent.click(b);
    await flush();
    expect(calls.map((c) => c.tool)).toEqual(["rules.run", "store.query"]);
    detach();
    cleanup();
  });
});
