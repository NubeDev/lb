// Deep diagnostics — the layer below the PerfPanel's headline numbers. Built to
// answer "why does it hang / lag / spike" when the coarse FPS number lies.
//
// Everything here is mutate-in-place and O(1) at the call site. A snapshot is
// computed only when someone asks (the reporter, on an interval) so the hot
// paths (every WS frame, every React render) stay cheap.
//
// The data is streamed to a dev-server sink (POST /__diag) so it can be read
// off the filesystem while the UI is live — see diagReporter() and the Vite
// plugin in vite.config.ts.

// The `/__diag` dev sink exists only under the standalone ce-ui Vite server;
// the rubix extension has no such endpoint, so reporting is off here.
const DIAG_SINK_ENABLED = false;

// ---- ring buffers -----------------------------------------------------------

class Ring {
  private buf: Float64Array;
  private i = 0;
  private filled = 0;
  constructor(size: number) {
    this.buf = new Float64Array(size);
  }
  push(v: number) {
    this.buf[this.i] = v;
    this.i = (this.i + 1) % this.buf.length;
    if (this.filled < this.buf.length) this.filled++;
  }
  /** Sorted copy of the live values. */
  sorted(): number[] {
    const out: number[] = [];
    for (let k = 0; k < this.filled; k++) out.push(this.buf[k]);
    out.sort((a, b) => a - b);
    return out;
  }
  get count() {
    return this.filled;
  }
  clear() {
    this.i = 0;
    this.filled = 0;
  }
}

function pct(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length));
  return sorted[idx];
}

// ---- frame timing -----------------------------------------------------------
// Driven by a self-contained rAF loop (NOT the PerfPanel's) so it measures the
// true frame cadence even if the panel is collapsed or unmounted. Each tick
// records the delta since the previous frame.

const frameDurations = new Ring(600); // ~10s at 60fps
let lastFrameTs = 0;
let frameRafRunning = false;

function frameTick(now: number) {
  if (lastFrameTs !== 0) frameDurations.push(now - lastFrameTs);
  lastFrameTs = now;
  if (frameRafRunning) requestAnimationFrame(frameTick);
}

// ---- long tasks -------------------------------------------------------------
// PerformanceObserver 'longtask' = any main-thread block ≥50ms. This is the
// gold-standard "what froze the UI" signal — it fires regardless of rAF, so a
// 300ms synchronous stall that the FPS counter would paper over shows up here.

interface LongTask {
  start: number;    // ms since timeOrigin
  duration: number; // ms
}
const longTasks: LongTask[] = [];
const LONGTASK_KEEP = 100;
let longTaskTotalMs = 0;
let longTaskCount = 0;
// Per-window (reset each report) so the panel shows live improvement, not a
// lifetime-cumulative number that only ever grows.
let longTaskCountWindow = 0;
let longTaskMsWindow = 0;

// ---- message + byte accounting ----------------------------------------------
// Per message-type counters. Reset each reporting window to yield per-sec rates.

interface TypeStat {
  count: number;
  bytes: number;
}
const msgStats = new Map<string, TypeStat>();

export function diagRecordMessage(type: string, bytes: number) {
  let s = msgStats.get(type);
  if (!s) {
    s = { count: 0, bytes: 0 };
    msgStats.set(type, s);
  }
  s.count++;
  s.bytes += bytes;
}

// ---- per-uid value-update accounting ----------------------------------------
// Which property uids are actually changing, and how often. This is the tool
// for "100kb/s when nothing's happening" — it names the chattering props.
// Keyed by uid → update count in the current window.

const uidUpdateCounts = new Map<number, number>();
let valueUpdatesThisWindow = 0;
let statusUpdatesThisWindow = 0;

export function diagRecordValueUids(uids: ArrayLike<number>) {
  for (let i = 0; i < uids.length; i++) {
    const u = uids[i];
    uidUpdateCounts.set(u, (uidUpdateCounts.get(u) ?? 0) + 1);
    valueUpdatesThisWindow++;
  }
}

export function diagRecordStatusUids(uids: ArrayLike<number>) {
  statusUpdatesThisWindow += uids.length;
}

// ---- frame anatomy ----------------------------------------------------------
// Per-frame value/status counts + bytes, so we can see whether a byte spike is
// "more frames" or "fatter frames", and whether STATUS sections dominate.

const frameValueCounts = new Ring(300);
const frameStatusCounts = new Ring(300);
const frameByteSizes = new Ring(300);
let framesThisWindow = 0;

export function diagRecordFrame(valueUids: number, statusUids: number, bytes: number) {
  frameValueCounts.push(valueUids);
  frameStatusCounts.push(statusUids);
  frameByteSizes.push(bytes);
  framesThisWindow++;
}

// ---- React render accounting ------------------------------------------------
// Component-id → render count in the current window. A re-render storm (every
// node re-rendering on every frame) shows as render counts ≈ frame count ×
// node count. Healthy: only changed components re-render.

const renderCounts = new Map<string, number>();
let totalRendersThisWindow = 0;

export function diagRecordRender(label: string) {
  renderCounts.set(label, (renderCounts.get(label) ?? 0) + 1);
  totalRendersThisWindow++;
}

// ---- gauges (set directly, not rate-based) ----------------------------------

export const diagGauges = {
  subscribedComponents: 0,
  visibleNodes: 0,
  totalComponents: 0,
  domNodes: 0,
  ghostNodes: 0,
  edges: 0,
  wsConnected: false,
  tickHz: 0,
  reconnects: 0,
  lastSeq: 0,
};

// ---- snapshot ---------------------------------------------------------------

export interface DiagSnapshot {
  ts: number;
  windowMs: number;
  frame: {
    fps: number;
    p50: number;
    p95: number;
    p99: number;
    max: number;
    samples: number;
  };
  longTasks: {
    countTotal: number;
    totalMs: number;
    countWindow: number; // new long tasks in this reporting window
    msWindow: number;    // blocked ms in this reporting window
    recent: LongTask[]; // last few, newest first
  };
  perSec: {
    frames: number;
    valueUpdates: number;
    statusUpdates: number;
    renders: number;
  };
  frameAnatomy: {
    avgValueUids: number;
    avgStatusUids: number;
    avgBytes: number;
    maxBytes: number;
  };
  messages: Array<{ type: string; perSec: number; bytesPerSec: number }>;
  topChattyUids: Array<{ uid: number; updatesPerSec: number }>;
  topRenderers: Array<{ label: string; rendersPerSec: number }>;
  gauges: typeof diagGauges;
}

function avg(r: Ring): number {
  const s = r.sorted();
  if (s.length === 0) return 0;
  let sum = 0;
  for (const v of s) sum += v;
  return sum / s.length;
}

let windowStart = 0;

export function diagSnapshot(nowMs: number): DiagSnapshot {
  const windowMs = windowStart === 0 ? 1000 : nowMs - windowStart;
  const secs = Math.max(0.001, windowMs / 1000);
  const fs = frameDurations.sorted();
  // fps from the median frame time is more honest than count/window because
  // it ignores idle gaps (tab blurred etc).
  const p50 = pct(fs, 50);
  const fps = p50 > 0 ? 1000 / p50 : 0;

  const messages = [...msgStats.entries()]
    .map(([type, s]) => ({
      type,
      perSec: s.count / secs,
      bytesPerSec: s.bytes / secs,
    }))
    .sort((a, b) => b.bytesPerSec - a.bytesPerSec);

  const topChattyUids = [...uidUpdateCounts.entries()]
    .map(([uid, c]) => ({ uid, updatesPerSec: c / secs }))
    .sort((a, b) => b.updatesPerSec - a.updatesPerSec)
    .slice(0, 20);

  const topRenderers = [...renderCounts.entries()]
    .map(([label, c]) => ({ label, rendersPerSec: c / secs }))
    .sort((a, b) => b.rendersPerSec - a.rendersPerSec)
    .slice(0, 20);

  const snap: DiagSnapshot = {
    ts: nowMs,
    windowMs,
    frame: {
      fps,
      p50,
      p95: pct(fs, 95),
      p99: pct(fs, 99),
      max: fs.length ? fs[fs.length - 1] : 0,
      samples: fs.length,
    },
    longTasks: {
      countTotal: longTaskCount,
      totalMs: longTaskTotalMs,
      countWindow: longTaskCountWindow,
      msWindow: longTaskMsWindow,
      recent: longTasks.slice(-8).reverse(),
    },
    perSec: {
      frames: framesThisWindow / secs,
      valueUpdates: valueUpdatesThisWindow / secs,
      statusUpdates: statusUpdatesThisWindow / secs,
      renders: totalRendersThisWindow / secs,
    },
    frameAnatomy: {
      avgValueUids: avg(frameValueCounts),
      avgStatusUids: avg(frameStatusCounts),
      avgBytes: avg(frameByteSizes),
      maxBytes: frameByteSizes.sorted().pop() ?? 0,
    },
    messages,
    topChattyUids,
    topRenderers,
    gauges: { ...diagGauges },
  };
  return snap;
}

// Reset the per-window accumulators (rates). Keep the gauges and the long-task
// running totals — those are cumulative by design.
export function diagResetWindow(nowMs: number) {
  windowStart = nowMs;
  msgStats.clear();
  uidUpdateCounts.clear();
  renderCounts.clear();
  valueUpdatesThisWindow = 0;
  statusUpdatesThisWindow = 0;
  framesThisWindow = 0;
  totalRendersThisWindow = 0;
  longTaskCountWindow = 0;
  longTaskMsWindow = 0;
  frameValueCounts.clear();
  frameStatusCounts.clear();
  frameByteSizes.clear();
}

// ---- lifecycle --------------------------------------------------------------

let started = false;
let observer: PerformanceObserver | null = null;

export function startDiagnostics() {
  if (started) return;
  started = true;
  windowStart = performance.now();

  frameRafRunning = true;
  lastFrameTs = 0;
  requestAnimationFrame(frameTick);

  if (typeof PerformanceObserver !== "undefined") {
    try {
      observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          longTasks.push({ start: entry.startTime, duration: entry.duration });
          if (longTasks.length > LONGTASK_KEEP) longTasks.shift();
          longTaskCount++;
          longTaskTotalMs += entry.duration;
          longTaskCountWindow++;
          longTaskMsWindow += entry.duration;
        }
      });
      observer.observe({ entryTypes: ["longtask"] });
    } catch {
      // longtask not supported in this browser — frame timing still works.
    }
  }
}

export function stopDiagnostics() {
  frameRafRunning = false;
  started = false;
  observer?.disconnect();
  observer = null;
}

// ---- reporter ---------------------------------------------------------------
// Streams a snapshot to the dev-server sink every intervalMs. The sink writes
// the latest snapshot to a file so it can be read while the UI runs. Also keeps
// an in-memory ring of recent snapshots for the in-UI DiagPanel.

const snapshotHistory: DiagSnapshot[] = [];
const SNAPSHOT_HISTORY = 60;
let reporterTimer: number | null = null;

export function getSnapshotHistory(): DiagSnapshot[] {
  return snapshotHistory;
}

export function startDiagReporter(intervalMs = 1000) {
  if (reporterTimer != null) return;
  const tick = () => {
    const now = performance.now();
    const snap = diagSnapshot(now);
    snapshotHistory.push(snap);
    if (snapshotHistory.length > SNAPSHOT_HISTORY) snapshotHistory.shift();
    diagResetWindow(now);
    // Fire-and-forget POST to the dev sink. keepalive so an in-flight report
    // survives a navigation. Failures are ignored — the sink only exists in
    // dev.
    // The `/__diag` sink only exists under the standalone ce-ui dev server;
    // inside the rubix extension there is no such endpoint, so skip the POST.
    if (DIAG_SINK_ENABLED) {
      try {
        void fetch("/__diag", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(snap),
          keepalive: true,
        }).catch(() => {});
      } catch {
        /* ignore */
      }
    }
    reporterTimer = window.setTimeout(tick, intervalMs);
  };
  reporterTimer = window.setTimeout(tick, intervalMs);
}

export function stopDiagReporter() {
  if (reporterTimer != null) {
    window.clearTimeout(reporterTimer);
    reporterTimer = null;
  }
}
