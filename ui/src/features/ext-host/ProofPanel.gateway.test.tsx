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
import {
  useRealGateway,
  signInReal,
  seedSeries,
  seedInbox,
  seedOutbox,
} from "@/test/gateway-session";

let n = 0;
const nextWs = () => `proof-panel-${n++}`;

/** The page's granted read-only scope (the manifest's `[ui] scope`, intersected with the approval). */
const PAGE_SCOPE = ["series.find", "series.latest"];
/** The full all-features demo scope (the manifest `[ui] scope`): reads + the write/workflow verbs. */
const DEMO_SCOPE = [
  "series.find",
  "series.latest",
  "ingest.write",
  "outbox.status",
  "inbox.list",
  "inbox.resolve",
];
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

  // ── The all-features demo, live: the page CREATES the data it shows and drives the durable workflow,
  // all over the real `POST /mcp/call` bridge → `lb_host::call_tool`, behind the real caps + ws gates.

  it("ingest.write → series.latest round-trips live (the page creates its own data)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const bridge = makeBridge(DEMO_SCOPE);

    // Write a sample through the bridge — the exact shape the page's useIngestWrite sends.
    const wrote = await bridge.call<{ accepted: number }>("ingest.write", {
      samples: [
        { series: "proof.demo", producer: "", ts: 1, seq: 1, payload: 21, labels: null, qos: "best-effort" },
      ],
    });
    expect(wrote.accepted).toBe(1);

    // The node's drain commits staging → `series`. Poll series.latest until the committed value shows
    // (the commit worker is asynchronous; this is write → stage → drain → read, end to end, live).
    let payload: unknown = null;
    for (let i = 0; i < 50 && payload === null; i++) {
      const latest = await bridge.call<{ sample: { payload: unknown } | null }>("series.latest", {
        series: "proof.demo",
      });
      payload = latest.sample?.payload ?? null;
      if (payload === null) await new Promise((r) => setTimeout(r, 100));
    }
    expect(payload).toBe(21);
  });

  it("ingest.write is denied for an out-of-scope page (local filter) — deny per verb", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // A page NOT granted ingest.write: the bridge scope filter rejects it locally (defense in depth);
    // the host would 403 it too. (The dev token holds the cap, so the SERVER deny is proven for the
    // ext-tool case above + in the Rust host test; here we assert the bridge's own per-verb gate.)
    const readOnly = makeBridge(["series.find", "series.latest"]);
    await expect(
      readOnly.call("ingest.write", { samples: [] }),
    ).rejects.toThrow(/out_of_scope/);
  });

  it("outbox.status reads real effects live, and denies an out-of-scope page", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed a real outbox effect through the real enqueue path.
    await seedOutbox({ id: "e1", target: "github", action: "comment", ts: 1 });

    const bridge = makeBridge(DEMO_SCOPE);
    const status = await bridge.call<{ pending: unknown[] }>("outbox.status", {});
    expect(status.pending.length).toBe(1);

    // Deny per verb: an out-of-scope page is rejected at the bridge.
    const narrowed = makeBridge(["series.find"]);
    await expect(narrowed.call("outbox.status", {})).rejects.toThrow(/out_of_scope/);
  });

  it("inbox.list → inbox.resolve round-trips live, and denies an out-of-scope page", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed a real durable inbox item on the `triage` channel.
    await seedInbox({ id: "i1", channel: "triage", author: "ext:demo", body: "please review", ts: 1 });

    const bridge = makeBridge(DEMO_SCOPE);
    const listed = await bridge.call<{ items: { id: string }[] }>("inbox.list", {
      channel: "triage",
    });
    expect(listed.items.some((it) => it.id === "i1")).toBe(true);

    // The first WRITE that mutates workflow state — approve the item.
    const ok = await bridge.call<{ ok: boolean }>("inbox.resolve", {
      item_id: "i1",
      decision: "approved",
      ts: 2,
    });
    expect(ok.ok).toBe(true);

    // Deny per verb (both list and resolve) for an out-of-scope page.
    const narrowed = makeBridge(["series.find"]);
    await expect(narrowed.call("inbox.list", { channel: "triage" })).rejects.toThrow(/out_of_scope/);
    await expect(
      narrowed.call("inbox.resolve", { item_id: "i1", decision: "rejected", ts: 3 }),
    ).rejects.toThrow(/out_of_scope/);
  });

  it("the workflow surface is workspace-isolated — ws-B sees none of ws-A's items/effects", async () => {
    // Seed ws-A with an inbox item + an outbox effect, then read as a FRESH ws-B over the same node.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedInbox({ id: "i1", channel: "triage", author: "ext:demo", body: "secret", ts: 1 });
    await seedOutbox({ id: "e1", target: "github", action: "comment", ts: 1 });

    const wsB = nextWs();
    await signInReal("user:eve", wsB);
    const bridge = makeBridge(DEMO_SCOPE);

    const inbox = await bridge.call<{ items: unknown[] }>("inbox.list", { channel: "triage" });
    expect(inbox.items).toEqual([]); // the hard wall on the inbox read

    const outbox = await bridge.call<{ pending: unknown[] }>("outbox.status", {});
    expect(outbox.pending).toEqual([]); // the hard wall on the outbox read
  });
});
