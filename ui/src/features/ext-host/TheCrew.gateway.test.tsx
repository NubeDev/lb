// `thecrew` — the graphics-canvas extension (100% UI) — driven against a REAL spawned gateway (no
// fake backend; CLAUDE §9 / testing §0). It proves the exact data path the federated page + widget
// are built on: the host-mediated bridge (`makeBridge`, the SAME seam the shell hands the mounts)
// forwarding `assets.put_doc`/`get_doc`/`list_docs` + `series.latest` over the real `POST /mcp/call`
// route to the real host, behind the real capability + workspace gates. Zero core additions — a pure
// consumer of shipped verbs.
//
// The scene-io + bridge-source RENDER/interim logic (conflict, dedupe, no-access) is covered against
// the same bridge contract in the co-located in-memory tests at
// rust/extensions/thecrew/ui/src/bridge/*.test.ts; here we prove the LIVE path + the mandatory
// capability-deny and workspace-isolation categories.
//
// NOTE (honest gap): the live SSE (`series.watch`) is NOT exercisable in this vitest harness — the
// real-gateway path here has no watch transport (matching proof-panel's live tile, proven via the
// watch stub + Playwright, not test:gateway). The backfill half (`series.latest`) IS proven live.

import { describe, expect, it, beforeAll } from "vitest";

import { makeBridge } from "./bridge";
import { useRealGateway, signInReal, signInWithCaps, seedSeries } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `thecrew-${n++}`;

/** The manifest `[ui] scope` (intersected with the install approval) — the page's full grant. */
const PAGE_SCOPE = [
  "assets.get_doc",
  "assets.put_doc",
  "assets.list_docs",
  "series.latest",
  "series.read",
  "series.watch",
];
/** The `[[widget]] scope` — deliberately NARROWER: read a scene + its live values, never save/list. */
const WIDGET_SCOPE = ["assets.get_doc", "series.latest", "series.watch"];

/** The mcp:*:call caps behind the page scope (for a real narrow-cap token in deny tests). */
const capsFor = (scope: string[]) => scope.map((t) => `mcp:${t}:call`);

/** The FULL grant a real thecrew install requests: the MCP verb caps PLUS the underlying doc-store
 *  caps `assets.put_doc`/`get_doc` gate on (`authorize_doc` → `store:doc/{id}:{read,write}`). The
 *  standard dev/member login does NOT carry `mcp:assets.put_doc:call` (finding — see session doc), so
 *  the positive doc-write tests mint this explicit grant, exactly as the install would. */
const INSTALL_CAPS = [...capsFor(PAGE_SCOPE), "store:doc/*:read", "store:doc/*:write"];

const SCENE_ID = "scene:ahu-1";
/** A minimal scene doc with one bound shape — the round-trip payload. */
const SCENE = {
  v: 1,
  camera: "ortho-top",
  shapes: {
    sf1: {
      type: "hvac.fan",
      t: { x: 96, y: 0 },
      props: { label: "SF-1" },
      bind: { speed: { channel: "ahu1.sf1.speed" } },
    },
  },
};
const serialize = (doc: unknown) => JSON.stringify(doc);

beforeAll(() => useRealGateway());

describe("thecrew data path (real gateway)", () => {
  it("seed→load→edit→save→reload round-trip is byte-stable through assets.put_doc/get_doc", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, INSTALL_CAPS);
    const bridge = makeBridge(PAGE_SCOPE);

    // Seed the scene through the REAL write path (assets.put_doc) — a real doc record, not a fake.
    await bridge.call("assets.put_doc", {
      id: SCENE_ID,
      title: "AHU-1",
      content: serialize(SCENE),
      content_type: "json",
      tags: ["scene"],
      ts: 1,
    });

    // list_docs sees it (the picker); the id carries the scene: prefix convention.
    const listed = await bridge.call<{ docs: { id: string; title: string }[] }>("assets.list_docs");
    expect(listed.docs.some((d) => d.id === SCENE_ID)).toBe(true);

    // load → the stored content round-trips byte-stable.
    const loaded = await bridge.call<{ content: string }>("assets.get_doc", { id: SCENE_ID });
    expect(loaded.content).toBe(serialize(SCENE));

    // edit + save → reload reflects the edit.
    const edited = { ...SCENE, shapes: { ...SCENE.shapes, sf1: { ...SCENE.shapes.sf1, props: { label: "SF-1a" } } } };
    await bridge.call("assets.put_doc", {
      id: SCENE_ID,
      title: "AHU-1",
      content: serialize(edited),
      content_type: "json",
      tags: ["scene"],
      ts: 2,
    });
    const reloaded = await bridge.call<{ content: string }>("assets.get_doc", { id: SCENE_ID });
    expect(reloaded.content).toBe(serialize(edited));
  });

  it("binding backfill: series.latest delivers a seeded sample to a bound channel", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed the bound series through the real ingest path (a committed sample + discovery tag).
    await seedSeries({ series: "ahu1.sf1.speed", seq: 1, payload: 880, key: "kind", value: "speed" });

    const bridge = makeBridge(PAGE_SCOPE);
    const latest = await bridge.call<{ sample: { payload: unknown } | null }>("series.latest", {
      series: "ahu1.sf1.speed",
    });
    expect(latest.sample).not.toBeNull();
    expect(latest.sample?.payload).toBe(880);
  });

  it("capability deny: a viewer without the put_doc grant is DENIED a save (real host gate)", async () => {
    const ws = nextWs();
    // A real signed token scoped to READ-ONLY caps (no assets.put_doc) — the widget-class grant.
    await signInWithCaps("user:ada", ws, capsFor(WIDGET_SCOPE));
    // The bridge scope filter would also reject, so use a bridge that permits put_doc to prove the
    // SERVER-side deny (not just the client filter).
    const bridge = makeBridge(["assets.put_doc"]);
    await expect(
      bridge.call("assets.put_doc", {
        id: "scene:nope",
        title: "x",
        content: "{}",
        content_type: "json",
        tags: ["scene"],
        ts: 1,
      }),
    ).rejects.toThrow();
  });

  it("capability deny (client scope filter): the widget grant cannot reach list_docs/put_doc", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const widget = makeBridge(WIDGET_SCOPE);
    // The widget scope omits these — the bridge filter rejects before the host even sees them.
    await expect(widget.call("assets.list_docs")).rejects.toThrow(/out_of_scope/);
    await expect(widget.call("assets.put_doc", { id: "x", title: "x", content: "{}", ts: 1 })).rejects.toThrow(
      /out_of_scope/,
    );
  });

  it("workspace isolation: a scene saved in ws A is invisible from ws B", async () => {
    const wsA = nextWs();
    await signInWithCaps("user:ada", wsA, INSTALL_CAPS);
    const bridgeA = makeBridge(PAGE_SCOPE);
    await bridgeA.call("assets.put_doc", {
      id: SCENE_ID,
      title: "A's scene",
      content: serialize(SCENE),
      content_type: "json",
      tags: ["scene"],
      ts: 1,
    });
    // A sees it.
    const inA = await bridgeA.call<{ docs: { id: string }[] }>("assets.list_docs");
    expect(inA.docs.some((d) => d.id === SCENE_ID)).toBe(true);

    // A fresh workspace B (same user) cannot list or read A's scene — the workspace wall.
    const wsB = nextWs();
    await signInWithCaps("user:ada", wsB, INSTALL_CAPS);
    const bridgeB = makeBridge(PAGE_SCOPE);
    const inB = await bridgeB.call<{ docs: { id: string }[] }>("assets.list_docs");
    expect(inB.docs.some((d) => d.id === SCENE_ID)).toBe(false);
    await expect(bridgeB.call("assets.get_doc", { id: SCENE_ID })).rejects.toThrow();
  });

  it("widget no-access: a viewer denied the bound series gets an honest empty backfill, not a value", async () => {
    const ws = nextWs();
    // Seed a series, then sign in with a token that lacks series.latest — the bound shape's no-access.
    await signInReal("user:ada", ws);
    await seedSeries({ series: "ahu1.sf1.speed", seq: 1, payload: 880, key: "kind", value: "speed" });
    await signInWithCaps("user:ada", ws, capsFor(["assets.get_doc"])); // no series.latest
    const bridge = makeBridge(["series.latest"]); // client permits it; host must deny
    await expect(bridge.call("series.latest", { series: "ahu1.sf1.speed" })).rejects.toThrow();
  });
});
