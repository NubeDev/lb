import type { SchemaMessage, TopologyMsg } from "./engine-types";
import type { DecodedFrame } from "./wire";

// The seam between the editor and however it reaches a Control Engine. Sits at
// the PROTOCOL altitude, not the HTTP altitude: callers already speak "typed
// request + decoded stream" (rest.ts's typed wrappers, CeEditor's WS handlers),
// so this is exactly where rest.ts's internal http() and ws.ts's message
// dispatch sit today. `DirectTransport` (transport-direct.ts) is the only
// implementation in this package — it reproduces today's direct-to-CE behavior
// verbatim. Other transports (e.g. an MCP/Zenoh bridge) implement the same
// interface outside this package and are injected via `CeEditor`'s `transport`
// prop.

export type RequestMethod = "GET" | "POST" | "PATCH" | "DELETE";

export interface EngineRequest {
  method: RequestMethod;
  // Relative to /api/v0, e.g. "/nodes/uid/42".
  path: string;
  body?: unknown;
  // Was header X-CE-Session — carried so the engine can attribute the
  // resulting topology events to the caller (echo suppression / origin
  // filtering on the stream side).
  session?: string | null;
  // Was header X-Actor-Id — scopes the engine's per-actor undo/redo stack.
  actor?: number | null;
  // Was header X-Gesture-Id — groups writes sharing one non-zero id into a
  // single atomic undo entry.
  gesture?: number | null;
}

// Thrown by a transport's `request()` on failure. Deliberately the same shape
// `RestError` already had (rest.ts re-exports `RestError` as this type so
// existing catch sites are unaffected).
export interface EngineRequestError extends Error {
  status: number;
  url: string;
  method: string;
  requestBody?: unknown;
  responseBody?: unknown;
}

export interface StreamHandlers {
  onSchema(msg: SchemaMessage): void;
  onTopology(msg: TopologyMsg): void;
  // Values/status frames — decoded. wire.ts's binary decode is a
  // DirectTransport concern; a consumer never sees raw bytes.
  onFrame(frame: DecodedFrame): void;
  onStatus(s: "connecting" | "open" | "closed"): void;
}

export interface EngineStream {
  // The visible-uid diff (subscriptions.ts). Replaces
  // CeRestWs.setDesiredSubscription (component-level value/status streaming).
  setSubscriptions(uids: Set<number>): void;
  // Property-level subscription diff (exposed ports + drawer widgets that
  // need one off-canvas prop's value without its whole component). Replaces
  // CeRestWs.setDesiredPropSubscription — same diff-and-send shape, a second
  // independent channel alongside setSubscriptions (the real WS protocol has
  // two distinct subscribe/unsubscribe message kinds, components vs
  // properties; the slice doc's single "visible-uid diff" line is the
  // component channel, this is the sibling channel it doesn't call out but
  // whose behavior must carry over verbatim).
  setPropSubscriptions(uids: Set<number>): void;
  setTickHz(hz: number): void;
  // Last value/status push rate this side asked for (null = engine default,
  // never explicitly set). Replaces CeRestWs.getRate — the DiagPanel rate
  // controls read this to show the live setting.
  getTickHz(): number | null;
  close(): void;
  // Engine session id (undo/redo attribution). Null until the stream's first
  // schema message arrives.
  readonly sessionId: string | null;
}

export interface EngineTransport {
  // Request half — replaces rest.ts's internal http(). The typed wrappers
  // (readNodes, addComponent, patchComponent, …) stay in rest.ts and call
  // this. Returns the unwrapped `data`; throws on failure.
  request(req: EngineRequest): Promise<unknown>;

  // Stream half — replaces ws.ts's socket ownership.
  openStream(handlers: StreamHandlers): EngineStream;
}
