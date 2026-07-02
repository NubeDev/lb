import { useEffect, useMemo, useState, type CSSProperties } from "react";
import { CornerDownRight } from "lucide-react";
import { ROLE_NORMAL, CATEGORY_INPUT, CATEGORY_CONFIG } from "../../lib/engine-types";
import { getNodeByUid } from "../../lib/rest";
import { useStructural } from "../../lib/store";
import {
  facetFor,
  parseFacet,
  parseAliasInput,
  rawFacet,
  serializeFacet,
  MAX_DECIMALS,
  type ComponentFacet,
  type PropFacet,
} from "../../lib/facet";

// Root component uid — the top-level container. Ports can't expose onto root.
const ROOT_UID = 0;

const detailsField: CSSProperties = {
  background: "hsl(var(--background))",
  color: "hsl(var(--foreground))",
  border: "1px solid hsl(var(--border))",
  borderRadius: 2,
  padding: "2px 5px",
  fontSize: 11,
  fontFamily: "var(--font-mono)",
  boxSizing: "border-box",
  outline: "none",
  minWidth: 0,
};

// Configure panel — author the per-prop __facet (labels, units, decimals,
// aliases, hidden) AND manage which props are exposed as ports. Read-modify-
// write: clones the current facet and overlays the edited cosmetic fields so
// fields it doesn't touch (action/min/max/order, plus exposed-port
// expose/childComponent/facetProp) survive. Cosmetic edits apply on Save/Enter;
// expose toggles are structural and apply immediately (own props expose onto the
// parent folder; a folder's existing port rows un-expose in place).
export function ConfigurePanel({
  componentUid,
  currentParentUid,
  exposeProp,
  unexposeProp,
  onSave,
  onClose,
}: {
  componentUid: number;
  currentParentUid: number;
  exposeProp: (childPropUid: number) => Promise<void> | void;
  unexposeProp: (folderUid: number, childPropUid: number) => Promise<void> | void;
  onSave: (facetString: string) => void;
  onClose: () => void;
}) {
  const comp = useStructural((s) => s.components.get(componentUid));
  const linkedProps = useStructural((s) => s.linkedProps);
  const props = useMemo(() => {
    if (!comp) return [] as { uid: number; name: string; category: number }[];
    return Object.entries(comp.properties)
      .filter(([, p]) => (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL)
      .map(([name, p]) => ({ uid: p.uid, name, category: p.category }));
  }, [comp]);
  const initial = useMemo(
    () => facetFor(componentUid, rawFacet(comp?.properties)),
    [comp, componentUid],
  );
  // Exposed-port projections this component carries (facet records with `expose`
  // keyed by a CHILD prop uid — not one of our own props). Folders show these so
  // the port can be configured here instead of via a separate right-click.
  const portRows = useMemo(() => {
    const own = new Set(props.map((p) => p.uid));
    const out: { uid: number; name: string; side: "input" | "output" }[] = [];
    for (const [uid, f] of initial) {
      if (f.expose && !own.has(uid)) {
        out.push({ uid, name: f.label ?? `port ${uid}`, side: f.expose });
      }
    }
    return out;
  }, [initial, props]);

  type Draft = {
    label: string;
    unit: string;
    decimals: string;
    hidden: boolean;
    aliases: string;
    format: string;
  };
  const empty: Draft = { label: "", unit: "", decimals: "", hidden: false, aliases: "", format: "" };
  const seed = (uid: number): Draft => {
    const f = initial.get(uid);
    return {
      label: f?.label ?? "",
      unit: f?.unit ?? "",
      decimals: f?.decimals != null ? String(f.decimals) : "",
      hidden: f?.hidden ?? false,
      aliases: f?.aliases?.map((a) => `${a.code}=${a.label}`).join(", ") ?? "",
      format: f?.format ?? "",
    };
  };
  const [draft, setDraft] = useState<Record<number, Draft>>(() => {
    const d: Record<number, Draft> = {};
    for (const p of props) d[p.uid] = seed(p.uid);
    for (const pr of portRows) d[pr.uid] = seed(pr.uid);
    return d;
  });
  const set = (uid: number, patch: Partial<Draft>) =>
    setDraft((d) => ({ ...d, [uid]: { ...(d[uid] ?? empty), ...patch } }));

  // Which of our props are already exposed on the parent folder. The parent is
  // off-canvas (one level up), so fetch its facet once; toggles update locally.
  const canExposeHere =
    comp != null && comp.parent === currentParentUid && currentParentUid !== ROOT_UID;
  const [exposedOnParent, setExposedOnParent] = useState<Set<number>>(() => new Set());
  useEffect(() => {
    if (!canExposeHere) return;
    let cancelled = false;
    void getNodeByUid(currentParentUid, { depth: 0 })
      .then((resp) => {
        if (cancelled) return;
        const pf = parseFacet(rawFacet(resp.nodes[0]?.properties) ?? "");
        const s = new Set<number>();
        for (const [uid, f] of pf) if (f.expose != null) s.add(uid);
        setExposedOnParent(s);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [canExposeHere, currentParentUid, componentUid]);

  // Resolve each exposed port's child component name + prop name (the child is
  // off-canvas — we're configuring the folder, not inside it). Fetch the folder's
  // children once so port rows can show "compName · propName" instead of a uid.
  const [portInfo, setPortInfo] = useState<Map<number, { comp: string; prop: string }>>(
    () => new Map(),
  );
  useEffect(() => {
    if (portRows.length === 0) return;
    let cancelled = false;
    void getNodeByUid(componentUid, { depth: 1, nested: true })
      .then((resp) => {
        if (cancelled) return;
        const children = resp.nodes[0]?.children ?? [];
        const byComp = new Map(children.map((c) => [c.uid, c]));
        const m = new Map<number, { comp: string; prop: string }>();
        for (const [uid, f] of initial) {
          if (!f.expose || f.childComponent == null) continue;
          const child = byComp.get(f.childComponent);
          if (!child) continue;
          const propName = Object.entries(child.properties).find(([, p]) => p.uid === uid)?.[0];
          m.set(uid, { comp: child.name, prop: propName ?? String(uid) });
        }
        setPortInfo(m);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [componentUid, initial, portRows.length]);

  const toggleExpose = (p: { uid: number; name: string; category: number }) => {
    const next = new Set(exposedOnParent);
    if (next.has(p.uid)) {
      next.delete(p.uid);
      setExposedOnParent(next);
      void unexposeProp(currentParentUid, p.uid);
    } else {
      next.add(p.uid);
      setExposedOnParent(next);
      void exposeProp(p.uid);
    }
  };

  const applyCosmetic = (f: PropFacet, d: Draft) => {
    if (d.label.trim()) f.label = d.label.trim();
    else delete f.label;
    if (d.unit.trim()) f.unit = d.unit.trim();
    else delete f.unit;
    const dec = Number(d.decimals);
    if (d.decimals.trim() !== "" && Number.isFinite(dec)) {
      f.decimals = Math.min(MAX_DECIMALS, Math.max(0, Math.trunc(dec)));
    }
    else delete f.decimals;
    if (d.hidden) f.hidden = true;
    else delete f.hidden;
    const aliases = parseAliasInput(d.aliases);
    if (aliases.length) f.aliases = aliases;
    else delete f.aliases;
    if (d.format === "datetime" || d.format === "date" || d.format === "time") f.format = d.format;
    else delete f.format;
  };

  const save = () => {
    // Clone every existing record (preserves exposed-port records + fields this
    // panel doesn't edit), then overlay the cosmetic edits for each visible row.
    const facet: ComponentFacet = new Map();
    for (const [uid, f] of initial) facet.set(uid, { ...f });
    for (const p of props) {
      const f: PropFacet = { ...(facet.get(p.uid) ?? {}) };
      applyCosmetic(f, draft[p.uid] ?? empty);
      if (Object.keys(f).length > 0) facet.set(p.uid, f);
      else facet.delete(p.uid);
    }
    for (const pr of portRows) {
      const f: PropFacet = { ...(facet.get(pr.uid) ?? {}) };
      applyCosmetic(f, draft[pr.uid] ?? empty);
      facet.set(pr.uid, f); // keeps expose/childComponent/facetProp
    }
    onSave(serializeFacet(facet));
    onClose();
  };

  // Enter anywhere in a field confirms+closes; Esc cancels. Always stop
  // propagation so canvas keyboard shortcuts don't fire while typing.
  const onFieldKey = (e: React.KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      save();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  };

  useEffect(() => {
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onEsc);
    return () => document.removeEventListener("keydown", onEsc);
  }, [onClose]);

  // The label/unit/decimals/hide/aliases editor shared by own-prop and port rows.
  const cosmeticFields = (uid: number) => {
    const d = draft[uid] ?? empty;
    return (
      <>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 64px 46px auto",
            gap: 6,
            alignItems: "center",
          }}
        >
          <input
            placeholder="label"
            value={d.label}
            onChange={(e) => set(uid, { label: e.target.value })}
            onKeyDown={onFieldKey}
            style={detailsField}
          />
          <input
            placeholder="unit"
            value={d.unit}
            onChange={(e) => set(uid, { unit: e.target.value })}
            onKeyDown={onFieldKey}
            style={detailsField}
          />
          <input
            placeholder="dec"
            value={d.decimals}
            onChange={(e) => set(uid, { decimals: e.target.value })}
            onKeyDown={onFieldKey}
            style={detailsField}
          />
          <label
            title={linkedProps.has(uid) ? "can't hide a wired prop" : undefined}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              color: linkedProps.has(uid) ? "hsl(var(--input))" : "hsl(var(--muted-foreground))",
            }}
          >
            <input
              type="checkbox"
              checked={d.hidden && !linkedProps.has(uid)}
              disabled={linkedProps.has(uid)}
              onChange={(e) => set(uid, { hidden: e.target.checked })}
            />
            hide
          </label>
        </div>
        <input
          placeholder="aliases   e.g.  0=off, 1=auto, 2=manual"
          value={d.aliases}
          onChange={(e) => set(uid, { aliases: e.target.value })}
          onKeyDown={onFieldKey}
          style={{ ...detailsField, width: "100%", marginTop: 6 }}
        />
        <select
          value={d.format}
          onChange={(e) => set(uid, { format: e.target.value })}
          title="render a numeric (epoch) value as a local date/time"
          style={{ ...detailsField, width: "100%", marginTop: 6 }}
        >
          <option value="">format: number</option>
          <option value="datetime">format: date &amp; time (local)</option>
          <option value="date">format: date (local)</option>
          <option value="time">format: time (local)</option>
        </select>
      </>
    );
  };

  return (
    <div
      onClick={onClose}
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 200,
        background: "rgba(0,0,0,0.45)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 480,
          maxHeight: "80vh",
          background: "hsl(var(--card))",
          border: "1px solid hsl(var(--border))",
          borderRadius: 6,
          boxShadow: "0 8px 28px rgba(0,0,0,0.6)",
          display: "flex",
          flexDirection: "column",
          color: "hsl(var(--foreground))",
          fontFamily: "-apple-system, system-ui, sans-serif",
          fontSize: 12,
        }}
      >
        <div
          style={{
            padding: "8px 12px",
            borderBottom: "1px solid hsl(var(--border))",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <span style={{ fontWeight: 600 }}>
            Configure — <span style={{ color: "hsl(var(--cool))" }}>{comp?.name ?? componentUid}</span>
          </span>
          <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>label · unit · decimals · aliases</span>
        </div>
        <div style={{ overflowY: "auto" }}>
          {props.length === 0 && portRows.length === 0 ? (
            <div style={{ padding: "12px", color: "hsl(var(--muted-foreground))" }}>no editable properties</div>
          ) : (
            props.map((p) => {
              const canExpose = canExposeHere && p.category !== CATEGORY_CONFIG;
              return (
                <div key={p.uid} style={{ borderBottom: "1px solid hsl(var(--secondary))", padding: "8px 12px" }}>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      marginBottom: 5,
                    }}
                  >
                    <span
                      style={{
                        color: "hsl(var(--cool))",
                        fontFamily: "var(--font-mono)",
                      }}
                    >
                      {p.name}
                    </span>
                    {canExpose && (
                      <label
                        title={`Expose this ${p.category === CATEGORY_INPUT ? "input" : "output"} as a port on the parent folder`}
                        style={{ display: "flex", alignItems: "center", gap: 4, color: "hsl(var(--muted-foreground))" }}
                      >
                        <input
                          type="checkbox"
                          checked={exposedOnParent.has(p.uid)}
                          onChange={() => toggleExpose(p)}
                        />
                        expose
                      </label>
                    )}
                  </div>
                  {cosmeticFields(p.uid)}
                </div>
              );
            })
          )}
          {portRows.length > 0 && (
            <div
              style={{
                padding: "6px 12px",
                color: "hsl(var(--muted-foreground))",
                fontSize: 10,
                textTransform: "uppercase",
                letterSpacing: 0.5,
                borderBottom: "1px solid hsl(var(--secondary))",
                background: "hsl(var(--background))",
              }}
            >
              exposed ports
            </div>
          )}
          {portRows.map((pr) => {
            const info = portInfo.get(pr.uid);
            return (
            <div key={pr.uid} style={{ borderBottom: "1px solid hsl(var(--secondary))", padding: "8px 12px" }}>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  marginBottom: 5,
                }}
              >
                <span
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 4,
                    color: "hsl(var(--cool))",
                    fontFamily: "var(--font-mono)",
                  }}
                >
                  <CornerDownRight size={12} strokeWidth={2} />
                  {info ? `${info.comp} · ${info.prop}` : pr.name}{" "}
                  <span style={{ color: "hsl(var(--muted-foreground))" }}>({pr.side})</span>
                </span>
                <label
                  title="Un-expose this port"
                  style={{ display: "flex", alignItems: "center", gap: 4, color: "hsl(var(--muted-foreground))" }}
                >
                  <input
                    type="checkbox"
                    checked
                    onChange={() => void unexposeProp(componentUid, pr.uid)}
                  />
                  exposed
                </label>
              </div>
              {cosmeticFields(pr.uid)}
            </div>
            );
          })}
        </div>
        <div
          style={{
            padding: "8px 12px",
            borderTop: "1px solid hsl(var(--border))",
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
          }}
        >
          <button
            onClick={onClose}
            style={{
              background: "transparent",
              color: "hsl(var(--muted-foreground))",
              border: "1px solid hsl(var(--border))",
              borderRadius: 3,
              padding: "4px 12px",
              cursor: "pointer",
              fontSize: 12,
            }}
          >
            Cancel
          </button>
          <button
            onClick={save}
            style={{
              background: "hsl(var(--cool) / 0.18)",
              color: "hsl(var(--cool))",
              border: "1px solid hsl(var(--cool))",
              borderRadius: 3,
              padding: "4px 14px",
              cursor: "pointer",
              fontSize: 12,
            }}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
