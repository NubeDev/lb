// The generic bus bridge over a REAL gateway (widget-config-vars "Platform fix" + Slices 4/5; CLAUDE
// §9 — no fake backend). A dashboard cell reaches `bus.publish` through the SAME host-mediated
// WidgetBridge every tool rides; the host re-checks the cap + walls the subject. Covers: a granted
// publish round-trips `{ok:true}`; a publish to a tool OUTSIDE the cell's tool set is rejected by the
// bridge leash; a reserved/cross-ws subject is refused server-side. (The publish→watch SSE round-trip
// is proven at the transport in role/gateway/tests/bus_routes_test.rs — jsdom has no EventSource.)

import { describe, expect, it, beforeAll } from "vitest";

import { makeWidgetBridge } from "./widgetBridge";
import { interpolateArgs, resolveBuiltins } from "@/lib/vars";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `bus-bridge-${n++}`;

beforeAll(() => useRealGateway());

describe("bus bridge (real gateway)", () => {
  it("publishes through the leashed bridge → {ok:true} (fire-and-forget, host-walled)", async () => {
    await signInReal("user:ada", nextWs());
    const bridge = makeWidgetBridge(["bus.publish"]); // the cell's tool set ∩ grant
    const res = await bridge.call<{ ok: boolean }>("bus.publish", {
      subject: "ui/banner",
      payload: { msg: "hello" },
    });
    expect(res.ok).toBe(true);
  });

  it("rejects a tool OUTSIDE the cell's tool set at the bridge leash (defense in depth)", async () => {
    await signInReal("user:ada", nextWs());
    const bridge = makeWidgetBridge(["series.read"]); // bus.publish NOT in the set
    await expect(bridge.call("bus.publish", { subject: "x", payload: {} })).rejects.toThrow(
      /out_of_scope/,
    );
  });

  it("refuses a reserved subject server-side even with the cap", async () => {
    await signInReal("user:ada", nextWs());
    const bridge = makeWidgetBridge(["bus.publish"]);
    // `series/...` is platform motion — the host's wall_subject guard refuses it (400 → a thrown error).
    await expect(
      bridge.call("bus.publish", { subject: "series/cpu", payload: {} }),
    ).rejects.toThrow();
  });

  it("Slice 5 — a JSON payload template interpolates + sends over bus.publish end to end", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // The JsonPayloadField's send logic: parse the template, interpolate against the scope, then
    // bridge.call(target, { subject, payload }). A `${newTodo}`/`${__workspace}` template resolves.
    const scope = { values: { newTodo: "buy milk" }, builtins: resolveBuiltins({ workspace: ws }) };
    const template = JSON.parse('{"text":"${newTodo}","ws":"${__workspace}"}');
    const payload = interpolateArgs(template, scope) as Record<string, unknown>;
    expect(payload).toEqual({ text: "buy milk", ws });

    const bridge = makeWidgetBridge(["bus.publish"]);
    const res = await bridge.call<{ ok: boolean }>("bus.publish", { subject: "ui/todo", payload });
    expect(res.ok).toBe(true); // published (fire-and-forget) — handed to the bus, host-walled + gated
  });
});
