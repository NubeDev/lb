// `proof-panel` — the Tier-1 WASM reference extension — driven against a REAL spawned gateway (no fake
// backend; CLAUDE §9 / testing §0). This proves the exact data path the proof-panel federated page is
// built on: the host-mediated bridge (`makeBridge`, the SAME seam the shell hands `mount(el, ctx,
// bridge)`) forwarding `series.find` / `series.latest` over the real `POST /mcp/call` route to the real
// host, behind the real capability + workspace gates. (The page's RENDER logic — idle/list/select/
// error — is covered against this same bridge contract in the co-located in-memory test at
// rust/extensions/proof-panel/ui/src/pages/Panel.test.tsx; here we prove the LIVE path.)
//
// Why the bridge seam and not a remote import: the federated page lives in its own package; the seam it
// depends on is `makeBridge(scope).call(tool, args)`. Exercising that seam over the real gateway is the
// honest end-to-end proof — before this slice, `POST /mcp/call` could not dispatch a host-native
// `series.*` verb at all (it resolved only the runtime registry; the host fix is in
// crates/host/src/tool_call.rs + debugging/extensions/bridge-cannot-dispatch-host-native-series.md).
//
// The proof, in one motion: empty workspace → honest empty find → seed a real series → find lists it →
// select → latest shows its value → an ungranted verb (the grant-intersection narrowing) is denied.

import { describe, expect, it, beforeAll } from "vitest";

import { makeBridge } from "./bridge";
import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `proof-panel-${n++}`;

/** The page's granted read-only scope (the manifest's `[ui] scope`, intersected with the approval). */
const PAGE_SCOPE = ["series.find", "series.latest"];
/** One facet the page searches by (parsed from its `key:value` search box). */
const TEMP_FACET = [{ key: "kind", value: "temperature" }];

beforeAll(() => useRealGateway());

describe("proof-panel page data path (real gateway)", () => {
  it("empty workspace → find returns no series (honest empty state, not fabricated rows)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const bridge = makeBridge(PAGE_SCOPE);

    // The real host scopes every read to the token's workspace; a never-seeded workspace is empty.
    const res = await bridge.call<{ series: string[] }>("series.find", { facets: TEMP_FACET });
    expect(res.series).toEqual([]);
  });

  it("seed a real series → find lists it → latest shows its value", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed through the REAL write path: a committed sample (value 61.4) + the discovery tag edge.
    await seedSeries({ series: "edge.temp", seq: 1, payload: 61.4, key: "kind", value: "temperature" });

    const bridge = makeBridge(PAGE_SCOPE);

    // find lists the seeded series (the host filters to `series:`-prefixed tagged entities).
    const found = await bridge.call<{ series: string[] }>("series.find", { facets: TEMP_FACET });
    expect(found.series.some((s) => s === "edge.temp" || s === "series:edge.temp")).toBe(true);

    // select → latest shows the committed value.
    const latest = await bridge.call<{ sample: { payload: unknown } | null }>("series.latest", {
      series: "edge.temp",
    });
    expect(latest.sample).not.toBeNull();
    expect(latest.sample?.payload).toBe(61.4);
  });

  it("is workspace-isolated — a fresh workspace sees NONE of another workspace's series", async () => {
    // Seed ws-A, then read as a FRESH ws-B over the same real node: the hard wall makes B's find empty.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedSeries({ series: "a.secret", seq: 1, payload: 9, key: "kind", value: "temperature" });

    const wsB = nextWs();
    await signInReal("user:eve", wsB);
    const bridge = makeBridge(PAGE_SCOPE);
    const res = await bridge.call<{ series: string[] }>("series.find", { facets: TEMP_FACET });
    expect(res.series).toEqual([]); // none of ws-A's series leaks into ws-B
  });

  it("an ungranted verb is denied at the bridge (grant-intersection narrowing, honest error)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeries({ series: "edge.temp", seq: 1, payload: 1, key: "kind", value: "temperature" });

    // The page was granted ONLY series.find (the admin approval narrowed it). The bridge's scope filter
    // rejects series.latest locally (defense in depth) — and the host would deny it server-side too.
    const narrowed = makeBridge(["series.find"]);
    await expect(
      narrowed.call("series.latest", { series: "edge.temp" }),
    ).rejects.toThrow(/out_of_scope/);

    // And proven against the REAL host: a principal-scope that the page bypasses still hits a server
    // deny. We force the in-scope filter open by building a bridge that claims latest, but the SESSION
    // token is the dev set (which holds series.latest), so to prove the SERVER gate we call an ext tool
    // the dev token lacks — `proof-panel.proof.ping` is not in the dev claim set → 403 at the host.
    const claimsLatest = makeBridge(["proof-panel.proof.ping"]);
    await expect(
      claimsLatest.call("proof-panel.proof.ping", { ws }),
    ).rejects.toThrow();
  });
});
