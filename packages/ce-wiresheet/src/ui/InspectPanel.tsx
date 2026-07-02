// ui/InspectPanel.tsx — a full, read-only overview of one component, shown in the
// drawer alongside the tree/table views. Registered as the "inspect" widget; the
// root extension's "Inspect" UI (root-ext-stub) is a `follow` panel bound to the
// selected component, opened from the canvas right-click → "Inspect".
//
// Shows: identity (name/type/uid/path/parent), decoded status, metadata, every
// property grouped by category (with live value, uid, role, status, and facet
// presentation), the raw __facets string, and the edges touching the component.

import { useEffect, useMemo, useState } from "react";
import { Locate } from "lucide-react";
import { registerWidget, type WidgetProps } from "./registry";
import { useStructural, useValues, useStatusFlags } from "../lib/store";
import { getNodeByUid, getEdges } from "../lib/rest";
import { rawFacet, parseFacet, type PropFacet } from "../lib/facet";
import { fmtValueFacet, inferDataType } from "../lib/format";
import type { DecodedValue } from "../lib/wire";
import { CopyUid, CopyText, stripRoot } from "../components/FunctionBlock";
import {
  CATEGORY_INPUT,
  CATEGORY_OUTPUT,
  ROLE_NORMAL,
  STATUS_FAULT,
  STATUS_OVERRIDDEN,
  type Component,
  type Edge,
  type Property,
} from "../lib/engine-types";

const CATEGORY_LABEL: Record<number, string> = { 0: "input", 1: "output", 2: "config" };

function decodeStatus(flags: number): string {
  if (!flags) return "ok";
  const out: string[] = [];
  if (flags & STATUS_FAULT) out.push("fault");
  if (flags & STATUS_OVERRIDDEN) out.push("overridden");
  // surface any other set bits numerically so nothing is silently hidden
  const known = STATUS_FAULT | STATUS_OVERRIDDEN;
  const rest = flags & ~known;
  if (rest) out.push(`0x${rest.toString(16)}`);
  return out.join(" · ") || `0x${flags.toString(16)}`;
}

function fmt(v: unknown): string {
  if (v == null) return "—";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

const mono: React.CSSProperties = { fontFamily: "var(--font-mono)", fontVariantNumeric: "tabular-nums" };

function InspectPanel({ ctx }: WidgetProps) {
  // Follow the host selection when bound; otherwise honour the explicit focus uid
  // set by the canvas right-click → "Inspect" (which doesn't change canvas selection).
  const uid = ctx.componentUid ?? ctx.focusUid;
  const inStore = useStructural((s) => (uid != null ? s.components.get(uid) : undefined));
  const [fetched, setFetched] = useState<Component | undefined>(undefined);
  const comp = inStore ?? fetched;

  // Fetch the component if it isn't in the store (e.g. selected in another folder).
  useEffect(() => {
    if (uid == null || inStore) { setFetched(undefined); return; }
    let live = true;
    getNodeByUid(uid, { depth: 0 }).then((r) => { if (live) setFetched(r.nodes[0]); }).catch(() => {});
    return () => { live = false; };
  }, [uid, inStore]);

  // Edges touching this component (both directions).
  const [edges, setEdges] = useState<Edge[]>([]);
  useEffect(() => {
    if (uid == null) { setEdges([]); return; }
    let live = true;
    getEdges(uid).then((es) => { if (live) setEdges(es); }).catch(() => { if (live) setEdges([]); });
    return () => { live = false; };
  }, [uid]);

  // Real, user-facing properties only — hide the system props (__status, __facets).
  // The user doesn't think of those as properties and their raw values are opaque;
  // their info surfaces elsewhere (status in the header + per-row status column,
  // __facets as the readable "Presentation" section). Order: outputs, inputs, config.
  const rows = useMemo(() => {
    if (!comp) return [];
    const list = Object.entries(comp.properties ?? {})
      .map(([name, p]) => ({ name, p: p as Property }))
      .filter(({ p }) => (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL);
    const rank = (c: number) => (c === CATEGORY_OUTPUT ? 0 : c === CATEGORY_INPUT ? 1 : 2);
    return list.sort((a, b) => rank(a.p.category) - rank(b.p.category) || a.p.uid - b.p.uid);
  }, [comp]);

  // uid → name across ALL props (incl. system) so facet entries, which are keyed by
  // prop uid, can be labelled by their property name in the Presentation section.
  const nameByUid = useMemo(() => {
    const m = new Map<number, string>();
    if (comp) for (const [name, p] of Object.entries(comp.properties ?? {})) m.set((p as Property).uid, name);
    return m;
  }, [comp]);

  const propUids = useMemo(() => rows.map((r) => r.p.uid), [rows]);
  // Stream live values for every property while the panel is mounted.
  useEffect(() => {
    if (!ctx.subscribeProps || propUids.length === 0) return;
    return ctx.subscribeProps(propUids);
  }, [ctx, propUids]);

  const facet = useMemo(() => (comp ? parseFacet(rawFacet(comp.properties) ?? "") : new Map<number, PropFacet>()), [comp]);

  if (uid == null) {
    return <Empty text="Select a component to inspect." />;
  }
  if (!comp) {
    return <Empty text="Loading…" />;
  }

  return (
    <div style={{ height: "100%", overflow: "auto", padding: "12px 14px", color: "hsl(var(--foreground))", fontSize: 12 }}>
      {/* Identity */}
      <div style={{ marginBottom: 14 }}>
        <div style={{ fontSize: 16, fontWeight: 600 }}>{comp.name || comp.type}</div>
        <div style={{ ...mono, fontSize: 11, color: "hsl(var(--muted-foreground))", marginTop: 2 }}>{comp.type}</div>
        <div style={{ ...mono, fontSize: 10.5, color: "hsl(var(--muted-foreground))", marginTop: 6, display: "flex", gap: 12, flexWrap: "wrap" }}>
          <CopyUid label="uid" value={comp.uid} />
          <CopyText display={stripRoot(comp.path)} value={stripRoot(comp.path)} title="click to copy path" />
          <span>parent #{comp.parent}</span>
          {comp.childrenCount ? <span>{comp.childrenCount} children</span> : null}
          <span>status: {decodeStatus((comp as { statusFlags?: number }).statusFlags ?? 0)}</span>
        </div>
      </div>

      <Section title={`Properties (${rows.length})`}>
        <table style={{ width: "100%", borderCollapse: "collapse", ...mono, fontSize: 11 }}>
          <thead>
            <tr style={{ color: "hsl(var(--muted-foreground))", textAlign: "left" }}>
              <Th>name</Th><Th>uid</Th><Th>cat</Th><Th>value</Th><Th>status</Th>
            </tr>
          </thead>
          <tbody>
            {rows.map(({ name, p }) => (
              <PropRow key={p.uid} name={name} p={p} facet={facet.get(p.uid)} />
            ))}
          </tbody>
        </table>
      </Section>

      {edges.length > 0 && (
        <Section title={`Edges (${edges.length})`}>
          <table style={{ width: "100%", borderCollapse: "collapse", ...mono, fontSize: 11 }}>
            <thead>
              <tr style={{ color: "hsl(var(--muted-foreground))", textAlign: "left" }}>
                <Th>uid</Th><Th>source</Th><Th>target</Th><Th>flags</Th>
              </tr>
            </thead>
            <tbody>
              {edges.map((e) => (
                <tr key={e.uid} style={{ borderTop: "1px solid hsl(var(--border))" }}>
                  <Td>{e.uid}</Td>
                  <Td><Locatable uid={e.sourceUid} label={`${e.sourceUid}.${e.sourceProperty}`} onLocate={ctx.locate} /></Td>
                  <Td><Locatable uid={e.targetUid} label={`${e.targetUid}.${e.targetProperty}`} onLocate={ctx.locate} /></Td>
                  <Td>{[e.loopBack ? "loopBack" : null, e.hidden ? "hidden" : null].filter(Boolean).join(" ") || "—"}</Td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      )}

      <Section title="Metadata">
        <KV k="position" v={`${comp.metadata?.position?.x ?? 0}, ${comp.metadata?.position?.y ?? 0}`} />
        <KV k="size" v={comp.metadata?.size ? `${comp.metadata.size.w ?? "?"} × ${comp.metadata.size.h ?? "?"}` : "—"} />
        <KV k="typeId" v={fmt((comp as { typeId?: number }).typeId)} />
      </Section>

      {facet.size > 0 && (
        <Section title="Presentation">
          {[...facet.entries()].map(([uid, f]) => {
            const effects = describeFacet(f);
            if (effects.length === 0) return null;
            return (
              <div key={uid} style={{ display: "flex", gap: 8, padding: "3px 0", ...mono, fontSize: 11, borderTop: "1px solid hsl(var(--border))" }}>
                <span style={{ width: 96, flexShrink: 0 }}>{nameByUid.get(uid) ?? `#${uid}`}</span>
                <span style={{ color: "hsl(var(--muted-foreground))" }}>{effects.join(" · ")}</span>
              </div>
            );
          })}
        </Section>
      )}
    </div>
  );
}

// Per-property row: live value from the stream (falls back to the stored value),
// formatted the SAME way the component's node rows are — alias label, then the
// facet's decimals/format/unit (default 2dp for floats). So 0.36262… shows as 0.36.
function PropRow({ name, p, facet }: { name: string; p: Property; facet?: PropFacet }) {
  const live = useValues((s) => s.values.get(p.uid));
  const liveFlags = useStatusFlags((s) => s.flags.get(p.uid));
  const flags = liveFlags ?? p.statusFlags ?? 0;
  const raw = live !== undefined ? live : p.value;
  const value =
    raw == null ? "—" :
    typeof raw === "object" ? JSON.stringify(raw) :
    fmtValueFacet(raw as DecodedValue, inferDataType(raw), facet);
  return (
    <tr style={{ borderTop: "1px solid hsl(var(--border))" }}>
      <Td>{name}</Td>
      <Td>{p.uid}</Td>
      <Td>{CATEGORY_LABEL[p.category] ?? p.category}</Td>
      <Td title={value} style={{ maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{value}</Td>
      <Td style={{ color: flags ? "hsl(var(--amber))" : "hsl(var(--muted-foreground))" }}>{decodeStatus(flags)}</Td>
    </tr>
  );
}

// Plain-language list of what a facet does to a property (label, unit, formatting,
// exposure, and the actual value→label aliases) — instead of the raw control-char
// __facets string, which means nothing to a user.
function describeFacet(f: PropFacet): string[] {
  const out: string[] = [];
  if (f.label) out.push(`labeled “${f.label}”`);
  if (f.unit) out.push(`unit ${f.unit}`);
  if (f.decimals != null) out.push(`${f.decimals} decimal${f.decimals === 1 ? "" : "s"}`);
  if (f.format) out.push(`shown as ${f.format}`);
  if (f.min != null || f.max != null) out.push(`range ${f.min ?? "−∞"}…${f.max ?? "∞"}`);
  if (f.hidden) out.push("hidden");
  if (f.order != null) out.push(`order ${f.order}`);
  if (f.expose) out.push(`exposed as ${f.expose}${f.chain ? " (chained)" : ""}`);
  if (f.aliases?.length) out.push(`values: ${f.aliases.map((a) => `${a.code} → ${a.label}`).join(", ")}`);
  return out;
}

// An edge endpoint that jumps the canvas to the component when clicked (drill in +
// centre + select). Plain text when no locate handler is wired.
function Locatable({ uid, label, onLocate }: { uid: number; label: string; onLocate?: (uid: number) => void }) {
  if (!onLocate) return <>{label}</>;
  return (
    <button
      type="button"
      onClick={() => onLocate(uid)}
      title="Locate on canvas"
      style={{ display: "inline-flex", alignItems: "center", gap: 4, background: "transparent", border: "none", color: "hsl(var(--cool))", cursor: "pointer", padding: 0, font: "inherit" }}
    >
      {label}
      <Locate size={11} />
    </button>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 16 }}>
      <div style={{ fontSize: 10, textTransform: "uppercase", letterSpacing: 0.5, color: "hsl(var(--muted-foreground))", marginBottom: 6, fontWeight: 600 }}>{title}</div>
      {children}
    </div>
  );
}
function KV({ k, v }: { k: string; v: string }) {
  return (
    <div style={{ display: "flex", gap: 8, padding: "2px 0", ...mono, fontSize: 11 }}>
      <span style={{ width: 80, color: "hsl(var(--muted-foreground))", flexShrink: 0 }}>{k}</span>
      <span>{v}</span>
    </div>
  );
}
const Th = ({ children }: { children: React.ReactNode }) => <th style={{ padding: "2px 8px 4px 0", fontWeight: 500 }}>{children}</th>;
const Td = ({ children, style, title }: { children: React.ReactNode; style?: React.CSSProperties; title?: string }) => (
  <td title={title} style={{ padding: "3px 8px 3px 0", verticalAlign: "top", ...style }}>{children}</td>
);
function Empty({ text }: { text: string }) {
  return <div style={{ padding: 16, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>{text}</div>;
}

registerWidget("inspect", InspectPanel);
