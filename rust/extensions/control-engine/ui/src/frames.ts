// Decode the S6 `control-engine.watch` JSON frames into the vendored editor's `StreamHandlers` callbacks.
// This is the browser-side counterpart of the sidecar's `watch/frame.rs` re-encode: the sidecar maps a
// decoded `CovEvent` → a plumbing-agnostic JSON frame, this maps that frame → the editor's already-typed
// `DecodedFrame` / `TopologyMsg` (so the editor never sees the binary `wire.ts` decode over the bridge).
//
// S6 frame contract (see control-engine `watch/frame.rs`):
//   { kind:"cov", ts, values:[{uid,v}], status?:[{uid,s}] }   → onFrame(DecodedFrame)
//   { kind:"topology", ts, msg:{op,seq,componentUids,edgeUids?} } → onTopology(TopologyMsg)
//   (no "schema" kind is emitted by S6; a defensive arm forwards one to onSchema if it ever appears.)
//
// The >2^53 rule (mirror of frame.rs): an integer past `Number.MAX_SAFE_INTEGER` arrives as a JSON
// STRING. We coerce an all-digits string back to a `bigint` so the editor's `DecodedValue` keeps it
// lossless; any other string stays a string (a genuine string value passes through untouched).
//
// SEAM NOTE (resolved WITHOUT a vendored edit): the vendored `index.ts` exports `StreamHandlers` but NOT
// the `DecodedFrame`/`DecodedValue`/`TopologyMsg`/`SchemaMessage` TYPES nor the wire-tag CONSTANTS. So we
// (a) DERIVE those types from the exported `StreamHandlers` signature (`Parameters<>`), and (b) declare
// the two wire constants we need as fixed S6-protocol literals here. A cleaner fix is upstream (S1
// re-export), tracked as a follow-up in the session doc — but nothing here touches the vendored package.

import type { StreamHandlers } from "@nube/ce-wiresheet";

// Types recovered from the exported handler signatures (no named type export exists to import directly).
type DecodedFrame = Parameters<StreamHandlers["onFrame"]>[0];
type DecodedSection = DecodedFrame["sections"][number];
type DecodedValue = Extract<DecodedSection["values"], unknown[]>[number];
type TopologyMsg = Parameters<StreamHandlers["onTopology"]>[0];
type SchemaMessage = Parameters<StreamHandlers["onSchema"]>[0];

// The two S6-frame wire tags we build. Fixed protocol integers (see the vendored `engine-types.ts`
// `TYPE_STATUS`/`MSG_UPDATE` + `wire.ts`): STATUS sections carry per-uid uint32 status bits, everything
// else is a typed value section. Declared here because the vendored index doesn't re-export them.
const TYPE_STATUS = 0x40; // STATUS section tag — the editor routes these uids to its statusFlags store.
const TYPE_F64 = 0x22; // a generic (non-STATUS) value tag — the editor routes these uids to its value store.
const MSG_UPDATE = 0x01; // delta value frame (vs 0x02 snapshot).

/** The cov frame the sidecar writes onto the series (`watch/frame.rs::encode_values`). */
export interface CovFrame {
  kind: "cov";
  ts: number;
  values: Array<{ uid: number; v: DecodedValue }>;
  status?: Array<{ uid: number; s: number }>;
}

/** The topology frame (`watch/frame.rs::encode_topology`). `msg` is the decoded variant + its uids. */
export interface TopologyFrame {
  kind: "topology";
  ts: number;
  msg: {
    op: "added" | "removed" | "changed";
    seq: number;
    componentUids: number[];
    edgeUids?: number[];
  };
}

type Frame = CovFrame | TopologyFrame | { kind: "schema"; msg: SchemaMessage };

/** Coerce one JSON value the sidecar wrote back to a `DecodedValue`. An all-digits (optionally signed)
 *  string is a >2^53 integer the sidecar stringified — restore it to a `bigint`; anything else is a
 *  genuine value and passes through. */
function coerceValue(v: DecodedValue): DecodedValue {
  if (typeof v === "string" && /^-?\d+$/.test(v)) {
    try {
      return BigInt(v) as DecodedValue;
    } catch {
      return v;
    }
  }
  return v;
}

/** Map a decoded cov frame to the editor's `DecodedFrame`. Values go in one non-STATUS section (the
 *  editor routes any non-STATUS typeTag to its value store — the exact tag is cosmetic, so we use
 *  `TYPE_F64` as the generic value tag); nonzero status flags go in a STATUS section the editor routes
 *  to its statusFlags store. A frame with no status omits the section (matching a clean tick). */
export function decodeCov(frame: CovFrame): DecodedFrame {
  const valueUids = new Uint32Array(frame.values.map((c) => c.uid));
  const values: DecodedValue[] = frame.values.map((c) => coerceValue(c.v));
  const sections: DecodedSection[] = [{ typeTag: TYPE_F64, uids: valueUids, values }];
  if (frame.status && frame.status.length > 0) {
    sections.push({
      typeTag: TYPE_STATUS,
      uids: new Uint32Array(frame.status.map((s) => s.uid)),
      values: new Uint32Array(frame.status.map((s) => s.s)),
    });
  }
  return { msgType: MSG_UPDATE, timestampMs: frame.ts, sections };
}

/** Map a decoded topology frame to the editor's `TopologyMsg`. The editor keys on `seq` + the uid lists
 *  and refetches the affected subtree via `control-engine.tree`, so we hand it the minimal descriptor
 *  shape its `onTopology` handler reads (uid arrays; no per-item structural descriptors — a resync, not
 *  a patch). `originSessionId` is null: no per-actor origin is forwarded over the bridge (v1 gap). */
export function decodeTopology(frame: TopologyFrame): TopologyMsg {
  const { op, seq, componentUids, edgeUids } = frame.msg;
  if (op === "removed") {
    return {
      type: "topologyRemoved",
      seq,
      originSessionId: null,
      componentUids,
      edgeUids: edgeUids ?? [],
    };
  }
  if (op === "changed") {
    // A structural "changed" signal with no descriptors → mark parent-changed so the editor refetches
    // (its `onTopology` refetches when `parent !== undefined`); positions/names aren't carried here.
    return {
      type: "topologyChanged",
      seq,
      originSessionId: null,
      components: componentUids.map((uid) => ({ uid, parent: undefined })),
    };
  }
  // "added": the editor refetches the current folder if it doesn't already have every uid; we hand it
  // bare descriptors (uid only) so its `haveAll` check drives a resync via `control-engine.tree`.
  return {
    type: "topologyAdded",
    seq,
    originSessionId: null,
    components: componentUids.map((uid) => ({ uid, kind: "" })),
    edges: (edgeUids ?? []).map((uid) => ({ uid, sourceProperty: 0, targetProperty: 0 })),
  };
}

/** Dispatch one decoded S6 frame (a `series.watch` SSE sample's payload) to the right handler. Unknown
 *  `kind` is ignored (forward-compat: a future frame kind must not throw the live feed down). */
export function dispatchFrame(frame: Frame, handlers: StreamHandlers): void {
  switch (frame.kind) {
    case "cov":
      handlers.onFrame(decodeCov(frame));
      return;
    case "topology":
      handlers.onTopology(decodeTopology(frame));
      return;
    case "schema":
      // S6 does not emit this; forward defensively if a later slice adds a schema passthrough.
      handlers.onSchema(frame.msg);
      return;
    default:
      return;
  }
}
