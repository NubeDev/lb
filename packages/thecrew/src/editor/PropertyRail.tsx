// The selected-shape editor (builder-ux-scope.md §tune): schema-driven from the
// symbol's propSchema (≤8 props), then bindings — a channel picker over
// ValueSource.channels() with the LIVE value shown beside each binding.
// No Apply button; every change reflects on-canvas immediately. No modals, ever.

import { Trash2 } from "lucide-react";
import { SYMBOLS } from "../canvas/ShapeNode";
import { useChannelValue, useValueSource } from "../data/use-values";
import type { SceneShape } from "../scene/scene.types";
import { useSceneStore } from "../state/scene-store";

const INPUT =
  "w-full rounded border border-[var(--tc-hairline)] bg-transparent px-2 py-1 text-sm text-slate-200 outline-none focus-visible:ring-1 focus-visible:ring-[var(--tc-accent)]";

function fmt(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "number") return Number.isInteger(v) ? String(v) : v.toFixed(1);
  if (typeof v === "boolean") return v ? "on" : "off";
  return String(v);
}

function Field({
  id,
  prop,
  schema,
  value,
}: {
  id: string;
  prop: string;
  schema: { label: string; kind: "text" | "number" | "select" | "boolean"; options?: string[] };
  value: unknown;
}) {
  const setProp = useSceneStore((s) => s.setProp);
  const commitKey = `${id}:${prop}:${String(value)}`; // remounts on external change (undo)

  return (
    <label className="block space-y-1">
      <span className="text-xs text-slate-400">{schema.label}</span>
      {schema.kind === "text" && (
        <input
          key={commitKey}
          type="text"
          defaultValue={typeof value === "string" ? value : ""}
          onBlur={(e) => e.target.value !== value && setProp(id, prop, e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          className={INPUT}
        />
      )}
      {schema.kind === "number" && (
        <input
          key={commitKey}
          type="number"
          defaultValue={typeof value === "number" ? value : 0}
          onBlur={(e) => {
            const n = Number(e.target.value);
            if (Number.isFinite(n) && n !== value) setProp(id, prop, n);
          }}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          className={`${INPUT} tabular-nums`}
        />
      )}
      {schema.kind === "select" && (
        <select
          value={typeof value === "string" ? value : ""}
          onChange={(e) => setProp(id, prop, e.target.value)}
          className={`${INPUT} bg-[var(--tc-panel)]`}
        >
          {(schema.options ?? []).map((o) => (
            <option key={o} value={o} className="bg-[var(--tc-panel-solid)]">
              {o}
            </option>
          ))}
        </select>
      )}
      {schema.kind === "boolean" && (
        <input
          type="checkbox"
          checked={value === true}
          onChange={(e) => setProp(id, prop, e.target.checked)}
          className="block h-4 w-4 accent-[var(--tc-accent)]"
        />
      )}
    </label>
  );
}

/** one binding slot: channel picker + the LIVE value beside it (tune-and-watch) */
function BindRow({ id, slot, shape }: { id: string; slot: string; shape: SceneShape }) {
  const source = useValueSource();
  const setBind = useSceneStore((s) => s.setBind);
  const channel = shape.bind?.[slot]?.channel ?? "";
  const live = useChannelValue(channel || null);

  return (
    <div className="space-y-1">
      <div className="flex items-baseline justify-between">
        <span className="text-xs text-slate-400">{slot}</span>
        <span className="text-xs tabular-nums text-[var(--tc-accent)]">{channel ? fmt(live) : ""}</span>
      </div>
      <select
        value={channel}
        onChange={(e) => setBind(id, slot, e.target.value || null)}
        className={`${INPUT} bg-[var(--tc-panel)]`}
      >
        <option value="" className="bg-[var(--tc-panel-solid)]">
          —
        </option>
        {source.channels().map((c) => (
          <option key={c} value={c} className="bg-[var(--tc-panel-solid)]">
            {c}
          </option>
        ))}
      </select>
    </div>
  );
}

export function PropertyRail() {
  const selection = useSceneStore((s) => s.selection);
  const doc = useSceneStore((s) => s.doc);
  const deleteSelection = useSceneStore((s) => s.deleteSelection);

  const body = (() => {
    if (selection.length === 0) {
      return <p className="pt-6 text-center text-xs text-slate-500">select a shape</p>;
    }
    if (selection.length > 1) {
      return (
        <div className="space-y-3 pt-2">
          <p className="text-sm text-slate-300">{selection.length} shapes selected</p>
          <button
            type="button"
            onClick={deleteSelection}
            className="flex items-center gap-1.5 rounded border border-[var(--tc-hairline)] px-2 py-1 text-xs text-slate-400 outline-none transition-colors hover:text-slate-200 focus-visible:ring-1 focus-visible:ring-[var(--tc-accent)]"
          >
            <Trash2 size={12} /> delete
          </button>
        </div>
      );
    }
    const id = selection[0]!;
    const shape = doc.shapes[id];
    if (!shape) return null;
    const def = SYMBOLS[shape.type];
    return (
      <div className="space-y-4">
        <header className="border-b border-[var(--tc-hairline)] pb-2">
          <div className="text-sm text-slate-200">{def?.label ?? shape.type}</div>
          <div className="text-xs text-slate-500">{id}</div>
        </header>
        {def &&
          Object.entries(def.propSchema).map(([prop, schema]) => (
            <Field key={`${id}:${prop}`} id={id} prop={prop} schema={schema} value={shape.props[prop]} />
          ))}
        {def && def.bindSlots.length > 0 && (
          <section className="space-y-3 border-t border-[var(--tc-hairline)] pt-3">
            <h2 className="text-[10px] font-medium uppercase tracking-widest text-slate-500">Bindings</h2>
            {def.bindSlots.map((slot) => (
              <BindRow key={`${id}:${slot}`} id={id} slot={slot} shape={shape} />
            ))}
          </section>
        )}
      </div>
    );
  })();

  return (
    <aside className="w-64 shrink-0 overflow-y-auto border-l border-[var(--tc-hairline)] bg-[var(--tc-panel)] p-3 backdrop-blur-md">
      {body}
    </aside>
  );
}
