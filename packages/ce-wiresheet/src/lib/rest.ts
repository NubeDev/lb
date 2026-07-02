import type {
  AddComponentRequest,
  Component,
  Edge,
  EdgeRequest,
  FlexValue,
  ReadNodesResponse,
  UpdateComponentRequest,
} from "./engine-types";
import type { EngineTransport, RequestMethod } from "./transport";
import { DirectTransport, RestError, setEngineBase } from "./transport-direct";

// Every typed wrapper below calls `request()`, which is routed through the
// injected EngineTransport (default `DirectTransport` — today's direct
// fetch-against-setEngineBase behavior, unchanged). `RestError` and
// `setEngineBase` live in transport-direct.ts now; re-exported here so
// existing import sites (`from "./lib/rest"`) keep working unchanged.
export { RestError, setEngineBase };

let transport: EngineTransport = new DirectTransport();
export function setRestTransport(t: EngineTransport) {
  transport = t;
}

// Set by App.tsx after the WS schema arrives. Sent as `X-CE-Session` on every mutating
// request so the engine can attribute the resulting topology events to us — useful for
// echo suppression / origin filtering on the WS side.
let currentSessionId: string | null = null;
export function setRestSessionId(id: string | null) {
  currentSessionId = id;
}

// Per-user actor id (low 16 bits) sent as `X-Actor-Id` so the engine scopes this
// client's undo/redo stack. Sent on EVERY mutating request (so each change is
// recorded on the right stack) AND on undo/redo/changelog (which address it).
// Unset (null) => the engine's shared stack (actor 0) — fine for a single editor.
let currentActorId: number | null = null;
export function setRestActorId(id: number | null) {
  currentActorId = id;
}

// Gesture id (int32) sent as `X-Gesture-Id`. All writes sharing one non-zero id
// are grouped by the engine into a SINGLE atomic undo entry — both a streamed
// drag (many position writes → one undo) and a compound gesture across endpoints
// (Group = add folder + reparent + facets → one undo). Set it around a gesture
// (try/finally), then clear. Unset => the engine's short time-window coalescing.
let currentGestureId: number | null = null;
export function setRestGestureId(id: number | null) {
  currentGestureId = id;
}
// Monotonic per-session gesture id generator (positive int32, never 0).
let gestureSeq = 0;
export function newGestureId(): number {
  gestureSeq = (gestureSeq + 1) & 0x7fffffff;
  return gestureSeq || 1;
}
// Run an async gesture with a fresh gesture id stamped on every write inside it,
// so the whole gesture undoes as one unit. Always clears, even on throw. Use the
// low-level setRestGestureId directly for gestures that span event handlers
// (e.g. a drag: set on dragstart, clear on dragstop).
export async function withGesture<T>(fn: () => Promise<T>): Promise<T> {
  setRestGestureId(newGestureId());
  try {
    return await fn();
  } finally {
    setRestGestureId(null);
  }
}

async function http<T>(method: RequestMethod, path: string, body?: unknown): Promise<T> {
  const data = await transport.request({
    method,
    path,
    body,
    session: currentSessionId,
    actor: currentActorId,
    gesture: currentGestureId,
  });
  return data as T;
}

// Components ----------------------------------------------------------------

export function getRootNodes(opts?: {
  depth?: number;
  nested?: boolean;
  withEdges?: boolean;
  // Return only components whose full type matches (engine does a flat global
  // scan — every folder). Used to enumerate all components of a kind (e.g. every
  // schedule) without a registry/index component.
  type?: string;
  // Set false to skip the per-property value plane (structure only) — lean reads
  // for list views that just need name/path/type.
  values?: boolean;
}): Promise<ReadNodesResponse> {
  const q = new URLSearchParams();
  if (opts?.depth != null) q.set("depth", String(opts.depth));
  if (opts?.nested != null) q.set("nested", String(opts.nested));
  if (opts?.withEdges != null) q.set("withEdges", String(opts.withEdges));
  if (opts?.type != null) q.set("type", opts.type);
  if (opts?.values != null) q.set("values", String(opts.values));
  // The engine matches the `type` filter literally and does NOT url-decode it, so
  // the "vendor-ext::name" colons must stay raw — URLSearchParams encodes ':' to
  // %3A, which would never match (silently returning 0). No other param carries a
  // colon, so unescaping them in the final query string is safe.
  const qs = q.toString().replace(/%3A/g, ":");
  return http<ReadNodesResponse>("GET", `/nodes${qs ? "?" + qs : ""}`);
}

export function getNodeByUid(
  uid: number,
  opts?: { depth?: number; nested?: boolean; withEdges?: boolean },
) {
  const q = new URLSearchParams();
  if (opts?.depth != null) q.set("depth", String(opts.depth));
  if (opts?.nested != null) q.set("nested", String(opts.nested));
  if (opts?.withEdges != null) q.set("withEdges", String(opts.withEdges));
  const qs = q.toString();
  return http<ReadNodesResponse>("GET", `/nodes/uid/${uid}${qs ? "?" + qs : ""}`);
}

export function addNode(req: AddComponentRequest) {
  return http<Component>("POST", `/nodes`, req);
}

export function updateNode(uid: number, req: UpdateComponentRequest) {
  return http<Component>("PATCH", `/nodes/uid/${uid}`, req);
}

// Override endpoints. Setting an override pins a property's value so cycle
// execution doesn't change it; clearing returns the property to engine control.
// `duration` is the override lifetime in seconds (0 = permanent).
export interface PropertyOverride {
  property: string;
  value: FlexValue;
  duration?: number;
}
export interface OverridesRequest {
  setOverrides?: PropertyOverride[];
  clearOverrides?: string[];
}
export function patchOverrides(uid: number, req: OverridesRequest) {
  return http<Component>("PATCH", `/overrides/nodes/uid/${uid}`, req);
}

// Action dispatch — `POST /call/nodes/uid/{uid}`. Invoke one named action with
// an optional params map; the engine returns the action's `returns` values. The
// action's signature (which params/returns it has) is type-level and comes from
// `/schema`; this call just dispatches it on one component instance.
export function callAction(
  uid: number,
  action: string,
  params?: Record<string, FlexValue>,
): Promise<{ returns: Record<string, FlexValue> }> {
  return http<{ returns: Record<string, FlexValue> }>("POST", `/call/nodes/uid/${uid}`, {
    action,
    params,
  });
}

export function removeNode(uid: number) {
  return http<unknown>("DELETE", `/nodes/uid/${uid}`);
}

// Undo / redo ----------------------------------------------------------------
// First-class on the engine: it journals each mutation and inverts it on undo.
// Scoped per actor via the X-Actor-Id header (see setRestActorId). `ok:false`
// with a `reason` means the change couldn't be applied (e.g. precondition gone);
// `componentUids`/`edgeUids` name what was touched so the caller can reload.
// NOTE: the journal covers structural ops (add/remove/edge/batch/copy/move/
// override) — NOT property-value updates, so canvas position drags and prop/
// rename edits are not engine-undoable.
export type ChangeOp =
  | "addComponent" | "removeComponent" | "addEdge" | "removeEdge"
  | "batchAdd" | "batchRemove" | "copy" | "moveComponent"
  | "setOverride" | "clearOverride";
export interface UndoResult {
  ok: boolean;
  reason?: string;
  changeId?: number;
  opType?: ChangeOp;
  componentUids?: number[];
  edgeUids?: number[];
}
export interface ChangeEntry {
  changeId: number;
  actorId: number;
  opType: ChangeOp;
  state: "applied" | "undone" | "expired";
  seq: number;
  createdAt: number;
  componentUids?: number[];
  edgeUids?: number[];
  undoable: boolean;
}
export function undoChange(changeId?: number) {
  return http<UndoResult>("POST", `/undo`, changeId ? { changeId } : {});
}
export function redoChange() {
  return http<UndoResult>("POST", `/redo`, {});
}
export function getChangeLog(limit?: number) {
  const qs = limit != null ? `?limit=${limit}` : "";
  return http<{ undoable: ChangeEntry[]; redoable: ChangeEntry[] }>("GET", `/changelog${qs}`);
}

// Edges ---------------------------------------------------------------------

export function getEdges(component?: string | number) {
  const qs = component != null ? `?component=${encodeURIComponent(String(component))}` : "";
  return http<Edge[]>("GET", `/edges${qs}`);
}

// Classified folder view: every edge with >=1 endpoint anywhere in `subtree`'s
// tree (any depth), each carrying `class` + source/targetContainer (the engine
// pre-classifies relative to the view — see API_REQUESTS §2). Replaces the deep
// node refetch for recovering grandchildren edges + their container membership.
export function getSubtreeEdges(subtree: string | number) {
  return http<Edge[]>("GET", `/edges?subtree=${encodeURIComponent(String(subtree))}`);
}

export function addEdge(req: EdgeRequest) {
  return http<{ uid: number } & Edge>("POST", `/edge`, req);
}

// Facets / grouping — the engine (ce-rest) is the authoritative exposure
// maintainer; the client never writes `__facets`. See EXPOSURE_SPEC.

// First-class group: create a Folder under `parentUid` (root if absent),
// reparent the selection into it, reconcile exposure — one transaction. Returns
// the new folder.
export function groupComponents(req: { componentUids: number[]; parentUid?: number; name?: string }) {
  return http<Component>("POST", `/group`, req);
}
// Dissolve a folder: reparent its children back to its parent, remove it, reconcile.
export function ungroupFolder(folderUid: number) {
  return http<{ folderUid: number; reparented: number[] }>("POST", `/ungroup`, { folderUid });
}
// Manually pin a child prop as a port on `folderUid` (persists with no edge).
export function exposePort(folderUid: number, propUid: number) {
  return http<unknown>("POST", `/facets/uid/${folderUid}/port/${propUid}`);
}
// Clear the manual pin (the port survives only if a boundary edge still justifies it).
export function unexposePort(folderUid: number, propUid: number) {
  return http<unknown>("DELETE", `/facets/uid/${folderUid}/port/${propUid}`);
}

// Result shape shared by the bulk write endpoints. componentUids/edgeUids are
// the survivors (failed inputs are absent); `partialSuccess` + `errors` carry
// per-item failures (HTTP 207). (Client no longer creates from specs — paste
// uses /copy/nodes, undo-delete uses /restore — but bulkUpdate still returns
// this shape.)
export interface BulkAddResponse {
  components?: Array<{ uid: number; name: string; path: string }>;
  edges?: Array<{ uid: number }>;
  result?: {
    componentUids?: number[];
    edgeUids?: number[];
    partialSuccess?: boolean;
    errors?: {
      components?: Array<{ inputIndex: number; message: string }>;
      edges?: Array<{ inputIndex: number; message: string }>;
    };
    componentErrors?: Array<{ inputIndex: number; message: string }>;
    edgeErrors?: Array<{ inputIndex: number; message: string }>;
  };
}

// PATCH /bulknodes — atomic multi-component update. Shares its body shape
// with PATCH /nodes/uid/{uid} per entry. Used to batch group drag moves so
// 8-component multi-select drag at 10 Hz doesn't fire 80 PATCHes/sec.
export interface BulkUpdateEntry {
  uid: number;
  name?: string;
  parentUid?: number;
  position?: { x?: number; y?: number };
  properties?: Record<string, { value: FlexValue }>;
}

export function bulkUpdate(updates: BulkUpdateEntry[]): Promise<BulkAddResponse> {
  return http<BulkAddResponse>("PATCH", `/bulknodes`, { updates });
}

// DELETE /bulknodes — atomic multi-component / multi-edge removal (soft-delete;
// restorable via POST /restore).
export interface UidLists {
  componentUids?: number[];
  edgeUids?: number[];
}
export function bulkDelete(req: UidLists): Promise<unknown> {
  return http<unknown>("DELETE", `/bulknodes`, req);
}

// POST /copy/nodes — clone components (and their internal edges) by UID under a
// destination parent. Server-side clone: full fidelity, tiny request. Returns
// the created components (full shape, incl. positions copied from the source).
// This is how paste works — no client-side spec reconstruction.
export function copyNodes(req: {
  componentUids: number[];
  destParentUid: number;
  includeInternalEdges?: boolean;
}): Promise<ReadNodesResponse> {
  return http<ReadNodesResponse>("POST", `/copy/nodes`, {
    includeInternalEdges: true,
    ...req,
  });
}

// POST /restore — restore soft-deleted components/edges by their ORIGINAL uids
// (full state). This is undo-of-delete: strictly better than recreating from a
// snapshot (keeps uids, full values, and the deleted subtree).
export function restoreItems(req: UidLists): Promise<unknown> {
  return http<unknown>("POST", `/restore`, req);
}

// Returns a human-readable summary if a bulk response is a partial success
// (HTTP 207 — some items failed). `res.ok` is true for 207 so http() returns
// normally; without this check the failures are invisible. Callers surface it
// without throwing, so the items that DID succeed still apply.
export function bulkPartialError(res: BulkAddResponse): string | null {
  if (!res.result?.partialSuccess) return null;
  const errs = [
    ...(res.result.errors?.components ?? res.result.componentErrors ?? []),
    ...(res.result.errors?.edges ?? res.result.edgeErrors ?? []),
  ];
  if (errs.length === 0) return "bulk op partially failed";
  const first = errs[0];
  return `bulk op: ${errs.length} item(s) failed — ${first.message}`;
}

export function removeEdge(uid: number) {
  return http<unknown>("DELETE", `/edge/uid/${uid}`);
}

// PATCH /edge/uid/{uid}. `reEvaluate: true` re-fires the source value through
// the edge once — useful when the downstream component's last-computed value
// stuck and you want to nudge a recompute without changing the source.
export interface UpdateEdgeRequest {
  loopBack?: boolean;
  hidden?: boolean;
  reEvaluate?: boolean;
}

export function updateEdge(uid: number, req: UpdateEdgeRequest) {
  return http<Edge>("PATCH", `/edge/uid/${uid}`, req);
}

// System / schema -----------------------------------------------------------

export interface ExtensionInfo {
  name: string;
  version?: string;
  components?: string[];
}

export function getExtensions() {
  return http<ExtensionInfo[]>("GET", `/extensions`);
}

// GET /schema — the add-node palette/type catalogue (extensions + their
// component defs, incl. action signatures + per-property choices). Distinct
// from the WS `schema` message (a flat value-plane decode table only) — see
// slice-1-wiresheet-transport-seam.md "Open questions" for how this was
// confirmed. Routed through the same transport seam as every other verb here
// so a bridge transport doesn't need a second, raw-fetch path. Deliberately
// loose (`unknown`-ish leaves): callers (CeEditor's palette load, choices.ts)
// already narrow with their own precise shapes via a cast, same as when this
// was a raw `fetch` — this type only needs to describe the JSON shape.
export interface SchemaComponentDef {
  name: string;
  icon?: string;
  actions?: unknown[];
  role?: string;
  singleton?: boolean;
  properties?: Array<{ name: string; choices?: string }>;
}
export interface SchemaExtensionDef {
  vendor: string;
  name: string;
  version?: string;
  components?: SchemaComponentDef[];
}
export function getSchema() {
  return http<SchemaExtensionDef[]>("GET", `/schema`);
}
