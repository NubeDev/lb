// Self-contained inline-SVG chart primitives for the nube catalog — no external chart lib (the package
// has react as its only peer dep; recharts/d3 are deliberately NOT added). Every renderer is a PURE
// function of already-resolved props and is DEFENSIVE: a bad/empty value renders a `gu-placeholder`
// "no data" node, never throws. Colours come from the scoped `--gu-*` tokens (currentColor / var()),
// never from a user-controlled prop, so there is no class/style injection (promotion-checklist item 5).

import type { ReactNode } from "react";

/** Coerce to a finite number, or fall back. Never throws. */
function num(v: unknown, fallback = NaN): number {
  const n = typeof v === "number" ? v : Number(v);
  return Number.isFinite(n) ? n : fallback;
}

/** Read a numeric field off a row object (or the row itself if it is a bare number). */
function pick(row: unknown, field: string | undefined): number {
  if (field && row && typeof row === "object") return num((row as Record<string, unknown>)[field]);
  return num(row);
}

/** A shared "no data" placeholder so every chart degrades identically. */
function noData(label = "no data"): ReactNode {
  return <div className="gu-placeholder">{label}</div>;
}

/** The fixed drawStyle whitelist for timeseries — an enum prop NEVER reaches className/attributes raw. */
type DrawStyle = "line" | "bars" | "points";
function drawStyleOf(v: unknown): DrawStyle {
  return v === "bars" || v === "points" ? v : "line";
}

// ── timeseries ──────────────────────────────────────────────────────────────────────────────────
export function Timeseries(props: Record<string, unknown>): ReactNode {
  const rows = Array.isArray(props.rows) ? props.rows : [];
  if (rows.length === 0) return noData();
  const yField = typeof props.yField === "string" ? props.yField : undefined;
  const xField = typeof props.xField === "string" ? props.xField : undefined;
  const style = drawStyleOf(props.drawStyle);

  const ys = rows.map((r) => pick(r, yField));
  const xsRaw = xField ? rows.map((r) => pick(r, xField)) : rows.map((_, i) => i);
  const finiteY = ys.filter((n) => Number.isFinite(n));
  if (finiteY.length === 0) return noData();

  const W = 300;
  const H = 100;
  const pad = 4;
  const minY = Math.min(...finiteY);
  const maxY = Math.max(...finiteY);
  const spanY = maxY - minY || 1;
  const xs = xsRaw.map((x) => (Number.isFinite(x) ? x : 0));
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const spanX = maxX - minX || 1;

  const sx = (x: number) => pad + ((x - minX) / spanX) * (W - 2 * pad);
  const sy = (y: number) => H - pad - ((y - minY) / spanY) * (H - 2 * pad);

  const pts = ys.map((y, i) => ({ x: sx(xs[i]), y: Number.isFinite(y) ? sy(y) : sy(minY) }));

  return (
    <div className="gu-chart">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label="timeseries">
        {style === "line" && (
          <polyline
            points={pts.map((p) => `${p.x},${p.y}`).join(" ")}
            fill="none"
            stroke="var(--gu-accent)"
            strokeWidth={1.5}
          />
        )}
        {style === "points" &&
          pts.map((p, i) => <circle key={i} cx={p.x} cy={p.y} r={2} fill="var(--gu-accent)" />)}
        {style === "bars" &&
          pts.map((p, i) => (
            <rect
              key={i}
              x={p.x - 1.5}
              y={p.y}
              width={3}
              height={Math.max(0, H - pad - p.y)}
              fill="var(--gu-accent)"
            />
          ))}
      </svg>
    </div>
  );
}

// ── barchart ────────────────────────────────────────────────────────────────────────────────────
function nameValueRows(data: unknown): { name: string; value: number }[] {
  if (!Array.isArray(data)) return [];
  return data
    .map((d) => {
      if (!d || typeof d !== "object") return null;
      const o = d as Record<string, unknown>;
      return { name: String(o.name ?? ""), value: num(o.value) };
    })
    .filter((r): r is { name: string; value: number } => r !== null && Number.isFinite(r.value));
}

export function Barchart(props: Record<string, unknown>): ReactNode {
  const rows = nameValueRows(props.data);
  if (rows.length === 0) return noData();
  const horizontal = props.horizontal === true;
  const max = Math.max(...rows.map((r) => r.value), 0) || 1;

  const W = 300;
  const H = Math.max(60, rows.length * (horizontal ? 22 : 0) || 100);
  const gap = 4;

  if (horizontal) {
    const rowH = (H - gap) / rows.length;
    return (
      <div className="gu-chart">
        <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label="barchart">
          {rows.map((r, i) => {
            const w = (r.value / max) * (W - 2);
            return (
              <rect key={i} x={1} y={i * rowH + gap / 2} width={Math.max(0, w)} height={rowH - gap} fill="var(--gu-accent)" />
            );
          })}
        </svg>
      </div>
    );
  }

  const barW = (W - gap) / rows.length;
  return (
    <div className="gu-chart">
      <svg viewBox={`0 0 ${W} 100`} role="img" aria-label="barchart">
        {rows.map((r, i) => {
          const h = (r.value / max) * (100 - 2);
          return (
            <rect key={i} x={i * barW + gap / 2} y={100 - h} width={barW - gap} height={Math.max(0, h)} fill="var(--gu-accent)" />
          );
        })}
      </svg>
    </div>
  );
}

// ── piechart ────────────────────────────────────────────────────────────────────────────────────
type PieType = "pie" | "donut";
function pieTypeOf(v: unknown): PieType {
  return v === "donut" ? "donut" : "pie";
}

// A small fixed palette drawn from the scoped tokens — indexed by slice, never user-supplied.
const SLICE_FILLS = ["var(--gu-accent)", "var(--gu-warn)", "var(--gu-bad)", "var(--gu-muted)"];

export function Piechart(props: Record<string, unknown>): ReactNode {
  const rows = nameValueRows(props.data).filter((r) => r.value >= 0);
  const total = rows.reduce((s, r) => s + r.value, 0);
  if (rows.length === 0 || total <= 0) return noData();
  const type = pieTypeOf(props.pieType);

  const size = 120;
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - 2;
  let acc = 0;

  const arc = (frac: number, start: number) => {
    const a0 = start * 2 * Math.PI - Math.PI / 2;
    const a1 = (start + frac) * 2 * Math.PI - Math.PI / 2;
    const large = frac > 0.5 ? 1 : 0;
    const x0 = cx + r * Math.cos(a0);
    const y0 = cy + r * Math.sin(a0);
    const x1 = cx + r * Math.cos(a1);
    const y1 = cy + r * Math.sin(a1);
    return `M ${cx} ${cy} L ${x0} ${y0} A ${r} ${r} 0 ${large} 1 ${x1} ${y1} Z`;
  };

  return (
    <div className="gu-chart">
      <svg viewBox={`0 0 ${size} ${size}`} role="img" aria-label="piechart">
        {rows.map((row, i) => {
          const frac = row.value / total;
          const d = arc(frac, acc);
          acc += frac;
          return <path key={i} d={d} fill={SLICE_FILLS[i % SLICE_FILLS.length]} />;
        })}
        {type === "donut" && <circle cx={cx} cy={cy} r={r * 0.55} fill="var(--gu-bg)" />}
      </svg>
    </div>
  );
}

// ── gauge ───────────────────────────────────────────────────────────────────────────────────────
export function Gauge(props: Record<string, unknown>): ReactNode {
  const value = num(props.value);
  if (!Number.isFinite(value)) return noData();
  const min = num(props.min, 0);
  const max = num(props.max, 100);
  const span = max - min || 1;
  const frac = Math.max(0, Math.min(1, (value - min) / span));

  // Threshold tone from a whitelisted set — thresholds is an ascending list of numbers.
  const thresholds = Array.isArray(props.thresholds)
    ? (props.thresholds.map((t) => num(t)).filter((n) => Number.isFinite(n)) as number[])
    : [];
  let tone = "var(--gu-ok)";
  if (thresholds.length >= 1 && value >= thresholds[thresholds.length - 1]) tone = "var(--gu-bad)";
  else if (thresholds.length >= 2 && value >= thresholds[0]) tone = "var(--gu-warn)";

  const size = 120;
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - 8;
  // Semicircle arc from 180° to 0°.
  const a = Math.PI * (1 - frac);
  const startX = cx - r;
  const startY = cy;
  const endX = cx + r * Math.cos(a);
  const endY = cy - r * Math.sin(a);
  const large = 0;

  return (
    <div className="gu-chart">
      <svg viewBox={`0 0 ${size} ${cy + 6}`} role="img" aria-label="gauge">
        <path d={`M ${startX} ${startY} A ${r} ${r} 0 0 1 ${cx + r} ${cy}`} fill="none" stroke="var(--gu-border)" strokeWidth={8} />
        <path d={`M ${startX} ${startY} A ${r} ${r} 0 ${large} 1 ${endX} ${endY}`} fill="none" stroke={tone} strokeWidth={8} strokeLinecap="round" />
      </svg>
    </div>
  );
}
