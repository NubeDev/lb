import {
  MSG_SNAPSHOT,
  MSG_UPDATE,
  TYPE_STATUS,
  type SchemaMessage,
  type TopologyMsg,
} from "./engine-types";
import { decodeBinaryFrame } from "./wire";
import { diffSets } from "./subscriptions";
import {
  metrics,
  recordEvent,
  recordMessage,
  recordParse,
  recordTopology,
  recordValueCount,
} from "./instrumentation";
import {
  diagGauges,
  diagRecordFrame,
  diagRecordMessage,
  diagRecordStatusUids,
  diagRecordValueUids,
} from "./diagnostics";
import type { EngineRequest, EngineStream, EngineTransport, StreamHandlers } from "./transport";

// `DirectTransport` — today's behavior, extracted verbatim: `fetch` against
// `setEngineBase`'s origin, the binary WS with session resume, reconnect
// backoff, BroadcastChannel tab-ownership, `wire.ts` decode. All of this
// machinery is direct-mode-specific; a bridge transport (e.g. over MCP/Zenoh)
// needs none of it — it has its own reconnect story. This is the ONLY
// EngineTransport implementation in this package.

// REST origin for the selected control engine. Defaults to same-origin
// (the standalone ce-ui's Vite-proxy convention); the extension calls
// `setEngineBase("http://<ip>:<port>")` once per mounted editor so every
// request AND the WS URL derived from it target the chosen engine directly.
let ORIGIN = "";
let BASE = "/api/v0";
export function setEngineBase(origin: string) {
  ORIGIN = origin.replace(/\/+$/, "");
  BASE = `${ORIGIN}/api/v0`;
}

export class RestError extends Error {
  constructor(
    public status: number,
    message: string,
    public url: string,
    public method = "",
    public requestBody?: unknown,
    public responseBody?: unknown,
  ) {
    super(message);
  }

  // A copy-pasteable dump of the failed round-trip for debugging.
  get debug(): string {
    const fmt = (v: unknown) =>
      v === undefined ? "" : typeof v === "string" ? v : JSON.stringify(v, null, 2);
    const lines = [`${this.method} ${this.url}`, `→ ${this.status} ${this.message}`];
    if (this.requestBody !== undefined) lines.push("", "Request:", fmt(this.requestBody));
    if (this.responseBody !== undefined) lines.push("", "Response:", fmt(this.responseBody));
    return lines.join("\n");
  }
}

// All ce-rest responses are `{ data: T }` on success or `{ error: string }` on
// failure. Status codes derive from the error string (400/404/409/500/504). We
// unwrap once here and surface plain `T` to callers (rest.ts's typed
// wrappers); failures throw RestError.
async function directRequest(req: EngineRequest): Promise<unknown> {
  const { method, path, body } = req;
  const url = `${BASE}${path}`;
  const headers: Record<string, string> = {};
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (req.session && method !== "GET") headers["X-CE-Session"] = req.session;
  if (req.actor != null) headers["X-Actor-Id"] = String(req.actor);
  if (req.gesture != null && method !== "GET") headers["X-Gesture-Id"] = String(req.gesture);
  const res = await fetch(url, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  let payload: { data?: unknown; error?: string } = {};
  try {
    payload = (await res.json()) as typeof payload;
  } catch {
    // Empty body — e.g. some 5xx paths. Fall through; status determines outcome.
  }
  // Log mutations to the events panel. GETs would dominate the log so we
  // skip them — the panel is for tracking writes and the topology / value
  // pushes they trigger.
  if (method !== "GET") {
    const status = res.ok && !payload.error ? res.status : `ERR ${res.status}`;
    recordEvent("rest", `${method} ${path} → ${status}`);
  }
  if (!res.ok || payload.error) {
    throw new RestError(res.status, payload.error ?? res.statusText, url, method, body, payload);
  }
  return payload.data;
}

export function defaultWsUrl(): string {
  if (typeof window === "undefined") return "ws://127.0.0.1:8080/ws";
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}/ws`;
}

// Build the engine's WebSocket URL from its REST origin (`http://<ip>:<port>`
// → `ws://<ip>:<port>/ws`, `https` → `wss`). Used by the extension to point
// the live-value socket at the selected control engine directly.
export function wsUrlFromBase(origin: string): string {
  return `${origin.replace(/^http/, "ws").replace(/\/+$/, "")}/ws`;
}

// ce-rest WS client. Same-origin via Vite proxy (/ws → engine root with ws upgrade).
// Persists sessionId in sessionStorage so a tab reload resumes the session and the
// server replays a snapshot. Auto-reconnects with backoff.

const SESSION_STORAGE_KEY = "ce-ui.sessionId";
const TICKHZ_STORAGE_KEY = "ce-ui.tickHz";
const RECONNECT_MS = 500;
// Backoff ceiling + the "this connection was healthy" threshold. A socket that
// stays open past STABLE_MS resets the backoff; one that dies sooner ramps it.
// 30s ceiling so a long engine outage settles to ~2 attempts/min per tab (with
// jitter), not ~6/min — much lighter on a recovering engine.
const RECONNECT_MAX_MS = 30000;
const STABLE_MS = 5000;

// --- cross-tab session uniqueness -------------------------------------------
//
// sessionId lives in sessionStorage so a tab RELOAD resumes its engine session
// (keeps subscriptions + skips a full re-bootstrap). But "Duplicate Tab" copies
// sessionStorage wholesale, so the duplicate would resume the SAME session —
// the engine rebinds the session to the last socket and the two tabs'
// subscription sets fight, leaving one tab with no value frames.
//
// We use a BroadcastChannel to detect this: only CURRENTLY-RUNNING tabs reply
// to an ownership query. So on startup a tab asks "does anyone already own
// sessionId X?" — a live duplicate's original answers (→ start fresh), but
// after a plain reload nobody answers (the original is gone → safe to resume).

const SESSION_CHANNEL = "ce-ui.session-ownership";
// Per-tab id, fresh every load (NOT persisted) — identifies this tab in
// ownership exchanges.
const tabId = `${Math.floor(performance.now())}-${Math.trunc(performance.timeOrigin) % 100000}`;

interface OwnershipQuery {
  kind: "own?";
  sessionId: string;
  from: string;
}
interface OwnershipReply {
  kind: "owned";
  sessionId: string;
  by: string;
}
type OwnershipMsg = OwnershipQuery | OwnershipReply;

let sessionChannel: BroadcastChannel | null = null;
// The sessionId this tab actively owns (set once the engine confirms it via the
// schema message). Used to answer other tabs' ownership queries.
let activeOwnedSessionId: string | null = null;

function ensureChannel(): BroadcastChannel | null {
  if (sessionChannel) return sessionChannel;
  if (typeof BroadcastChannel === "undefined") return null;
  sessionChannel = new BroadcastChannel(SESSION_CHANNEL);
  sessionChannel.onmessage = (ev: MessageEvent<OwnershipMsg>) => {
    const m = ev.data;
    if (m.kind === "own?" && m.from !== tabId && m.sessionId === activeOwnedSessionId) {
      // Someone's asking about a session we actively hold → claim it.
      sessionChannel?.postMessage({ kind: "owned", sessionId: m.sessionId, by: tabId });
    }
  };
  return sessionChannel;
}

// Resolve whether `candidate` is safe to resume. Resolves true if no other live
// tab claims it within the timeout (→ resume), false if a live owner answers
// (→ start fresh). No channel support → assume safe (single-context browsers).
function isSessionFree(candidate: string, timeoutMs = 180): Promise<boolean> {
  const ch = ensureChannel();
  if (!ch) return Promise.resolve(true);
  return new Promise((resolve) => {
    let settled = false;
    const onReply = (ev: MessageEvent<OwnershipMsg>) => {
      const m = ev.data;
      if (m.kind === "owned" && m.sessionId === candidate && m.by !== tabId && !settled) {
        settled = true;
        ch.removeEventListener("message", onReply);
        resolve(false); // taken by a live tab
      }
    };
    ch.addEventListener("message", onReply);
    ch.postMessage({ kind: "own?", sessionId: candidate, from: tabId } satisfies OwnershipQuery);
    window.setTimeout(() => {
      if (settled) return;
      settled = true;
      ch.removeEventListener("message", onReply);
      resolve(true); // nobody claimed it → free to resume
    }, timeoutMs);
  });
}

// Presence frames relayed by the engine (opaque `state`). See the Presence
// section of the WS protocol docs in openapi.yaml. Direct-mode-only in v1 —
// not part of the generic EngineTransport/EngineStream interface (a bridge
// transport drops presence; see slice-1-wiresheet-transport-seam.md).
export interface PresenceMsg {
  type: "presence";
  sessionId: string;
  state: unknown;
}
export interface PresenceSnapshotMsg {
  type: "presenceSnapshot";
  presences: Array<{ sessionId: string; state: unknown }>;
}
export interface PresenceLeftMsg {
  type: "presenceLeft";
  sessionId: string;
}

export interface DirectPresenceHandlers {
  onPresence(msg: PresenceMsg): void;
  onPresenceSnapshot(msg: PresenceSnapshotMsg): void;
  onPresenceLeft(msg: PresenceLeftMsg): void;
}

// Direct-mode-only stream surface: the generic `EngineStream` (subscriptions +
// tick rate + close) PLUS presence publish/subscribe, which only exists when
// talking straight to a CE over its own WS. `CeEditor` narrows to this type
// only when its transport is (or behaves like) a `DirectTransport`.
export interface DirectEngineStream extends EngineStream {
  publishPresence(state: unknown): void;
}

class DirectStream implements DirectEngineStream {
  private ws: WebSocket | null = null;
  private reconnectTimer: number | null = null;
  private explicitlyClosed = false;
  private subscribedComponents = new Set<number>();
  private desiredSubscribed = new Set<number>();
  // Property-level subscription (alongside component-level) — used for exposed
  // ports, where we want a single off-canvas prop's value, not its whole
  // component. Diffed/sent the same way as components.
  private subscribedProps = new Set<number>();
  private desiredSubscribedProps = new Set<number>();
  private _sessionId: string | null = null;
  // Highest topology `seq` we've received. Sent on reconnect via `lastSeq` so the server
  // can replay missed topology events from its ring buffer instead of forcing a full
  // re-bootstrap.
  private lastSeq: number | null = null;
  // Desired value/status push rate for this session. null = use engine
  // default. Persisted so a reconnect re-applies it via the configure
  // message. Live changes also go out as a `setRate` message.
  private tickHz: number | null = null;
  // Reconnect backoff state.
  private reconnectDelay = RECONNECT_MS;
  private openedAt: number | null = null;
  private url: string;
  private h: StreamHandlers;
  private presenceHandlers: DirectPresenceHandlers;

  constructor(url: string, h: StreamHandlers, presence: DirectPresenceHandlers) {
    this.url = url;
    this.h = h;
    this.presenceHandlers = presence;
    try {
      const savedHz = window.localStorage.getItem(TICKHZ_STORAGE_KEY);
      if (savedHz != null) {
        const n = Number(savedHz);
        if (Number.isFinite(n) && n >= 1 && n <= 1000) this.tickHz = n;
      }
    } catch {
      /* ignore */
    }
    try {
      this._sessionId = window.sessionStorage.getItem(SESSION_STORAGE_KEY);
    } catch {
      this._sessionId = null;
    }
  }

  // Run the cross-tab ownership check once, before the first connect, so a
  // duplicated tab drops the copied sessionId and gets its own session. Only
  // gates the FIRST connect — reconnects keep the established session.
  private resumeChecked = false;
  private async ensureResumeAllowed() {
    if (this.resumeChecked) return;
    this.resumeChecked = true;
    if (!this._sessionId) return; // nothing to resume anyway
    const free = await isSessionFree(this._sessionId);
    if (!free) {
      // Another live tab owns this session → don't steal it. Drop the copied
      // id and clear it from storage so we mint a fresh session instead.
      recordEvent("ws-open", `session ${this._sessionId.slice(0, 8)} owned by another tab → fresh`);
      this._sessionId = null;
      try {
        window.sessionStorage.removeItem(SESSION_STORAGE_KEY);
      } catch {
        /* ignore */
      }
    }
  }

  async connect() {
    if (this.ws) return;
    this.h.onStatus("connecting");
    await this.ensureResumeAllowed();
    if (this.ws) return; // a reconnect may have raced in during the await
    this.explicitlyClosed = false;
    const ws = new WebSocket(this.url);
    ws.binaryType = "arraybuffer";
    this.ws = ws;
    ws.onopen = () => {
      metrics.wsConnected = true;
      this.openedAt = performance.now();
      this.h.onStatus("open");
      // First message: configure (with sessionId + lastSeq if we have them — server
      // resumes the session AND replays any topology events we missed since `lastSeq`).
      // tickHz sets this session's value/status push rate (clamped [1,1000] by
      // the engine; engine default CENG_REST_TICK_HZ = 10 Hz). We persist the
      // user's preference and send it on connect so a reconnect keeps the rate.
      const msg: {
        type: "configure";
        sessionId?: string;
        lastSeq?: number;
        tickHz?: number;
      } = { type: "configure" };
      if (this._sessionId) msg.sessionId = this._sessionId;
      if (this.lastSeq != null) msg.lastSeq = this.lastSeq;
      if (this.tickHz != null) msg.tickHz = this.tickHz;
      ws.send(JSON.stringify(msg));
      recordEvent(
        "ws-open",
        `→ configure${this._sessionId ? ` sid=${this._sessionId.slice(0, 8)}` : ""}${
          this.lastSeq != null ? ` lastSeq=${this.lastSeq}` : ""
        }${this.tickHz != null ? ` tickHz=${this.tickHz}` : ""}`,
      );
      // On reconnect with a fresh session the server's subscription set was wiped.
      // The flushSubscriptions call below re-sends desired subs. On resume the server
      // already has them; the diff will produce no message and a snapshot will land
      // automatically.
      this.subscribedComponents.clear();
    };
    ws.onclose = () => {
      metrics.wsConnected = false;
      metrics.reconnectCount++;
      this.ws = null;
      // Exponential backoff that resets only when a connection was STABLE
      // (stayed open past the stability threshold). A connection that opens
      // and immediately closes — e.g. the engine rejecting a session, which
      // is exactly what caused the 67k-reconnect storm on the duplicate-tab
      // session collision — ramps the delay up fast instead of tight-looping.
      const lived = this.openedAt != null ? performance.now() - this.openedAt : 0;
      this.openedAt = null;
      if (lived >= STABLE_MS) {
        this.reconnectDelay = RECONNECT_MS; // healthy session → reset
      } else {
        this.reconnectDelay = Math.min(this.reconnectDelay * 2, RECONNECT_MAX_MS);
      }
      recordEvent("ws-close", `connection closed (lived ${lived.toFixed(0)}ms, next in ${this.reconnectDelay}ms)`);
      this.h.onStatus("closed");
      if (!this.explicitlyClosed) this.scheduleReconnect();
    };
    ws.onerror = () => {
      /* close handler will run too */
    };
    ws.onmessage = (ev) => this.handleMessage(ev.data);
  }

  close() {
    this.explicitlyClosed = true;
    if (this.reconnectTimer != null) {
      window.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
  }

  private scheduleReconnect() {
    if (this.reconnectTimer != null) return;
    // Don't hammer while the tab is hidden — a backgrounded tab reconnecting
    // every few seconds is pure noise (and N background tabs are a thundering
    // herd on a recovering engine). Wait for visibility instead; reconnect
    // immediately when the tab is shown again.
    if (typeof document !== "undefined" && document.hidden) {
      const onVisible = () => {
        if (document.hidden) return;
        document.removeEventListener("visibilitychange", onVisible);
        if (!this.explicitlyClosed && !this.ws) void this.connect();
      };
      document.addEventListener("visibilitychange", onVisible);
      return;
    }
    // Full jitter: random in [0, reconnectDelay]. De-synchronizes multiple
    // tabs so they don't retry in lockstep against the engine.
    const delay = Math.random() * this.reconnectDelay;
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      void this.connect();
    }, delay);
  }

  /**
   * Set this session's value/status push rate (Hz). Clamped [1,1000]; the
   * engine clamps too. Persisted so a reconnect re-applies it. Sends a live
   * `setRate` message if the socket is open; otherwise the next configure
   * carries it.
   */
  setTickHz(hz: number) {
    const clamped = Math.max(1, Math.min(1000, Math.round(hz)));
    this.tickHz = clamped;
    try {
      window.localStorage.setItem(TICKHZ_STORAGE_KEY, String(clamped));
    } catch {
      /* ignore */
    }
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: "setRate", tickHz: clamped }));
      recordEvent("rest", `→ setRate ${clamped}Hz`);
    }
    diagGauges.tickHz = clamped;
  }

  getTickHz(): number | null {
    return this.tickHz;
  }

  /**
   * Publish opaque presence state for this session. The engine relays it
   * verbatim to other sessions (last-write-wins, ≤4 KB). No-op if the socket
   * isn't open — presence is ephemeral, a missed publish self-heals on the
   * next selection change. Direct-mode-only (see DirectEngineStream).
   */
  publishPresence(state: unknown) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(JSON.stringify({ type: "presence", state }));
  }

  /**
   * Diff-and-send subscribe / unsubscribe. Caller sets the desired component set; we
   * only emit the delta vs. what the server currently holds.
   */
  setSubscriptions(desired: Set<number>) {
    this.desiredSubscribed = desired;
    this.flushSubscriptions();
  }

  /** Property-level subscription (exposed ports). Diff-and-send like components. */
  setPropSubscriptions(desired: Set<number>) {
    this.desiredSubscribedProps = desired;
    this.flushSubscriptions();
  }

  private flushSubscriptions() {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    // Component-level deltas. Diff in lib/subscriptions (tested).
    const { added, removed } = diffSets(this.subscribedComponents, this.desiredSubscribed);
    if (added.length > 0) {
      this.ws.send(JSON.stringify({ type: "subscribe", components: added }));
      for (const u of added) this.subscribedComponents.add(u);
      recordEvent("subscribe", `+[${added.join(",")}]`);
    }
    if (removed.length > 0) {
      this.ws.send(JSON.stringify({ type: "unsubscribe", components: removed }));
      for (const u of removed) this.subscribedComponents.delete(u);
      recordEvent("unsubscribe", `-[${removed.join(",")}]`);
    }
    // Property-level deltas (for exposed ports).
    const { added: addedP, removed: removedP } = diffSets(
      this.subscribedProps,
      this.desiredSubscribedProps,
    );
    if (addedP.length > 0) {
      this.ws.send(JSON.stringify({ type: "subscribe", properties: addedP }));
      for (const u of addedP) this.subscribedProps.add(u);
      recordEvent("subscribe", `props +[${addedP.join(",")}]`);
    }
    if (removedP.length > 0) {
      this.ws.send(JSON.stringify({ type: "unsubscribe", properties: removedP }));
      for (const u of removedP) this.subscribedProps.delete(u);
      recordEvent("unsubscribe", `props -[${removedP.join(",")}]`);
    }
  }

  /** Most-recently observed sessionId; exposed so REST mutations can send the
   *  X-CE-Session header for change attribution. */
  get sessionId(): string | null {
    return this._sessionId;
  }

  private handleMessage(data: ArrayBuffer | string) {
    const isBinary = typeof data !== "string";
    const bytes = isBinary ? (data as ArrayBuffer).byteLength : (data as string).length;
    recordMessage(bytes, isBinary);
    const t0 = performance.now();
    try {
      this.dispatchMessage(data);
    } finally {
      recordParse(performance.now() - t0);
    }
  }

  private dispatchMessage(data: ArrayBuffer | string) {
    if (typeof data === "string") {
      let msg: { type?: string; seq?: number } & Record<string, unknown>;
      try {
        msg = JSON.parse(data) as typeof msg;
      } catch {
        return;
      }
      diagRecordMessage(msg.type ?? "unknown", data.length);
      if (msg.type === "schema") {
        const s = msg as unknown as SchemaMessage;
        // Persist sessionId for resume.
        if (s.sessionId) {
          this._sessionId = s.sessionId;
          // Claim ownership for cross-tab arbitration: from now on we answer
          // other tabs' "who owns X?" queries for this id, so a duplicate
          // started later sees it taken and goes fresh.
          activeOwnedSessionId = s.sessionId;
          ensureChannel();
          try {
            window.sessionStorage.setItem(SESSION_STORAGE_KEY, s.sessionId);
          } catch {
            /* private mode etc — non-fatal */
          }
        }
        // The schema's currentSeq is our baseline — every topology event we receive
        // afterwards should have seq > currentSeq.
        if (typeof s.currentSeq === "number") {
          this.lastSeq = s.currentSeq;
          metrics.lastSeq = s.currentSeq;
        }
        metrics.sessionId = s.sessionId ?? "";
        recordEvent(
          "schema",
          `sid=${s.sessionId?.slice(0, 8) ?? "—"} seq=${s.currentSeq} props=${
            s.properties?.length ?? 0
          }${s.resumed ? " (resumed)" : ""}`,
        );
        this.h.onSchema(s);
        // After schema lands we have the topology; re-issue any pending subs.
        this.flushSubscriptions();
        // Re-assert the push rate. The engine honors `configure.tickHz` on a
        // FRESH session but appears to drop it on RESUME (a resumed session
        // reverts to the server default ~tickHz, so the client thinks it's at
        // N Hz while the engine streams at default — the "rate honored
        // sometimes" symptom across reconnects). A single setRate here is
        // safe (the #12 crash was sustained spam, not one call) and makes the
        // requested rate stick across reconnects/resumes.
        if (this.tickHz != null) {
          this.ws?.send(JSON.stringify({ type: "setRate", tickHz: this.tickHz }));
          recordEvent("rest", `→ setRate ${this.tickHz}Hz (assert post-schema)`);
        }
        return;
      }
      if (msg.type === "topologyAdded" || msg.type === "topologyRemoved" || msg.type === "topologyChanged") {
        const t = msg as unknown as TopologyMsg;
        // Gap detection: every message bumps seq by exactly 1. If we see a hole, we
        // missed something — drop the socket so we reconnect-with-resume (with our
        // current lastSeq) and the server replays from its ring buffer.
        if (this.lastSeq != null && t.seq !== this.lastSeq + 1) {
          // Gap. Force reconnect.
          this.ws?.close();
          return;
        }
        this.lastSeq = t.seq;
        metrics.lastSeq = t.seq;
        if (t.type === "topologyAdded") recordTopology("added");
        else if (t.type === "topologyRemoved") recordTopology("removed");
        else recordTopology("changed");
        recordEvent("topology", summarizeTopology(t));
        this.h.onTopology(t);
        return;
      }
      // Presence relay (opaque collaborator state). The engine never echoes
      // our own presence back, so anything here is another session.
      // Direct-mode-only — routed to the presence handlers, not StreamHandlers.
      if (msg.type === "presence") {
        const p = msg as unknown as PresenceMsg;
        recordEvent("rest", `presence ← ${p.sessionId?.slice(0, 8)}`);
        this.presenceHandlers.onPresence(p);
        return;
      }
      if (msg.type === "presenceSnapshot") {
        const p = msg as unknown as PresenceSnapshotMsg;
        recordEvent("rest", `presenceSnapshot (${p.presences?.length ?? 0})`);
        this.presenceHandlers.onPresenceSnapshot(p);
        return;
      }
      if (msg.type === "presenceLeft") {
        const p = msg as unknown as PresenceLeftMsg;
        recordEvent("rest", `presenceLeft ${p.sessionId?.slice(0, 8)}`);
        this.presenceHandlers.onPresenceLeft(p);
        return;
      }
      if (msg.type === "presenceError") {
        recordEvent("rest", `presenceError: ${(msg as { reason?: string }).reason ?? "?"}`);
        return;
      }
      return;
    }
    // Binary value frame.
    const frame = decodeBinaryFrame(data);
    if (frame.msgType !== MSG_UPDATE && frame.msgType !== MSG_SNAPSHOT) return;
    let n = 0;
    let valUids = 0;
    let statusUids = 0;
    for (const s of frame.sections) {
      n += s.uids.length;
      if (s.typeTag === TYPE_STATUS) {
        statusUids += s.uids.length;
        diagRecordStatusUids(s.uids);
      } else {
        valUids += s.uids.length;
        diagRecordValueUids(s.uids);
      }
    }
    recordValueCount(n, frame.sections.length, data.byteLength);
    diagRecordMessage("binaryFrame", data.byteLength);
    diagRecordFrame(valUids, statusUids, data.byteLength);
    recordEvent(
      "frame",
      `${frame.msgType === MSG_SNAPSHOT ? "snapshot" : "update"} ${valUids}v +${statusUids}s · ${frame.sections.length} sec · ${data.byteLength}B`,
    );
    this.h.onFrame(frame);
  }
}

function summarizeTopology(t: TopologyMsg): string {
  if (t.type === "topologyAdded") {
    // Include kind + parent so the events log names *what* is being added and
    // *where* — e.g. a component churned every tick shows its type + folder.
    const comps = t.components
      .map((c) => `${c.uid}:${c.kind}@${c.parent ?? "?"}`)
      .join(", ");
    return `+ comp[${comps}] edge[${t.edges.map((e) => e.uid).join(",")}]`;
  }
  if (t.type === "topologyRemoved") {
    return `- comp[${t.componentUids.join(",")}] edge[${t.edgeUids.join(",")}]`;
  }
  return `~ comp[${t.components.map((c) => `${c.uid}@${c.parent ?? "?"}`).join(",")}]`;
}

// DirectTransport — the default EngineTransport, reproducing today's
// direct-to-CE behavior. Constructed with no args by default (uses the
// module-level BASE set via setEngineBase, and derives the WS URL from it);
// a bridge transport implements EngineTransport independently and is injected
// via CeEditor's `transport` prop instead of constructing this class.
export class DirectTransport implements EngineTransport {
  request(req: EngineRequest): Promise<unknown> {
    return directRequest(req);
  }

  // `wsUrl` defaults to `wsUrlFromBase(BASE)` (today's behavior: derive the
  // socket URL from the REST origin set via setEngineBase). `presence` is the
  // direct-mode-only presence surface (CeEditor wires it when its transport
  // is a DirectTransport; other transports don't get presence in v1).
  openStream(handlers: StreamHandlers, presence?: DirectPresenceHandlers): DirectEngineStream {
    const url = ORIGIN ? wsUrlFromBase(ORIGIN) : defaultWsUrl();
    const stream = new DirectStream(url, handlers, presence ?? noopPresenceHandlers);
    void stream.connect();
    return stream;
  }
}

const noopPresenceHandlers: DirectPresenceHandlers = {
  onPresence: () => {},
  onPresenceSnapshot: () => {},
  onPresenceLeft: () => {},
};
