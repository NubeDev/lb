// The `cron` widget — a small helper to set a 5-field cron expression and show
// when it will next run. Next-run times are computed client-side (local time);
// Set commits via the bound action (default `setCron`, param `expr` + `hold`).
// Standard fields: min hour day-of-month month day-of-week, with * , - and /.

import { useEffect, useMemo, useState } from "react";
import { Save } from "lucide-react";
import { useValues } from "../lib/store";
import { useComponent } from "./useComponent";
import { updateNode } from "../lib/rest";
import { fmtDateTime } from "../lib/format";
import { registerWidget, type WidgetProps } from "./registry";

const PRESETS: { label: string; expr: string }[] = [
  { label: "Every minute", expr: "* * * * *" },
  { label: "Hourly", expr: "0 * * * *" },
  { label: "Daily 02:00", expr: "0 2 * * *" },
  { label: "Weekdays 08:00", expr: "0 8 * * 1-5" },
  { label: "Mondays 08:00", expr: "0 8 * * 1" },
  { label: "1st of month", expr: "0 0 1 * *" },
];

function parseField(f: string, lo: number, hi: number): Set<number> | null {
  const out = new Set<number>();
  for (const part of f.split(",")) {
    let step = 1;
    let range = part;
    const slash = part.split("/");
    if (slash.length === 2) {
      step = Number(slash[1]);
      range = slash[0];
      if (!Number.isInteger(step) || step < 1) return null;
    }
    let a = lo;
    let b = hi;
    if (range !== "*") {
      const dash = range.split("-");
      a = Number(dash[0]);
      b = dash.length === 2 ? Number(dash[1]) : a;
      if (!Number.isFinite(a) || !Number.isFinite(b) || a < lo || b > hi || a > b) return null;
    }
    for (let v = a; v <= b; v += step) out.add(v);
  }
  return out.size ? out : null;
}

/** Next `count` run times (epoch ms, local) from `fromMs`, or null if invalid. */
export function cronNextRuns(expr: string, fromMs: number, count: number): number[] | null {
  const parts = expr.trim().split(/\s+/);
  if (parts.length !== 5) return null;
  const min = parseField(parts[0], 0, 59);
  const hour = parseField(parts[1], 0, 23);
  const dom = parseField(parts[2], 1, 31);
  const mon = parseField(parts[3], 1, 12);
  const dow = parseField(parts[4], 0, 6);
  if (!min || !hour || !dom || !mon || !dow) return null;
  const domR = parts[2] !== "*";
  const dowR = parts[4] !== "*";
  const out: number[] = [];
  const d = new Date(fromMs);
  d.setSeconds(0, 0);
  d.setMinutes(d.getMinutes() + 1);
  for (let i = 0; i < 527040 && out.length < count; i++) {
    if (mon.has(d.getMonth() + 1) && hour.has(d.getHours()) && min.has(d.getMinutes())) {
      const domOk = dom.has(d.getDate());
      const dowOk = dow.has(d.getDay());
      const dayOk = domR && dowR ? domOk || dowOk : domR ? domOk : dowR ? dowOk : true;
      if (dayOk) out.push(d.getTime());
    }
    d.setMinutes(d.getMinutes() + 1);
  }
  return out;
}

function CronPanel({ node, ctx }: WidgetProps) {
  const comp = useComponent(ctx.componentUid);
  // manifest: cron has `expr` (string input) + `hold` (uint input) + `out`.
  const propName = node.bind?.prop ?? "expr";
  const uid = comp?.properties[propName]?.uid;
  const live = useValues((s) => (uid != null ? s.values.get(uid) : undefined));
  const liveExpr = typeof live === "string" ? live : "";
  const holdUid = comp?.properties["hold"]?.uid;
  const liveHold = useValues((s) => (holdUid != null ? s.values.get(holdUid) : undefined));

  const [expr, setExpr] = useState("0 2 * * *");
  const [hold, setHold] = useState(60);
  const [dirty, setDirty] = useState(false);
  const setE = (e: string) => { setExpr(e); setDirty(true); };
  // Seed from the live values until the user starts editing.
  useEffect(() => {
    if (dirty) return;
    if (liveExpr) setExpr(liveExpr);
    if (typeof liveHold === "number") setHold(liveHold);
  }, [liveExpr, liveHold, dirty]);

  const runs = useMemo(() => cronNextRuns(expr, Date.now(), 5), [expr]);
  const valid = runs != null;
  const canCall = ctx.componentUid != null && !!ctx.callAction;

  const save = () => {
    if (!canCall || !valid) return;
    // manifest: setCron(expr) — expr only. `hold` is a plain input property, so
    // write it directly (PATCH) rather than passing it to the action.
    void ctx.callAction!(ctx.componentUid!, node.action?.name ?? "setCron", { expr });
    if (typeof liveHold !== "number" || hold !== liveHold) {
      void updateNode(ctx.componentUid!, { properties: { hold: { value: hold } } });
    }
    setDirty(false);
  };

  if (!comp) return null;

  return (
    <div style={{ padding: 12, display: "flex", flexDirection: "column", gap: 12, fontSize: 12, color: "hsl(var(--foreground))", maxWidth: 420 }}>
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <input
            value={expr}
            onChange={(e) => setE(e.target.value)}
            placeholder="min hour dom mon dow"
            spellCheck={false}
            style={{ ...inp, flex: 1, fontFamily: "var(--font-mono)", borderColor: valid ? "hsl(var(--border))" : "hsl(var(--crit))" }}
          />
          <button onClick={save} disabled={!canCall || !valid || !dirty} style={{ ...btn, ...btnPrimary, opacity: !canCall || !valid || !dirty ? 0.4 : 1 }}>
            <Save size={13} /> Set
          </button>
        </div>
        <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, marginTop: 3 }}>minute · hour · day-of-month · month · day-of-week</div>
      </div>

      <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
        {PRESETS.map((p) => (
          <button key={p.expr} onClick={() => setE(p.expr)} title={p.expr} style={chip(expr === p.expr)}>{p.label}</button>
        ))}
      </div>

      <label style={{ display: "flex", alignItems: "center", gap: 6, color: "hsl(var(--muted-foreground))" }}>
        hold
        <input type="number" min={0} value={hold} onChange={(e) => { setHold(Number(e.target.value)); setDirty(true); }} style={{ ...inp, width: 80 }} />
        seconds the output stays on after each run
      </label>

      <div>
        <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 11, marginBottom: 4 }}>Next runs (local)</div>
        {!valid ? (
          <div style={{ color: "hsl(var(--crit))", fontSize: 12 }}>invalid cron expression</div>
        ) : runs.length === 0 ? (
          <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 12 }}>no run in the next ~year</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            {runs.map((t, i) => (
              <div key={t} style={{ color: i === 0 ? "hsl(var(--cool))" : "hsl(var(--foreground))", fontVariantNumeric: "tabular-nums" }}>
                {fmtDateTime(Math.round(t / 1000), "datetime")}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

const inp: React.CSSProperties = { background: "hsl(var(--background))", color: "hsl(var(--foreground))", border: "1px solid hsl(var(--border))", borderRadius: 3, padding: "4px 7px", fontSize: 12, outline: "none" };
const btn: React.CSSProperties = { display: "flex", alignItems: "center", gap: 5, padding: "5px 12px", fontSize: 12, color: "hsl(var(--foreground))", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 4, cursor: "pointer" };
const btnPrimary: React.CSSProperties = { background: "hsl(var(--cool) / 0.18)", borderColor: "hsl(var(--cool))", color: "hsl(var(--cool))" };
const chip = (on: boolean): React.CSSProperties => ({ padding: "3px 10px", fontSize: 11, borderRadius: 3, border: `1px solid ${on ? "hsl(var(--cool))" : "hsl(var(--border))"}`, cursor: "pointer", color: on ? "hsl(var(--cool))" : "hsl(var(--muted-foreground))", background: on ? "hsl(var(--cool) / 0.18)" : "transparent" });

registerWidget("cron", CronPanel);
