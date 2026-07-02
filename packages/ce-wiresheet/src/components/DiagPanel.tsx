import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { useStoreApi } from "@xyflow/react";
import { getSnapshotHistory, type DiagSnapshot } from "../lib/diagnostics";
import { metrics, tickInstrumentation } from "../lib/instrumentation";
import { useStructural } from "../lib/store";

// Single diagnostics panel (top-right). Two data planes in one collapsible box:
//   - LIVE (ref-driven via rAF, never re-renders React): connection, fps, frame
//     ms, throughput bytes/sec + sparkline, value rate, dom counts, topology seq.
//   - SNAPSHOT (sampled every 500ms while open): frame percentiles, long tasks,
//     render-storm detection, value-plane anatomy, bytes-by-message, chattiest
//     props, structure gauges — plus the push-rate control.
//
// Collapsed, it's just the connection dot + connected/disconnected. The rAF tick
// runs even while collapsed so that indicator stays live (and instrumentation
// keeps ticking); the 500ms snapshot poll only runs while expanded.

const COLLAPSED_KEY = "ce-ui.diagpanel.collapsed";

function fmtBytes(n: number): string {
  if (n < 1024) return `${n.toFixed(0)} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}

const rowS: CSSProperties = { display: "flex", justifyContent: "space-between", padding: "1px 0" };
const lab: CSSProperties = { color: "hsl(var(--muted-foreground))" };
const val: CSSProperties = { color: "hsl(var(--foreground))", fontVariantNumeric: "tabular-nums" };

export function DiagPanel({
  wsRef,
  autoRate,
  manualRate,
  onSetManualRate,
  onToggleAutoRate,
  embedded = false,
}: {
  wsRef: { setRate(hz: number): void; getRate(): number | null } | null;
  autoRate: boolean;
  manualRate: number;
  onSetManualRate: (hz: number) => void;
  onToggleAutoRate: () => void;
  // Embedded mode: render filling the parent (the bottom drawer) rather than as
  // a fixed top-right panel, and drop the collapsed-pill / own collapse control
  // (the drawer owns open/close).
  embedded?: boolean;
}) {
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    try {
      // Default collapsed — start as just the connection indicator.
      return window.localStorage.getItem(COLLAPSED_KEY) !== "0";
    } catch {
      return true;
    }
  });
  useEffect(() => {
    try {
      window.localStorage.setItem(COLLAPSED_KEY, collapsed ? "1" : "0");
    } catch {
      /* ignore */
    }
  }, [collapsed]);

  // In embedded mode the drawer is the open/closed control, so the panel is
  // never internally "collapsed" — it always renders its full body.
  const effCollapsed = embedded ? false : collapsed;

  const [snap, setSnap] = useState<DiagSnapshot | null>(null);
  const [copied, setCopied] = useState(false);

  // Non-reactive handle on the RF store so we can read the viewport transform in
  // the rAF loop WITHOUT subscribing (subscribing would re-render this panel on
  // every pan/zoom frame).
  const storeApi = useStoreApi();

  // Live refs (collapsed ones — rDot/rConn — are always mounted; the rest only
  // exist when expanded, and the setter no-ops on null).
  const rDot = useRef<HTMLSpanElement>(null);
  const rConn = useRef<HTMLSpanElement>(null);
  const rReconn = useRef<HTMLSpanElement>(null);
  const rSession = useRef<HTMLSpanElement>(null);
  const rSeq = useRef<HTMLSpanElement>(null);
  const rTopo = useRef<HTMLSpanElement>(null);
  const rFps = useRef<HTMLSpanElement>(null);
  const rFrame = useRef<HTMLSpanElement>(null);
  const rJank = useRef<HTMLSpanElement>(null);
  const rZoom = useRef<HTMLSpanElement>(null);
  const rMsgs = useRef<HTMLSpanElement>(null);
  const rBytes = useRef<HTMLSpanElement>(null);
  const rValues = useRef<HTMLSpanElement>(null);
  const rFrameLast = useRef<HTMLSpanElement>(null);
  const rParse = useRef<HTMLSpanElement>(null);
  const rNodes = useRef<HTMLSpanElement>(null);
  const rEdges = useRef<HTMLSpanElement>(null);
  const rCanvas = useRef<HTMLCanvasElement>(null);

  // rAF tick: drives instrumentation + all live refs. Runs for the panel's
  // lifetime regardless of collapse so the connection dot is always current.
  useEffect(() => {
    let raf = 0;
    const last = new Map<HTMLSpanElement, string>();
    const set = (el: HTMLSpanElement | null, v: string) => {
      if (!el) return;
      if (last.get(el) === v) return;
      last.set(el, v);
      el.textContent = v;
    };
    let lastConn: boolean | null = null;
    const tick = (t: number) => {
      tickInstrumentation(t);

      const tr = storeApi.getState().transform;
      metrics.zoom = tr[2];
      metrics.panX = tr[0];
      metrics.panY = tr[1];
      metrics.totalComponents = useStructural.getState().components.size;

      if (rDot.current && lastConn !== metrics.wsConnected) {
        rDot.current.style.background = metrics.wsConnected ? "hsl(var(--green))" : "hsl(var(--crit))";
        lastConn = metrics.wsConnected;
      }
      set(rConn.current, metrics.wsConnected ? "connected" : "disconnected");
      set(rReconn.current, String(metrics.reconnectCount));
      set(rSession.current, metrics.sessionId ? metrics.sessionId.slice(0, 8) : "—");
      set(rSeq.current, String(metrics.lastSeq));
      set(rTopo.current, `+${metrics.topoAdded} -${metrics.topoRemoved} ~${metrics.topoChanged}`);
      set(rFps.current, metrics.fps.toFixed(1));
      set(rFrame.current, metrics.frameMs.toFixed(2) + " ms");
      set(rJank.current, `${metrics.longFramesPerSec} (max ${metrics.maxFrameMs.toFixed(1)} ms)`);
      set(rZoom.current, metrics.zoom.toFixed(3));
      set(rMsgs.current, `${metrics.msgsPerSec} /s  (${metrics.framesPerSec} bin)`);
      set(rBytes.current, fmtBytes(metrics.bytesPerSec) + "/s");
      set(rValues.current, `${metrics.valuesPerSec} /s`);
      set(
        rFrameLast.current,
        `${metrics.lastFrameValues} v · ${metrics.lastFrameSections} sec · ${fmtBytes(metrics.lastFrameBytes)}`,
      );
      set(rParse.current, metrics.parseAvgMs.toFixed(3) + " ms");
      set(rNodes.current, `${metrics.domNodes} / ${metrics.totalComponents}`);
      set(rEdges.current, String(metrics.domEdges));

      drawSparkline(rCanvas.current);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [storeApi]);

  // Deep snapshot poll — only while expanded.
  useEffect(() => {
    if (effCollapsed) return;
    const id = window.setInterval(() => {
      const hist = getSnapshotHistory();
      setSnap(hist.length ? hist[hist.length - 1] : null);
    }, 500);
    return () => window.clearInterval(id);
  }, [effCollapsed]);

  // Resolve a chatty uid to "componentName.propName" so the row is readable.
  const labelForUid = (uid: number): string => {
    const comps = useStructural.getState().components;
    for (const c of comps.values()) {
      for (const [pname, p] of Object.entries(c.properties)) {
        if (p.uid === uid) return `${c.name || c.type}.${pname}`;
      }
    }
    return String(uid);
  };

  // ---- collapsed: just the live connection indicator ----
  if (!embedded && collapsed) {
    return (
      <button
        onClick={() => setCollapsed(false)}
        title="Show diagnostics"
        style={{
          position: "fixed",
          top: 12,
          right: 12,
          zIndex: 31,
          display: "flex",
          alignItems: "center",
          gap: 7,
          padding: "6px 11px",
          background: "hsl(var(--card) / 0.92)",
          border: "1px solid hsl(var(--border))",
          borderRadius: 6,
          color: "hsl(var(--foreground))",
          fontSize: 11,
          fontFamily: "var(--font-mono)",
          cursor: "pointer",
        }}
      >
        <span
          ref={rDot}
          style={{ width: 8, height: 8, borderRadius: 4, background: "hsl(var(--crit))", display: "inline-block" }}
        />
        <span ref={rConn}>disconnected</span>
        <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>▾</span>
      </button>
    );
  }

  const rate = wsRef?.getRate() ?? null;
  // Storm = renders far outpacing frames AND high enough in absolute terms to
  // matter (the ratio alone false-positives at low push rates).
  const storms =
    snap != null &&
    snap.perSec.renders > Math.max(120, snap.perSec.frames * 4) &&
    snap.perSec.renders > (snap.gauges.visibleNodes || 1) * 2;
  const renderStorm = !!storms;
  const slowP95 = snap != null && snap.frame.p95 > 32;

  // ---- expanded: full panel ----
  const containerStyle: CSSProperties = embedded
    ? {
        position: "relative",
        width: "100%",
        height: "100%",
        background: "transparent",
        color: "hsl(var(--foreground))",
        fontSize: 11,
        fontFamily: "var(--font-mono)",
        display: "flex",
        flexDirection: "column",
      }
    : {
        position: "fixed",
        top: 12,
        right: 12,
        bottom: 12,
        width: 320,
        zIndex: 31,
        background: "hsl(var(--card) / 0.96)",
        border: "1px solid hsl(var(--border))",
        borderRadius: 6,
        color: "hsl(var(--foreground))",
        fontSize: 11,
        fontFamily: "var(--font-mono)",
        display: "flex",
        flexDirection: "column",
      };
  return (
    <div style={containerStyle}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 7,
          padding: "8px 10px",
          borderBottom: "1px solid hsl(var(--border))",
        }}
      >
        <span
          ref={rDot}
          style={{ width: 8, height: 8, borderRadius: 4, background: "hsl(var(--crit))", display: "inline-block" }}
        />
        <span ref={rConn} style={{ fontWeight: 600 }}>disconnected</span>
        <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>
          reconn <span ref={rReconn}>0</span>
        </span>
        <span style={{ flex: 1 }} />
        <button
          onClick={() => {
            const text = snap ? formatReport(snap, rate, labelForUid) : "(no snapshot yet)";
            void navigator.clipboard
              ?.writeText(text)
              .then(() => {
                setCopied(true);
                window.setTimeout(() => setCopied(false), 1200);
              })
              .catch(() => {});
          }}
          title="Copy a diagnostics report to clipboard"
          style={{
            background: copied ? "hsl(var(--cool))" : "hsl(var(--secondary))",
            border: "1px solid hsl(var(--border))",
            borderRadius: 3,
            color: copied ? "#fff" : "hsl(var(--foreground))",
            cursor: "pointer",
            fontSize: 10,
            fontFamily: "inherit",
            padding: "2px 8px",
            marginRight: 4,
          }}
        >
          {copied ? "copied ✓" : "copy"}
        </button>
        {!embedded && (
          <button
            onClick={() => setCollapsed(true)}
            title="Collapse to indicator"
            style={{
              background: "transparent",
              border: "none",
              color: "hsl(var(--muted-foreground))",
              cursor: "pointer",
              fontSize: 14,
            }}
          >
            ▴
          </button>
        )}
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: "8px 10px" }}>
        {/* Push rate control */}
        <Section title={autoRate ? "push rate (auto: follows zoom)" : "push rate (manual)"}>
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
            {[1, 5, 10, 30, 60].map((hz) => (
              <button
                key={hz}
                onClick={() => !autoRate && onSetManualRate(hz)}
                disabled={autoRate}
                title={autoRate ? "turn off auto-scale to set manually" : `${hz} Hz`}
                style={{
                  flex: 1,
                  padding: "3px 0",
                  background: !autoRate && manualRate === hz ? "hsl(var(--cool))" : "hsl(var(--secondary))",
                  color: !autoRate && manualRate === hz ? "#fff" : "hsl(var(--foreground))",
                  border: "1px solid hsl(var(--border))",
                  borderRadius: 3,
                  cursor: autoRate ? "default" : "pointer",
                  opacity: autoRate ? 0.45 : 1,
                  fontSize: 10,
                  fontFamily: "inherit",
                }}
              >
                {hz}Hz
              </button>
            ))}
          </div>
          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 5,
              marginTop: 5,
              color: "hsl(var(--foreground))",
              fontSize: 10,
              cursor: "pointer",
            }}
          >
            <input type="checkbox" checked={autoRate} onChange={onToggleAutoRate} />
            auto-scale rate with zoom
          </label>
          <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 9, marginTop: 3 }}>
            current: {rate ?? "engine default"} Hz
            {autoRate ? " (zoom-driven)" : " (manual)"}
          </div>
        </Section>

        {/* LIVE (ref-driven) */}
        <Section title="live">
          <div style={rowS}>
            <span style={lab}>FPS</span>
            <span style={val}>
              <span ref={rFps}>0</span>
              {"  "}(<span ref={rFrame}>0.00 ms</span>)
            </span>
          </div>
          <div style={rowS}>
            <span style={lab}>long frames /s</span>
            <span style={val} ref={rJank}>0 (max 0.0 ms)</span>
          </div>
          <div style={rowS}>
            <span style={lab}>zoom</span>
            <span style={val} ref={rZoom}>1.000</span>
          </div>
          <div style={rowS}>
            <span style={lab}>messages</span>
            <span style={val} ref={rMsgs}>0 /s</span>
          </div>
          <div style={rowS}>
            <span style={lab}>bytes</span>
            <span style={val} ref={rBytes}>0 B/s</span>
          </div>
          <canvas
            ref={rCanvas}
            width={296}
            height={36}
            style={{ width: "100%", height: 36, background: "hsl(var(--background))", border: "1px solid hsl(var(--secondary))", margin: "2px 0" }}
          />
          <div style={rowS}>
            <span style={lab}>values</span>
            <span style={val} ref={rValues}>0 /s</span>
          </div>
          <div style={rowS}>
            <span style={lab}>last frame</span>
            <span style={val} ref={rFrameLast}>—</span>
          </div>
          <div style={rowS}>
            <span style={lab}>parse (avg)</span>
            <span style={val} ref={rParse}>0.000 ms</span>
          </div>
          <div style={rowS}>
            <span style={lab}>nodes (DOM/total)</span>
            <span style={val} ref={rNodes}>0 / 0</span>
          </div>
          <div style={rowS}>
            <span style={lab}>edges (DOM)</span>
            <span style={val} ref={rEdges}>0</span>
          </div>
        </Section>

        <Section title="topology">
          <div style={rowS}>
            <span style={lab}>session</span>
            <span style={val} ref={rSession}>—</span>
          </div>
          <div style={rowS}>
            <span style={lab}>seq</span>
            <span style={val} ref={rSeq}>0</span>
          </div>
          <div style={rowS}>
            <span style={lab}>events (+/-/~)</span>
            <span style={val} ref={rTopo}>+0 -0 ~0</span>
          </div>
        </Section>

        {/* SNAPSHOT (deep) */}
        {snap == null ? (
          <div style={{ color: "hsl(var(--muted-foreground))", padding: "8px 0" }}>collecting…</div>
        ) : (
          <>
            <Section title="frames (windowed)">
              <Row k="fps (from p50)" v={snap.frame.fps.toFixed(1)} />
              <Row k="p50" v={`${snap.frame.p50.toFixed(1)} ms`} />
              <Row k="p95" v={`${snap.frame.p95.toFixed(1)} ms`} warn={slowP95} />
              <Row k="p99" v={`${snap.frame.p99.toFixed(1)} ms`} />
              <Row k="max" v={`${snap.frame.max.toFixed(1)} ms`} warn={snap.frame.max > 100} />
            </Section>

            <Section title="long tasks (≥50ms blocks)">
              <Row
                k="this window"
                v={`${snap.longTasks.countWindow} (${snap.longTasks.msWindow.toFixed(0)}ms)`}
                warn={snap.longTasks.countWindow > 0}
              />
              <Row
                k="lifetime"
                v={`${snap.longTasks.countTotal} / ${snap.longTasks.totalMs.toFixed(0)}ms`}
              />
              {snap.longTasks.recent.slice(0, 3).map((lt, i) => (
                <Row key={i} k={`  recent ${i + 1}`} v={`${lt.duration.toFixed(0)} ms`} warn />
              ))}
            </Section>

            <Section title="render pressure">
              <Row k="frames/sec" v={snap.perSec.frames.toFixed(1)} />
              <Row k="renders/sec" v={snap.perSec.renders.toFixed(0)} warn={renderStorm} />
              {renderStorm && (
                <div style={{ color: "hsl(var(--crit))", fontSize: 9, marginTop: 2 }}>
                  ⚠ re-render storm — renders ≫ frames
                </div>
              )}
            </Section>

            <Section title="value plane">
              <Row k="value updates/sec" v={snap.perSec.valueUpdates.toFixed(0)} />
              <Row k="status updates/sec" v={snap.perSec.statusUpdates.toFixed(0)} />
              <Row k="avg val uids/frame" v={snap.frameAnatomy.avgValueUids.toFixed(0)} />
              <Row k="avg status uids/frame" v={snap.frameAnatomy.avgStatusUids.toFixed(0)} />
              <Row k="avg frame bytes" v={fmtBytes(snap.frameAnatomy.avgBytes)} />
              <Row k="max frame bytes" v={fmtBytes(snap.frameAnatomy.maxBytes)} />
            </Section>

            <Section title="bytes/sec by message">
              {snap.messages.slice(0, 6).map((m) => (
                <Row key={m.type} k={m.type} v={`${fmtBytes(m.bytesPerSec)}/s`} />
              ))}
            </Section>

            <Section title="chattiest props">
              {snap.topChattyUids.slice(0, 8).map((u) => (
                <Row key={u.uid} k={labelForUid(u.uid)} v={`${u.updatesPerSec.toFixed(0)}/s`} />
              ))}
              {snap.topChattyUids.length === 0 && (
                <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 9 }}>nothing updating</div>
              )}
            </Section>

            <Section title="structure">
              <Row k="visible nodes" v={String(snap.gauges.visibleNodes)} />
              <Row k="subscribed (streaming)" v={String(snap.gauges.subscribedComponents)} />
              <Row k="ghost nodes" v={String(snap.gauges.ghostNodes)} />
              <Row k="edges" v={String(snap.gauges.edges)} />
              <Row k="total components" v={String(snap.gauges.totalComponents)} />
              <Row k="reconnects" v={String(snap.gauges.reconnects)} warn={snap.gauges.reconnects > 0} />
            </Section>
          </>
        )}
      </div>
    </div>
  );
}

// Compact, paste-friendly text report. Mirrors the panel so what's copied
// matches what's shown — plus raw JSON at the end for machine parsing.
function formatReport(
  s: DiagSnapshot,
  rate: number | null,
  labelForUid: (uid: number) => string,
): string {
  const lines: string[] = [];
  lines.push("=== ce-ui diagnostics ===");
  lines.push(`pushRate=${rate ?? "engine-default"}`);
  lines.push(
    `frame: fps=${s.frame.fps.toFixed(1)} p50=${s.frame.p50.toFixed(1)}ms p95=${s.frame.p95.toFixed(1)}ms p99=${s.frame.p99.toFixed(1)}ms max=${s.frame.max.toFixed(1)}ms`,
  );
  lines.push(
    `longTasks: window=${s.longTasks.countWindow} (${s.longTasks.msWindow.toFixed(0)}ms) lifetime=${s.longTasks.countTotal}/${s.longTasks.totalMs.toFixed(0)}ms`,
  );
  lines.push(
    `render: frames/s=${s.perSec.frames.toFixed(1)} renders/s=${s.perSec.renders.toFixed(0)}`,
  );
  lines.push(
    `value: upd/s=${s.perSec.valueUpdates.toFixed(0)} status/s=${s.perSec.statusUpdates.toFixed(0)} avgValUids=${s.frameAnatomy.avgValueUids.toFixed(0)} avgStatusUids=${s.frameAnatomy.avgStatusUids.toFixed(0)} avgBytes=${s.frameAnatomy.avgBytes.toFixed(0)} maxBytes=${s.frameAnatomy.maxBytes.toFixed(0)}`,
  );
  lines.push("bytes/s by msg:");
  for (const m of s.messages.slice(0, 6)) {
    lines.push(`  ${m.type}: ${m.bytesPerSec.toFixed(0)} B/s (${m.perSec.toFixed(0)}/s)`);
  }
  lines.push("chattiest props:");
  for (const u of s.topChattyUids.slice(0, 10)) {
    lines.push(`  ${labelForUid(u.uid)} (${u.uid}): ${u.updatesPerSec.toFixed(0)}/s`);
  }
  const g = s.gauges;
  lines.push(
    `structure: visible=${g.visibleNodes} ghost=${g.ghostNodes} edges=${g.edges} total=${g.totalComponents} reconnects=${g.reconnects} ws=${g.wsConnected ? "up" : "down"}`,
  );
  lines.push("");
  lines.push("--- raw ---");
  lines.push(JSON.stringify(s));
  return lines.join("\n");
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 10 }}>
      <div
        style={{
          color: "hsl(var(--muted-foreground))",
          fontSize: 9,
          textTransform: "uppercase",
          letterSpacing: 0.4,
          marginBottom: 3,
        }}
      >
        {title}
      </div>
      {children}
    </div>
  );
}

function Row({ k, v, warn }: { k: string; v: string; warn?: boolean }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "1px 0" }}>
      <span style={{ color: "hsl(var(--muted-foreground))", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {k}
      </span>
      <span style={{ color: warn ? "hsl(var(--crit))" : "hsl(var(--foreground))", flexShrink: 0, marginLeft: 8 }}>{v}</span>
    </div>
  );
}

function drawSparkline(canvas: HTMLCanvasElement | null) {
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);
  const data = metrics.bytesSpark;
  let max = 1;
  for (let i = 0; i < data.length; i++) if (data[i] > max) max = data[i];
  ctx.strokeStyle = "hsl(var(--cool))";
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let i = 0; i < data.length; i++) {
    const x = (i / (data.length - 1)) * w;
    const y = h - (data[i] / max) * (h - 2) - 1;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();
}
