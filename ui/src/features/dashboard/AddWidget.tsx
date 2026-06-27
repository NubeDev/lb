// The widget palette — the dashboard editor's "add a tile" control (dashboard scope). The user picks
// a built-in widget type and a binding (an explicit series, or a tag-facet query), and a new cell is
// appended to the layout. This is the same `{widget_type, binding}` contract Phase 2's federated
// widget palette extends with `ext:<id>` tiles — the shape doesn't change, only the renderer.

import { useState } from "react";
import { Plus } from "lucide-react";

import type { Binding, Cell, WidgetType } from "@/lib/dashboard";

interface Props {
  existing: Cell[];
  onAdd: (cell: Cell) => void;
}

const TYPES: WidgetType[] = ["chart", "stat", "gauge"];

/** A fresh cell key that doesn't collide with the existing ones. */
function nextKey(existing: Cell[]): string {
  let n = existing.length + 1;
  const keys = new Set(existing.map((c) => c.i));
  while (keys.has(`w${n}`)) n += 1;
  return `w${n}`;
}

export function AddWidget({ existing, onAdd }: Props) {
  const [type, setType] = useState<WidgetType>("chart");
  const [series, setSeries] = useState("");
  const [tags, setTags] = useState("");

  const add = () => {
    const trimmedSeries = series.trim();
    const tagList = tags
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);
    // A binding is an explicit series OR a tag-facet query — prefer the explicit series when given.
    let binding: Binding;
    if (trimmedSeries) binding = { series: trimmedSeries };
    else if (tagList.length) binding = { find: { tags: tagList } };
    else return; // nothing to bind to — no-op (the form stays open)

    // Stack new cells down the left column at a default size; the user then drags/resizes.
    const y = existing.reduce((m, c) => Math.max(m, c.y + c.h), 0);
    onAdd({ i: nextKey(existing), x: 0, y, w: 4, h: 3, widget_type: type, binding, options: {} });
    setSeries("");
    setTags("");
  };

  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-border bg-panel px-3 py-3 text-xs">
      <span className="font-medium text-muted">Add widget</span>
      <select
        aria-label="widget type"
        className="control-field-sm"
        value={type}
        onChange={(e) => setType(e.target.value as WidgetType)}
      >
        {TYPES.map((t) => (
          <option key={t} value={t}>
            {t}
          </option>
        ))}
      </select>
      <input
        aria-label="widget series"
        placeholder="series (e.g. cooler.temp)"
        className="control-field-sm w-48"
        value={series}
        onChange={(e) => setSeries(e.target.value)}
      />
      <span className="text-muted">or tags</span>
      <input
        aria-label="widget tags"
        placeholder="kind:temperature, store:downtown-0421"
        className="control-field-sm w-64"
        value={tags}
        onChange={(e) => setTags(e.target.value)}
      />
      <button
        aria-label="add widget"
        className="soft-button-sm"
        onClick={add}
      >
        <Plus size={12} /> Add
      </button>
    </div>
  );
}
