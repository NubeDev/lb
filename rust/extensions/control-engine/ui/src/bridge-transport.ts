// `BridgeTransport` — the LB-authored `EngineTransport` that routes the vendored wiresheet's requests
// over the caps-gated `bridge.call('control-engine.*')` + `bridge.watch('series.watch')` seam instead of
// a direct fetch/WebSocket to a CE. Injected via `<CeEditor transport={...} />`; the browser never
// touches CE — every canvas action is a host-mediated MCP call, re-checked against install-scope ∩ grant.
//
// Two halves (see the seam in `@nube/ce-wiresheet` lib/transport.ts):
//   - request(EngineRequest{method,path,body,...}) — the wiresheet speaks `/api/v0`-relative REST paths
//     (rest.ts's typed wrappers). We map each path→a `control-engine.*` tool, translate the REST body to
//     the tool's arg shape (keyed node identity `{uid,kind,path}`; always inject the selected appliance),
//     and map the tool's JSON result back to the `data` shape rest.ts expects. A path with NO mapping
//     throws a LOUD error naming it (never a silent 404) — the signal a follow-up verb is needed.
//   - openStream(handlers) — arm the appliance's COV feed via `control-engine.watch`, then subscribe its
//     series through `bridge.watch('series.watch')`; each SSE sample's payload is an S6 frame → frames.ts
//     → the right handler. If the bridge has no `watch` (Tauri/tests), degrade to a static canvas.

import type {
  EngineRequest,
  EngineStream,
  EngineTransport,
  StreamHandlers,
} from "@nube/ce-wiresheet";

import type { Bridge } from "./contract";
import { dispatchFrame, type CovFrame, type TopologyFrame } from "./frames";

/** A CE node identity keyed the way the `control-engine.*` verbs parse it (`args.rs` `NodeKeyArg`): a UID
 *  is never a bare integer — it carries its pool `kind` and an optional snapshotted `path`. */
interface NodeKeyArg {
  uid: number;
  kind?: string;
  path?: string;
}

/** Extract `{uid}` from a `/nodes/uid/{uid}`-style path (trailing query stripped). */
function uidFromPath(path: string): number | null {
  const m = path.match(/\/uid\/(\d+)/);
  return m ? Number(m[1]) : null;
}

/** A component `NodeKeyArg` for a uid parsed from a path (the wiresheet addresses components by uid). */
function componentKey(uid: number): NodeKeyArg {
  return { uid, kind: "component" };
}

/** Thrown by `request()` when the wiresheet emits a REST path with no `control-engine.*` mapping. Loud
 *  and path-naming ON PURPOSE — a silent 404 would let an un-bridged canvas action fail invisibly; this
 *  names exactly which follow-up verb the extension still owes. */
export class UnmappedPathError extends Error {
  constructor(
    public method: string,
    public path: string,
  ) {
    super(
      `control-engine BridgeTransport: no control-engine.* verb maps ${method} ${path} — ` +
        `this canvas action needs a follow-up verb (add the mapping + the sidecar verb, do not silently 404).`,
    );
    this.name = "UnmappedPathError";
  }
}

export class BridgeTransport implements EngineTransport {
  constructor(
    private readonly bridge: Bridge,
    private readonly appliance: string,
  ) {}

  // --- request half --------------------------------------------------------

  async request(req: EngineRequest): Promise<unknown> {
    const { tool, args, unwrap } = this.map(req);
    const result = await this.bridge.call<unknown>(tool, { appliance: this.appliance, ...args });
    return unwrap ? unwrap(result) : result;
  }

  /** Map one `EngineRequest` to its tool call + a result-unwrap. Throws {@link UnmappedPathError} for any
   *  path the wiresheet can emit but no verb backs (undo/redo/changelog/group/copy/bulk/edges-GET/…). */
  private map(req: EngineRequest): {
    tool: string;
    args: Record<string, unknown>;
    unwrap?: (r: unknown) => unknown;
  } {
    const { method, path, body } = req;
    const p = path.split("?")[0];

    // GET /schema — the add-node palette/type catalogue.
    if (method === "GET" && p === "/schema") {
      return { tool: "control-engine.schema", args: {}, unwrap: (r) => (r as { manifests: unknown }).manifests };
    }

    // GET /nodes  |  GET /nodes/uid/{uid} — structural read (whole tree, or a keyed subtree).
    if (method === "GET" && (p === "/nodes" || p.startsWith("/nodes/uid/"))) {
      const uid = uidFromPath(p);
      const args: Record<string, unknown> = {};
      if (uid != null) args.node = componentKey(uid);
      const depth = new URLSearchParams(path.split("?")[1] ?? "").get("depth");
      if (depth != null) args.depth = Number(depth);
      // control-engine.tree returns { nodes, edges } — the exact ReadNodesResponse shape rest.ts wants.
      return { tool: "control-engine.tree", args };
    }

    // POST /nodes — create a component. rest.ts body: AddComponentRequest { type, parentUid?, name?,
    // properties?/initialValues? }. Map to add-node { type, parent?, name?, initial_values? }.
    if (method === "POST" && p === "/nodes") {
      const b = (body ?? {}) as Record<string, unknown>;
      const args: Record<string, unknown> = { type: b.type };
      if (b.parentUid != null) args.parent = componentKey(Number(b.parentUid));
      if (b.name != null) args.name = b.name;
      const initial = (b.initialValues ?? b.properties) as Record<string, { value: unknown }> | Record<string, unknown> | undefined;
      if (initial) args.initial_values = flattenValues(initial);
      // add-node returns { uid, kind } — rest.ts's addNode caller reads a Component; the editor refetches
      // via the topology signal, so the keyed identity is enough to satisfy the write ack.
      return { tool: "control-engine.add-node", args };
    }

    // PATCH /nodes/uid/{uid} — write property values. rest.ts body: UpdateComponentRequest with
    // { properties: { name: { value } }, name?, position? }. Map the property values → patch { node,
    // values } (a flat name→scalar object). NOTE (v1 gap): name/position are NOT carried — set-layout is a
    // deferred verb, so a drag/rename does not persist (documented in the session doc).
    if (method === "PATCH" && p.startsWith("/nodes/uid/")) {
      const uid = requireUid(method, path);
      const b = (body ?? {}) as Record<string, unknown>;
      const values = flattenValues((b.properties ?? {}) as Record<string, { value: unknown }>);
      return {
        tool: "control-engine.patch",
        args: { node: componentKey(uid), values },
        unwrap: (r) => (r as { component: unknown }).component,
      };
    }

    // PATCH /overrides/nodes/uid/{uid} — set/clear overrides. rest.ts body: OverridesRequest
    // { setOverrides?: [{property, value, duration?}], clearOverrides?: [property] }. The wiresheet sends
    // ONE override op at a time in practice; we route the first set → set-override, else the first clear →
    // clear-override. (A batch of mixed ops is a follow-up; a single op is the v1 canvas gesture.)
    if (method === "PATCH" && p.startsWith("/overrides/nodes/uid/")) {
      const uid = requireUid(method, path);
      const b = (body ?? {}) as { setOverrides?: Array<{ property: string; value: unknown; duration?: number }>; clearOverrides?: string[] };
      const set = b.setOverrides?.[0];
      if (set) {
        return {
          tool: "control-engine.set-override",
          args: {
            node: componentKey(uid),
            property: set.property,
            value: set.value,
            ttl_secs: set.duration ?? 0,
          },
        };
      }
      const clear = b.clearOverrides?.[0];
      if (clear != null) {
        return {
          tool: "control-engine.clear-override",
          args: { node: componentKey(uid), property: clear },
        };
      }
      throw new UnmappedPathError(method, `${path} (empty override body)`);
    }

    // POST /edge — wire a dataflow edge. rest.ts body: EdgeRequest. The engine addresses endpoints by
    // PROPERTY uid; add-edge takes source/target NODE keys + property NAMES. rest.ts's EdgeRequest carries
    // sourceProperty/targetProperty (property uids) — map them as property-kind node keys with no name
    // (the sidecar resolves the property). See the v1 gap note: an edge whose endpoints rest.ts expresses
    // purely as property uids maps to add-edge with property-keyed endpoints.
    if (method === "POST" && p === "/edge") {
      const b = (body ?? {}) as Record<string, unknown>;
      return {
        tool: "control-engine.add-edge",
        args: {
          source: { uid: Number(b.sourceComponent ?? b.source), kind: "component" },
          source_property: b.sourceProperty,
          target: { uid: Number(b.targetComponent ?? b.target), kind: "component" },
          target_property: b.targetProperty,
        },
        unwrap: (r) => r,
      };
    }

    // DELETE /nodes/uid/{uid} — soft-delete a component + descendants.
    if (method === "DELETE" && p.startsWith("/nodes/uid/")) {
      const uid = requireUid(method, path);
      return { tool: "control-engine.remove-node", args: { node: componentKey(uid) } };
    }

    // POST /call/nodes/uid/{uid} — invoke a named action. rest.ts body: { action, params? }.
    if (method === "POST" && p.startsWith("/call/nodes/uid/")) {
      const uid = requireUid(method, path);
      const b = (body ?? {}) as { action: string; params?: Record<string, unknown> };
      return {
        tool: "control-engine.call-action",
        args: { node: componentKey(uid), action: b.action, params: b.params },
      };
    }

    // No mapping — loud, path-naming (never a silent 404). These are the wiresheet paths S7 does NOT
    // back yet: /undo /redo /changelog /extensions /group /ungroup /facets/* /bulknodes /copy/nodes
    // /restore /edges (GET) /edge/uid/{uid}. Each is a named follow-up verb.
    throw new UnmappedPathError(method, path);
  }

  // --- stream half ---------------------------------------------------------

  /** Arm the appliance's COV feed and pipe its series SSE into `handlers`. `control-engine.watch` returns
   *  `{ series, subject }`; `bridge.watch('series.watch', { series }, onEvent)` then streams each sample.
   *  If the bridge has no `watch` (Tauri desktop / the vitest harness — no SSE transport), we report the
   *  stream `closed` and degrade to a STATIC canvas (structure loads via `request`; no live values). */
  openStream(handlers: StreamHandlers): EngineStream {
    handlers.onStatus("connecting");
    let unsubscribe: (() => void) | undefined;
    let closed = false;

    if (typeof this.bridge.watch !== "function") {
      handlers.onStatus("closed");
      return makeStream(() => {});
    }
    const watch = this.bridge.watch;

    void this.bridge
      .call<{ series: string; subject: string }>("control-engine.watch", { appliance: this.appliance })
      .then(({ series }) => {
        if (closed) return;
        handlers.onStatus("open");
        unsubscribe = watch("series.watch", { series }, (event) => {
          // Each SSE sample is `{ payload, seq }`; the payload is an S6 frame (`watch/frame.rs`).
          const payload = extractPayload(event);
          if (payload) dispatchFrame(payload, handlers);
        });
      })
      .catch(() => {
        // Watch arming failed (denied, or no appliance) — honest closed, static canvas. Not a throw:
        // the request half still works, so the canvas renders without live values.
        if (!closed) handlers.onStatus("closed");
      });

    return makeStream(() => {
      closed = true;
      unsubscribe?.();
      handlers.onStatus("closed");
    });
  }
}

/** Pull an S6 frame off a `series.watch` sample. The shell delivers `{ payload, seq }`; the frame is the
 *  `payload`. Tolerates a bare frame too (a sample that is already the frame). Non-frames → null. */
function extractPayload(event: unknown): CovFrame | TopologyFrame | null {
  if (!event || typeof event !== "object") return null;
  const withPayload = event as { payload?: unknown };
  const candidate = "payload" in withPayload ? withPayload.payload : event;
  if (candidate && typeof candidate === "object" && "kind" in (candidate as object)) {
    return candidate as CovFrame | TopologyFrame;
  }
  return null;
}

/** Flatten rest.ts's `{ name: { value } }` (or a plain `{ name: value }`) into a flat name→scalar object,
 *  the shape the `patch`/`add-node` verbs' `value_pairs` parser expects. */
function flattenValues(props: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [name, v] of Object.entries(props)) {
    out[name] = v && typeof v === "object" && "value" in (v as object) ? (v as { value: unknown }).value : v;
  }
  return out;
}

function requireUid(method: string, path: string): number {
  const uid = uidFromPath(path);
  if (uid == null) throw new UnmappedPathError(method, `${path} (no uid in path)`);
  return uid;
}

/** The minimal `EngineStream` the bridge needs. Subscriptions are v1-SIMPLE: `openStream` already armed
 *  the WHOLE appliance's COV feed, so the per-uid `setSubscriptions`/`setPropSubscriptions` diffs are
 *  no-ops here — the editor filters to visible uids client-side (measuring per-uid bus scoping is a named
 *  follow-up). `setTickHz`/`getTickHz` are inert (the sidecar owns the CE tick rate); `sessionId` is null
 *  (no per-actor undo attribution over the bridge — a documented v1 gap). */
function makeStream(close: () => void): EngineStream {
  let tickHz: number | null = null;
  return {
    setSubscriptions() {},
    setPropSubscriptions() {},
    setTickHz(hz: number) {
      tickHz = hz;
    },
    getTickHz() {
      return tickHz;
    },
    close,
    get sessionId() {
      return null;
    },
  };
}
