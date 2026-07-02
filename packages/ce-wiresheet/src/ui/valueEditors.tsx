// Inline default-editor + override popover for value cells. Ported from the
// legacy ComponentTable so the declarative collection widget gets the same edit /
// override UX. NOTE: temporary duplicate of ComponentTable's editors — unify into
// one shared module when the legacy table is retired.

import { useEffect, useRef, useState } from "react";
import { DATATYPE_BOOL, DATATYPE_NUMBER, type FlexValue } from "../lib/engine-types";
import type { PropFacet } from "../lib/facet";
import type { DecodedValue } from "../lib/wire";

const codeOf = (v: DecodedValue | undefined) =>
  v === true ? 1 : v === false ? 0 : typeof v === "number" ? v : Number(v);

export const initialStr = (v: DecodedValue | undefined, dataType: number, facet: PropFacet | undefined) =>
  facet?.aliases?.length || dataType === DATATYPE_BOOL
    ? String(codeOf(v))
    : v == null
      ? ""
      : String(v);

export const coerceValue = (raw: string, dataType: number, facet: PropFacet | undefined): FlexValue => {
  if (facet?.aliases?.length) {
    const code = Number(raw);
    return dataType === DATATYPE_BOOL ? code === 1 : code;
  }
  if (dataType === DATATYPE_BOOL) return raw === "1" || raw === "true";
  if (dataType === DATATYPE_NUMBER) return Number(raw);
  return raw;
};

const editField = {
  background: "hsl(var(--background))",
  color: "hsl(var(--foreground))",
  border: "1px solid hsl(var(--cool))",
  borderRadius: 2,
  padding: "1px 4px",
  fontSize: 12,
  fontFamily: "var(--font-mono)",
  outline: "none",
} as const;

/** Inline editor for the stored DEFAULT value (left-click). Dropdown for aliased /
 *  boolean props, else text/number. Enter / blur commits, Esc cancels. */
export function DefaultEditor({
  initial,
  dataType,
  facet,
  onCommit,
  onCancel,
}: {
  initial: DecodedValue | undefined;
  dataType: number;
  facet: PropFacet | undefined;
  onCommit: (v: FlexValue) => void;
  onCancel: () => void;
}) {
  const aliases = facet?.aliases;
  const [text, setText] = useState(initialStr(initial, dataType, facet));
  const ref = useRef<HTMLInputElement | HTMLSelectElement>(null);
  useEffect(() => {
    ref.current?.focus();
    if (ref.current instanceof HTMLInputElement) ref.current.select();
  }, []);
  const onKey = (e: React.KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") onCommit(coerceValue(text, dataType, facet));
    else if (e.key === "Escape") onCancel();
  };
  if (aliases?.length || dataType === DATATYPE_BOOL) {
    const opts = aliases?.length
      ? aliases.map((a) => ({ v: String(a.code), label: a.label }))
      : [
          { v: "0", label: "false" },
          { v: "1", label: "true" },
        ];
    return (
      <select
        ref={ref as React.RefObject<HTMLSelectElement>}
        value={text}
        onChange={(e) => onCommit(coerceValue(e.target.value, dataType, facet))}
        onKeyDown={onKey}
        onBlur={onCancel}
        style={editField}
      >
        {opts.map((o) => (
          <option key={o.v} value={o.v}>
            {o.label}
          </option>
        ))}
      </select>
    );
  }
  return (
    <input
      ref={ref as React.RefObject<HTMLInputElement>}
      value={text}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={onKey}
      onBlur={() => onCommit(coerceValue(text, dataType, facet))}
      inputMode={dataType === DATATYPE_NUMBER ? "decimal" : "text"}
      style={{ ...editField, width: 70 }}
    />
  );
}

/** Override popover (right-click) — value + duration, set / clear. */
export function OverrideEditor({
  rect,
  cellLabel,
  initial,
  dataType,
  facet,
  overridden,
  onOverride,
  onClear,
  onClose,
}: {
  rect: DOMRect;
  cellLabel: string;
  initial: DecodedValue | undefined;
  dataType: number;
  facet: PropFacet | undefined;
  overridden: boolean;
  onOverride: (v: FlexValue, duration: number) => void;
  onClear: () => void;
  onClose: () => void;
}) {
  const aliases = facet?.aliases;
  const [text, setText] = useState(initialStr(initial, dataType, facet));
  const [duration, setDuration] = useState("60"); // matches standard override menu
  const ref = useRef<HTMLInputElement | HTMLSelectElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current?.focus();
    if (ref.current instanceof HTMLInputElement) ref.current.select();
  }, []);
  useEffect(() => {
    const dismiss = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) onClose();
    };
    document.addEventListener("pointerdown", dismiss, true);
    return () => document.removeEventListener("pointerdown", dismiss, true);
  }, [onClose]);

  const apply = () => onOverride(coerceValue(text, dataType, facet), Number(duration) || 0);
  const field = { ...editField, border: "1px solid hsl(var(--border))", borderRadius: 3, padding: "3px 6px" };
  const btn = (accent?: boolean) =>
    ({
      background: accent ? "hsl(var(--cool) / 0.18)" : "transparent",
      color: accent ? "hsl(var(--cool))" : "hsl(var(--muted-foreground))",
      border: `1px solid ${accent ? "hsl(var(--cool))" : "hsl(var(--border))"}`,
      borderRadius: 3,
      padding: "3px 8px",
      cursor: "pointer",
      fontSize: 11,
    }) as const;

  const left = Math.min(rect.left, window.innerWidth - 220);
  const top = Math.min(rect.bottom + 4, window.innerHeight - 160);

  return (
    <div
      ref={rootRef}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") onClose();
        else if (e.key === "Enter" && !(e.target instanceof HTMLSelectElement)) apply();
      }}
      style={{
        position: "fixed",
        left,
        top,
        zIndex: 200,
        width: 200,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 6,
        boxShadow: "0 8px 24px rgba(0,0,0,0.6)",
        padding: 8,
        display: "flex",
        flexDirection: "column",
        gap: 6,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
        fontSize: 12,
      }}
    >
      <div style={{ color: "hsl(var(--cool))", fontFamily: "var(--font-mono)", fontSize: 11 }}>
        Override {cellLabel}
      </div>

      {aliases?.length || dataType === DATATYPE_BOOL ? (
        <select ref={ref as React.RefObject<HTMLSelectElement>} value={text} onChange={(e) => setText(e.target.value)} style={field}>
          {(aliases?.length
            ? aliases.map((a) => ({ v: String(a.code), label: a.label }))
            : [
                { v: "0", label: "false" },
                { v: "1", label: "true" },
              ]
          ).map((o) => (
            <option key={o.v} value={o.v}>
              {o.label}
            </option>
          ))}
        </select>
      ) : (
        <input
          ref={ref as React.RefObject<HTMLInputElement>}
          value={text}
          onChange={(e) => setText(e.target.value)}
          inputMode={dataType === DATATYPE_NUMBER ? "decimal" : "text"}
          placeholder={facet?.unit}
          style={field}
        />
      )}

      <label style={{ display: "flex", alignItems: "center", gap: 6, color: "hsl(var(--muted-foreground))", fontSize: 11 }}>
        duration
        <select value={duration} onChange={(e) => setDuration(e.target.value)} style={{ ...field, flex: 1 }}>
          <option value="10">10 sec</option>
          <option value="30">30 sec</option>
          <option value="60">1 min</option>
          <option value="300">5 min</option>
          <option value="1200">20 min</option>
          <option value="3600">1 hr</option>
          <option value="7200">2 hr</option>
          <option value="86400">24 hr</option>
          <option value="0">permanent</option>
        </select>
      </label>

      <div style={{ display: "flex", gap: 6, marginTop: 2 }}>
        {overridden && (
          <button onClick={onClear} style={{ ...btn(), color: "hsl(var(--crit))" }} title="Clear override">
            clear
          </button>
        )}
        <button onClick={onClose} style={{ ...btn(), marginLeft: "auto" }}>
          cancel
        </button>
        <button onClick={apply} style={btn(true)}>
          set
        </button>
      </div>
    </div>
  );
}
