// Ref-style counters. The panel reads via rAF; nothing here participates in React
// reconciliation under load.

// --- Event log ---
//
// A ring buffer of recent wire events (WS messages + REST mutations) the UI can
// show in an "events" panel. Each event is one line: timestamp + kind + short
// detail. Kept here (module-level, mutated in place) so logging is O(1) at the
// call site and consumers can render via rAF without participating in React
// reconciliation under load.

export type EventKind =
  | "ws-open"
  | "ws-close"
  | "schema"
  | "subscribe"
  | "unsubscribe"
  | "frame"
  | "topology"
  | "rest";

export interface WireEvent {
  t: number;        // performance.now() at recording time
  kind: EventKind;
  text: string;     // pre-formatted summary (one line)
}

const EVENT_CAP = 500;
export const events: WireEvent[] = [];
// Bumped on every recordEvent so the panel knows when to redraw without
// diffing the array contents.
export const eventsVersion = { v: 0 };

export function recordEvent(kind: EventKind, text: string) {
  events.push({ t: performance.now(), kind, text });
  if (events.length > EVENT_CAP) events.splice(0, events.length - EVENT_CAP);
  eventsVersion.v++;
}

export function clearEvents() {
  events.length = 0;
  eventsVersion.v++;
}

export const metrics = {
  // WS state
  wsConnected: false,
  reconnectCount: 0,
  sessionId: "" as string,
  lastSeq: 0,

  // Counters since last 1s sample
  msgsThisSec: 0,
  bytesThisSec: 0,
  valuesThisSec: 0,
  framesThisSec: 0,

  // 1s rolling samples
  msgsPerSec: 0,
  bytesPerSec: 0,
  valuesPerSec: 0,
  framesPerSec: 0,

  // Frame timing
  fps: 0,
  frameMs: 0,
  maxFrameMs: 0,
  longFramesPerSec: 0,

  // Last binary frame stats
  lastFrameValues: 0,
  lastFrameSections: 0,
  lastFrameBytes: 0,

  // Wire parse time rolling avg (ms)
  parseAvgMs: 0,

  // Topology event counts
  topoAdded: 0,
  topoRemoved: 0,
  topoChanged: 0,

  // Viewport
  zoom: 1,
  panX: 0,
  panY: 0,

  // Last batch of select-change events from React Flow's onNodesChange. Captured here
  // so the ClickDebugger can show them next to each click without needing console.
  lastSelChange: "" as string,
  lastSelChangeAt: 0 as number,

  // Subscription / DOM counts (driven by App.tsx)
  subscribedComponents: 0,
  totalComponents: 0,
  domNodes: 0,
  domEdges: 0,

  // Bytes-per-sec sparkline, 30 samples deep.
  bytesSpark: new Float32Array(30),
};

export function recordMessage(bytes: number, isBinary: boolean) {
  metrics.msgsThisSec++;
  metrics.bytesThisSec += bytes;
  if (isBinary) metrics.framesThisSec++;
}

export function recordValueCount(n: number, sections: number, bytes: number) {
  metrics.valuesThisSec += n;
  metrics.lastFrameValues = n;
  metrics.lastFrameSections = sections;
  metrics.lastFrameBytes = bytes;
}

export function recordParse(ms: number) {
  const alpha = 0.1;
  metrics.parseAvgMs = metrics.parseAvgMs === 0 ? ms : metrics.parseAvgMs * (1 - alpha) + ms * alpha;
}

export function recordTopology(kind: "added" | "removed" | "changed") {
  if (kind === "added") metrics.topoAdded++;
  else if (kind === "removed") metrics.topoRemoved++;
  else metrics.topoChanged++;
}

let lastSec = performance.now();
let frames = 0;
let lastFrameTs = performance.now();
let frameMsAccum = 0;
let frameMsMax = 0;
let longFramesAccum = 0;
const LONG_FRAME_MS = 25;

export function tickInstrumentation(now: number) {
  const dt = now - lastFrameTs;
  lastFrameTs = now;
  frames++;
  frameMsAccum += dt;
  if (dt > frameMsMax) frameMsMax = dt;
  if (dt > LONG_FRAME_MS) longFramesAccum++;

  if (now - lastSec >= 1000) {
    const sec = (now - lastSec) / 1000;
    metrics.fps = frames / sec;
    metrics.frameMs = frameMsAccum / frames;
    metrics.maxFrameMs = frameMsMax;
    metrics.longFramesPerSec = longFramesAccum;
    frames = 0;
    frameMsAccum = 0;
    frameMsMax = 0;
    longFramesAccum = 0;
    lastSec = now;

    metrics.msgsPerSec = metrics.msgsThisSec;
    metrics.bytesPerSec = metrics.bytesThisSec;
    metrics.valuesPerSec = metrics.valuesThisSec;
    metrics.framesPerSec = metrics.framesThisSec;
    metrics.msgsThisSec = 0;
    metrics.bytesThisSec = 0;
    metrics.valuesThisSec = 0;
    metrics.framesThisSec = 0;

    const s = metrics.bytesSpark;
    s.copyWithin(0, 1);
    s[s.length - 1] = metrics.bytesPerSec;

    // DOM counts — querySelectorAll is cheap once a second.
    metrics.domNodes = document.querySelectorAll(".react-flow__node").length;
    metrics.domEdges = document.querySelectorAll(".react-flow__edge").length;
  }
}
