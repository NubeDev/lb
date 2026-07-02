import { useEffect, useState } from "react";
import type { FlexValue } from "../../lib/engine-types";
import { CopyButton } from "../../ui/CopyButton";
import { acInput, acBtn, acBtnPrimary, acRow } from "../menus/styles";
import { actionKind, coerceParam, defaultForType, type ActionDef, type ActionParamDef } from "./actions";

function ParamField({
  def,
  value,
  onChange,
}: {
  def: ActionParamDef;
  value: FlexValue;
  onChange: (v: FlexValue) => void;
}) {
  const kind = actionKind(def.type);
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 3, margin: "0 0 8px" }}>
      <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>
        {def.label ?? def.name}
        <span style={{ color: "hsl(var(--muted-foreground))" }}> · {def.type}</span>
      </span>
      {def.enum ? (
        <select
          value={String(value ?? "")}
          onChange={(e) => onChange(coerceParam(def.type, e.target.value))}
          style={acInput}
        >
          {def.enum.map((opt) => (
            <option key={String(opt)} value={String(opt)}>
              {String(opt)}
            </option>
          ))}
        </select>
      ) : kind === "bool" ? (
        <input
          type="checkbox"
          checked={Boolean(value)}
          onChange={(e) => onChange(e.target.checked)}
          style={{ width: 14, height: 14 }}
        />
      ) : (
        <input
          type={kind === "num" ? "number" : "text"}
          value={value === null || value === undefined ? "" : String(value)}
          onChange={(e) => onChange(coerceParam(def.type, e.target.value))}
          style={acInput}
        />
      )}
    </label>
  );
}

// Node-body "Action…" popup. Lists the actions available on the selected
// component(s), builds a params form from each action's signature, invokes them
// (via onInvoke) per target, and shows the returns.
export function ActionPicker({
  x,
  y,
  actions,
  targetUids,
  onInvoke,
  onClose,
}: {
  x: number;
  y: number;
  actions: ActionDef[];
  targetUids: number[];
  onInvoke: (
    uids: number[],
    action: string,
    params: Record<string, FlexValue>,
  ) => Promise<Array<{ returns: Record<string, FlexValue> }>>;
  onClose: () => void;
}) {
  const [selected, setSelected] = useState<ActionDef | null>(null);
  const [values, setValues] = useState<Record<string, FlexValue>>({});
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Record<string, FlexValue> | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const dismiss = (e: MouseEvent) => {
      const el = e.target as Element | null;
      if (el && el.closest("[data-ce-node-menu]")) return;
      onClose();
    };
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    // Capture phase + pointerdown: React Flow's pane (d3-zoom) calls
    // stopImmediatePropagation on pointer/mouse down, so a bubble-phase
    // document listener never sees outside clicks. Capture fires first.
    document.addEventListener("pointerdown", dismiss, true);
    document.addEventListener("contextmenu", dismiss, true);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("pointerdown", dismiss, true);
      document.removeEventListener("contextmenu", dismiss, true);
      document.removeEventListener("keydown", onEsc);
    };
  }, [onClose]);

  const run = async (a: ActionDef, params: Record<string, FlexValue>) => {
    setBusy(true);
    setError(null);
    try {
      const res = await onInvoke(targetUids, a.name, params);
      setResult(res[0]?.returns ?? {});
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  // Click an action: invoke immediately if it has no params, else open its form
  // seeded with each param's default.
  const choose = (a: ActionDef) => {
    setError(null);
    setResult(null);
    if (!a.params || a.params.length === 0) {
      void run(a, {});
      return;
    }
    const init: Record<string, FlexValue> = {};
    for (const p of a.params) init[p.name] = p.default ?? defaultForType(p.type);
    setValues(init);
    setSelected(a);
  };

  const PICKER_W = 280;
  const left = Math.min(x, window.innerWidth - PICKER_W - 8);
  const top = Math.min(y, window.innerHeight - 360);
  const count = targetUids.length;

  return (
    <div
      data-ce-node-menu
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        left,
        top,
        zIndex: 101,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        width: PICKER_W,
        maxHeight: 360,
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
        fontSize: 12,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div
        style={{
          padding: "6px 8px",
          borderBottom: "1px solid hsl(var(--border))",
          display: "flex",
          alignItems: "center",
          gap: 6,
        }}
      >
        {selected && !result && (
          <button
            onClick={() => {
              setSelected(null);
              setError(null);
            }}
            title="Back"
            style={{
              background: "transparent",
              border: "none",
              color: "hsl(var(--cool))",
              cursor: "pointer",
              fontSize: 14,
              padding: 0,
            }}
          >
            ‹
          </button>
        )}
        <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, flex: 1 }}>
          {result
            ? "Result"
            : selected
              ? selected.label ?? selected.name
              : `Action on ${count === 1 ? "1 component" : `${count} components`}`}
        </div>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 8 }}>
        {result ? (
          <div>
            {Object.keys(result).length === 0 ? (
              <div style={{ color: "hsl(var(--muted-foreground))" }}>done — no return values</div>
            ) : (
              Object.entries(result).map(([k, v]) => (
                <div key={k} style={acRow}>
                  <span style={{ color: "hsl(var(--muted-foreground))" }}>{k}</span>
                  <span style={{ display: "flex", alignItems: "center", gap: 6, minWidth: 0 }}>
                    <span style={{ color: "hsl(var(--foreground))", fontVariantNumeric: "tabular-nums", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {String(v)}
                    </span>
                    {String(v).length > 0 && <CopyButton text={String(v)} title={`Copy ${k}`} compact />}
                  </span>
                </div>
              ))
            )}
            <button onClick={onClose} style={acBtnPrimary}>
              Close
            </button>
          </div>
        ) : selected ? (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void run(selected, values);
            }}
          >
            {(selected.params ?? []).map((p) => (
              <ParamField
                key={p.name}
                def={p}
                value={values[p.name]}
                onChange={(v) => setValues((cur) => ({ ...cur, [p.name]: v }))}
              />
            ))}
            {error && <div style={{ color: "hsl(var(--crit))", margin: "6px 0" }}>{error}</div>}
            <button type="submit" disabled={busy} style={acBtnPrimary}>
              {busy ? "Running…" : `Run on ${count === 1 ? "1 component" : `${count} components`}`}
            </button>
          </form>
        ) : actions.length === 0 ? (
          <div style={{ color: "hsl(var(--muted-foreground))" }}>no actions for this component</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            {error && <div style={{ color: "hsl(var(--crit))", margin: "2px 0 6px" }}>{error}</div>}
            {actions.map((a) => (
              <button
                key={a.name}
                onClick={() => choose(a)}
                disabled={busy}
                title={a.description}
                style={acBtn}
              >
                <span>{a.label ?? a.name}</span>
                {a.params && a.params.length > 0 ? (
                  <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>…</span>
                ) : null}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
