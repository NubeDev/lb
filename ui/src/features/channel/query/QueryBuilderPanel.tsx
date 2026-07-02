// The in-channel "Customize chart" panel — wraps the shared `PlotBuilder` with a draft spec and a
// Save/Cancel footer. Editing is local (live preview updates as you go); Save commits the draft up to
// the card, which persists it as the viewer's per-item plot preference. Kept separate from `QueryCard`
// so the card owns mode/persistence and this owns only the draft-edit interaction (FILE-LAYOUT).

import { useState } from "react";
import { Check, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { PlotBuilder } from "@/features/charts";
import type { FieldInfo, PlotSpec } from "@/lib/charts";
import { isPlottable } from "@/lib/charts";

interface Props {
  fields: FieldInfo[];
  rows: Array<Record<string, unknown>>;
  initial: PlotSpec;
  saving: boolean;
  onSave: (spec: PlotSpec) => void;
  onCancel: () => void;
}

export function QueryBuilderPanel({ fields, rows, initial, saving, onSave, onCancel }: Props) {
  const [draft, setDraft] = useState<PlotSpec>(initial);

  return (
    <div className="rounded-xl border border-border bg-panel/30 p-3">
      <PlotBuilder fields={fields} rows={rows} spec={draft} onChange={setDraft} />
      <div className="mt-3 flex items-center justify-end gap-2 border-t border-border/60 pt-3">
        <Button type="button" variant="ghost" size="sm" onClick={onCancel} className="h-8 gap-1 px-3 text-xs">
          <X size={14} /> Cancel
        </Button>
        <Button
          type="button"
          size="sm"
          disabled={saving || !isPlottable(draft)}
          onClick={() => onSave(draft)}
          className="h-8 gap-1 px-3 text-xs"
        >
          <Check size={14} /> {saving ? "Saving…" : "Save chart"}
        </Button>
      </div>
    </div>
  );
}
