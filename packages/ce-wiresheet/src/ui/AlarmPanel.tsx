// The `alarms` widget — live ISA-18.2 console over the singleton alarm service.
// The service registry holds only rows still needing attention (active OR
// unacknowledged); each row carries raisedAt / ackedAt / clearedAt. Resolved
// globally (services folder) and polled. Acknowledge clears an RTN alarm; Clear
// is the operator dismiss. A config of the reusable DataTable.

import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, Trash2 } from "lucide-react";
import { getNodeByUid } from "../lib/rest";
import { fmtDateTime } from "../lib/format";
import { registerWidget, type WidgetProps } from "./registry";
import { DataTable, type Column, type Filter, type RowAction, type BulkAction } from "./DataTable";
import { resolveAlarmService } from "./alarmService";
import type { Component } from "../lib/engine-types";

interface Alarm {
  name: string;
  severity: number | string;
  message?: string;
  source?: string;
  acknowledged?: boolean;
  active?: boolean;
  raisedAt?: number;
  ackedAt?: number;
  clearedAt?: number;
}

const SEV = ["low", "medium", "high"];
const SEV_COLOR: Record<string, string> = { low: "hsl(var(--cool))", medium: "hsl(var(--amber))", high: "hsl(var(--crit))" };
const sevLabel = (s: number | string) => (typeof s === "number" ? SEV[s] ?? String(s) : s);
const sevRank = (s: number | string) => (typeof s === "number" ? s : SEV.indexOf(s));
// Active while not returned-to-normal (no clearedAt). Falls back to the `active` flag.
const isActive = (a: Alarm) => (a.clearedAt ? false : a.active ?? true);

function parseAlarms(raw: unknown): Alarm[] {
  if (typeof raw !== "string" || !raw) return [];
  let arr: Alarm[];
  try {
    const a = JSON.parse(raw);
    arr = Array.isArray(a) ? (a as Alarm[]) : [];
  } catch {
    return [];
  }
  // One row per alarm name — keep the most recent (by raisedAt).
  const byName = new Map<string, Alarm>();
  for (const a of arr) {
    const prev = byName.get(a.name);
    if (!prev || (a.raisedAt ?? 0) >= (prev.raisedAt ?? 0)) byName.set(a.name, a);
  }
  return [...byName.values()];
}

const alarmsOf = (c?: Component) => parseAlarms(c?.properties?.["alarms"]?.value);

function AlarmPanel({ ctx }: WidgetProps) {
  const [serviceUid, setServiceUid] = useState<number | null>(null);
  const [alarms, setAlarms] = useState<Alarm[]>([]);
  const [state, setState] = useState<"loading" | "ok" | "missing">("loading");

  useEffect(() => {
    let alive = true;
    resolveAlarmService()
      .then((svc) => {
        if (!alive) return;
        if (svc) { setServiceUid(svc.uid); setAlarms(alarmsOf(svc)); setState("ok"); }
        else setState("missing");
      })
      .catch(() => alive && setState("missing"));
    return () => { alive = false; };
  }, []);

  const refetch = useCallback(async () => {
    if (serviceUid == null) return;
    try {
      const r = await getNodeByUid(serviceUid, { depth: 0 });
      setAlarms(alarmsOf(r.nodes[0]));
    } catch {
      /* keep last known */
    }
  }, [serviceUid]);

  useEffect(() => {
    if (serviceUid == null) return;
    const id = setInterval(refetch, 3000);
    return () => clearInterval(id);
  }, [serviceUid, refetch]);

  const canCall = serviceUid != null && !!ctx.callAction;
  const call = (action: string, name = "") => {
    if (serviceUid == null || !ctx.callAction) return;
    void ctx.callAction(serviceUid, action, { name }).then(refetch);
  };

  const columns: Column<Alarm>[] = useMemo(() => [
    { key: "severity", header: "Severity", sortValue: (a) => sevRank(a.severity), render: (a) => <span style={{ color: SEV_COLOR[sevLabel(a.severity)] ?? "hsl(var(--muted-foreground))", fontWeight: 600 }}>{sevLabel(a.severity)}</span> },
    { key: "source", header: "Source", sortValue: (a) => a.source ?? a.name, render: (a) => <span style={{ color: "hsl(var(--foreground))" }}>{a.source ?? a.name}</span> },
    { key: "message", header: "Message", render: (a) => <span title={a.message} style={{ display: "block", maxWidth: 320, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{a.message}</span> },
    { key: "state", header: "State", render: (a) => (isActive(a) ? <span style={{ color: "hsl(var(--amber))" }}>active</span> : <span style={{ color: "hsl(var(--green))" }}>RTN</span>) },
    { key: "acknowledged", header: "Ack", render: (a) => <span style={{ color: a.acknowledged ? "hsl(var(--green))" : "hsl(var(--crit))" }}>{a.acknowledged ? "Yes" : "No"}</span> },
    { key: "raisedAt", header: "Raised", sortValue: (a) => a.raisedAt ?? 0, render: (a) => <span style={{ color: "hsl(var(--muted-foreground))", whiteSpace: "nowrap" }}>{a.raisedAt ? fmtDateTime(a.raisedAt, "datetime") : "—"}</span> },
  ], []);

  const filters: Filter<Alarm>[] = [
    { key: "severity", options: [{ label: "severity: all", value: "all" }, ...SEV.map((s) => ({ label: s, value: s }))], match: (a, v) => sevLabel(a.severity) === v },
    { key: "ack", options: [{ label: "ack: all", value: "all" }, { label: "unacked", value: "no" }, { label: "acked", value: "yes" }], match: (a, v) => (v === "yes" ? !!a.acknowledged : !a.acknowledged) },
    { key: "state", options: [{ label: "state: all", value: "all" }, { label: "active", value: "active" }, { label: "RTN", value: "rtn" }], match: (a, v) => (v === "active" ? isActive(a) : !isActive(a)) },
  ];

  const rowActions: RowAction<Alarm>[] = [
    { label: "Ack", show: (a) => !a.acknowledged, onClick: (a) => call("acknowledge", a.name) },
    { label: "Clear", danger: true, onClick: (a) => call("clear", a.name) },
  ];
  const bulkActions: BulkAction<Alarm>[] = [
    { label: "Ack selected", onClick: (rows) => rows.forEach((a) => call("acknowledge", a.name)) },
    { label: "Clear selected", danger: true, onClick: (rows) => rows.forEach((a) => call("clear", a.name)) },
  ];

  if (state === "loading") return <div style={{ padding: 16, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>loading alarm service…</div>;
  if (state === "missing") return <div style={{ padding: 16, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>Alarm service not found.</div>;

  return (
    <DataTable<Alarm>
      rows={alarms}
      getId={(a) => a.name}
      columns={columns}
      searchAccessor={(a) => `${a.source ?? ""} ${a.name} ${a.message ?? ""}`}
      filters={filters}
      selectable
      rowActions={rowActions}
      bulkActions={bulkActions}
      defaultSort={{ key: "raisedAt", dir: -1 }}
      rowTint={(a) => (!a.acknowledged && isActive(a) ? "rgba(224,112,122,0.06)" : undefined)}
      emptyText="no active alarms"
      toolbarRight={
        <>
          <button onClick={() => call("acknowledge", "")} disabled={!canCall} style={tb}><Check size={13} /> Ack all</button>
          <button onClick={() => call("clear", "")} disabled={!canCall} style={{ ...tb, color: "hsl(var(--crit))" }}><Trash2 size={13} /> Clear all</button>
        </>
      }
    />
  );
}

const tb: React.CSSProperties = { display: "flex", alignItems: "center", gap: 4, padding: "3px 9px", fontSize: 11, color: "hsl(var(--foreground))", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 4, cursor: "pointer" };

registerWidget("alarms", AlarmPanel);
