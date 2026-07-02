// The `alarmHistory` widget — queries the alarm service's SQLite history via the
// `searchHistory` action (server-side filters: name / severity / event / from /
// to / limit) and renders the transition log. `clearHistory` purges. Resolves the
// singleton service globally (same as the live console). A config of DataTable.

import { useCallback, useEffect, useMemo, useState } from "react";
import { Search, Trash2 } from "lucide-react";
import { fmtDateTime } from "../lib/format";
import { registerWidget, type WidgetProps } from "./registry";
import { DataTable, type Column } from "./DataTable";
import { resolveAlarmService } from "./alarmService";

interface HEvent {
  name?: string;
  severity?: number | string;
  event?: string; // raised | acknowledged | rtn | cleared
  message?: string;
  at?: number;
  timestamp?: number;
  time?: number;
}

const SEV = ["low", "medium", "high"];
const SEV_COLOR: Record<string, string> = { low: "hsl(var(--cool))", medium: "hsl(var(--amber))", high: "hsl(var(--crit))" };
const sevLabel = (s: number | string | undefined) => (typeof s === "number" ? SEV[s] ?? String(s) : s ?? "");
const EVENT_COLOR: Record<string, string> = { raised: "hsl(var(--crit))", acknowledged: "hsl(var(--cool))", rtn: "hsl(var(--green))", cleared: "hsl(var(--muted-foreground))" };
const timeOf = (e: HEvent) => e.at ?? e.timestamp ?? e.time ?? 0;
const toEpoch = (s: string) => (s ? Math.floor(new Date(s).getTime() / 1000) : 0);

function parseEvents(raw: unknown): HEvent[] {
  if (typeof raw !== "string" || !raw) return [];
  try {
    const a = JSON.parse(raw);
    return Array.isArray(a) ? (a as HEvent[]) : [];
  } catch {
    return [];
  }
}

function AlarmHistoryPanel({ ctx }: WidgetProps) {
  const [serviceUid, setServiceUid] = useState<number | null>(null);
  const [state, setState] = useState<"loading" | "ok" | "missing">("loading");
  const [events, setEvents] = useState<HEvent[]>([]);
  const [f, setF] = useState({ name: "", severity: -1, event: "", from: "", to: "", limit: 200 });

  useEffect(() => {
    let alive = true;
    resolveAlarmService()
      .then((svc) => {
        if (!alive) return;
        if (svc) { setServiceUid(svc.uid); setState("ok"); }
        else setState("missing");
      })
      .catch(() => alive && setState("missing"));
    return () => { alive = false; };
  }, []);

  const search = useCallback(async () => {
    if (serviceUid == null || !ctx.callAction) return;
    const r = await ctx.callAction(serviceUid, "searchHistory", {
      name: f.name,
      severity: f.severity,
      event: f.event,
      from: toEpoch(f.from),
      to: toEpoch(f.to),
      limit: f.limit,
    });
    setEvents(parseEvents(r.results));
  }, [serviceUid, ctx, f]);

  // Auto-run once the service resolves.
  useEffect(() => { if (serviceUid != null) void search(); /* eslint-disable-next-line */ }, [serviceUid]);

  const clearHistory = async () => {
    if (serviceUid == null || !ctx.callAction) return;
    if (!window.confirm("Clear ALL alarm history?")) return;
    await ctx.callAction(serviceUid, "clearHistory", { purgeBefore: 0 });
    void search();
  };

  const columns: Column<HEvent>[] = useMemo(() => [
    { key: "time", header: "Time", sortValue: (e) => timeOf(e), render: (e) => <span style={{ color: "hsl(var(--muted-foreground))", whiteSpace: "nowrap" }}>{timeOf(e) ? fmtDateTime(timeOf(e), "datetime") : "—"}</span> },
    { key: "event", header: "Event", render: (e) => <span style={{ color: EVENT_COLOR[e.event ?? ""] ?? "hsl(var(--foreground))", fontWeight: 600 }}>{e.event}</span> },
    { key: "severity", header: "Severity", sortValue: (e) => (typeof e.severity === "number" ? e.severity : -1), render: (e) => <span style={{ color: SEV_COLOR[sevLabel(e.severity)] ?? "hsl(var(--muted-foreground))" }}>{sevLabel(e.severity)}</span> },
    { key: "name", header: "Source", render: (e) => <span style={{ color: "hsl(var(--foreground))" }}>{e.name}</span> },
    { key: "message", header: "Message", render: (e) => <span title={e.message} style={{ display: "block", maxWidth: 320, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{e.message}</span> },
  ], []);

  if (state === "loading") return <div style={{ padding: 16, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>loading alarm service…</div>;
  if (state === "missing") return <div style={{ padding: 16, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>Alarm service not found.</div>;

  return (
    <DataTable<HEvent>
      rows={events}
      getId={(e) => `${e.name ?? ""}|${e.event ?? ""}|${timeOf(e)}`}
      columns={columns}
      searchAccessor={(e) => `${e.name ?? ""} ${e.message ?? ""} ${e.event ?? ""}`}
      defaultSort={{ key: "time", dir: -1 }}
      emptyText="no history"
      toolbarRight={
        <>
          <button onClick={() => void search()} disabled={serviceUid == null} style={tb}><Search size={13} /> Search</button>
          <button onClick={clearHistory} disabled={serviceUid == null} style={{ ...tb, color: "hsl(var(--crit))" }}><Trash2 size={13} /> Clear history</button>
        </>
      }
      banner={
        <div style={{ display: "flex", alignItems: "center", gap: 6, padding: "6px 8px", borderBottom: "1px solid hsl(var(--border))", background: "hsl(var(--background))", flexWrap: "wrap" }}>
          <input value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} placeholder="name" style={{ ...inp, width: 120 }} />
          <select value={f.severity} onChange={(e) => setF({ ...f, severity: Number(e.target.value) })} style={inp}>
            <option value={-1}>severity: all</option>
            {SEV.map((s, i) => <option key={s} value={i}>{s}</option>)}
          </select>
          <select value={f.event} onChange={(e) => setF({ ...f, event: e.target.value })} style={inp}>
            <option value="">event: all</option>
            {Object.keys(EVENT_COLOR).map((ev) => <option key={ev} value={ev}>{ev}</option>)}
          </select>
          <label style={lbl}>from <input type="datetime-local" value={f.from} onChange={(e) => setF({ ...f, from: e.target.value })} style={inp} /></label>
          <label style={lbl}>to <input type="datetime-local" value={f.to} onChange={(e) => setF({ ...f, to: e.target.value })} style={inp} /></label>
          <label style={lbl}>limit <input type="number" value={f.limit} onChange={(e) => setF({ ...f, limit: Number(e.target.value) })} style={{ ...inp, width: 70 }} /></label>
        </div>
      }
    />
  );
}

const tb: React.CSSProperties = { display: "flex", alignItems: "center", gap: 4, padding: "3px 9px", fontSize: 11, color: "hsl(var(--foreground))", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 4, cursor: "pointer" };
const inp: React.CSSProperties = { background: "hsl(var(--background))", color: "hsl(var(--foreground))", border: "1px solid hsl(var(--border))", borderRadius: 3, padding: "3px 6px", fontSize: 12, outline: "none" };
const lbl: React.CSSProperties = { display: "flex", alignItems: "center", gap: 4, color: "hsl(var(--muted-foreground))", fontSize: 11 };

registerWidget("alarmHistory", AlarmHistoryPanel);
