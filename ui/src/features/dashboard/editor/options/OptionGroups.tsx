// The registry-driven option renderer (editor-parity scope, step 2) — given a list of `OptionDef`s and
// the editor state, renders them grouped, each row bound to the state via read/writeOption + the one
// `Control`. The Field tab and the per-viz tabs both render through THIS, so every option is authored by
// its registered control, consistently. An optional `search` filters options by label/id/keywords (the
// options-search box, now actually useful because the options exist). One responsibility: render option
// rows from the registry.

import type { EditorState } from "../cellEditorState";
import type { OptionDef } from "./types";
import { groupOptions } from "./registry";
import { readOption, writeOption } from "./binding";
import { Control } from "./Control";

/** The rich controls that render full-width UNDER their label (not inline beside it). */
const BLOCK_CONTROLS = new Set(["thresholds", "mappings", "color-scheme", "data-links"]);

interface Props {
  defs: OptionDef[];
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  /** Optional search query — filters options by label/id/keywords. */
  search?: string;
}

/** Does `def` match the search query? */
function matchesSearch(def: OptionDef, q: string): boolean {
  if (!q) return true;
  const needle = q.toLowerCase();
  return (
    def.label.toLowerCase().includes(needle) ||
    def.id.toLowerCase().includes(needle) ||
    def.group.toLowerCase().includes(needle) ||
    (def.keywords ?? []).some((k) => k.toLowerCase().includes(needle))
  );
}

export function OptionGroups({ defs, state, patch, search = "" }: Props) {
  const filtered = defs.filter((d) => matchesSearch(d, search));
  const groups = groupOptions(filtered);

  if (groups.length === 0) return <p className="py-2 text-xs text-muted">No matching options.</p>;

  return (
    <div className="grid gap-3 py-3 text-xs" aria-label="option groups">
      {groups.map(({ group, options }) => (
        <section key={group} className="grid gap-2" data-options-group={group.toLowerCase().replace(/\s+/g, "-")}>
          <div className="font-medium text-muted">{group}</div>
          {options.map((def) => {
            const value = readOption(state, def);
            const set = (v: unknown) => patch(writeOption(state, def, v));
            const block = BLOCK_CONTROLS.has(def.control.kind);
            if (block) {
              return (
                <div key={def.id} className="grid gap-1">
                  <span className="text-muted">{def.label}</span>
                  <Control control={def.control} label={def.label} value={value} onChange={set} />
                </div>
              );
            }
            if (def.control.kind === "toggle") {
              return (
                <label key={def.id} className="flex items-center gap-2 text-muted">
                  <Control control={def.control} label={def.label} value={value} onChange={set} />
                  {def.label}
                </label>
              );
            }
            return (
              <label key={def.id} className="grid gap-1 text-muted">
                {def.label}
                <Control control={def.control} label={def.label} value={value} onChange={set} />
              </label>
            );
          })}
        </section>
      ))}
    </div>
  );
}
