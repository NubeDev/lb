// The Field tab (viz panel-editor scope; field-config scope owns the semantics) — authors
// `fieldConfig.defaults`: the per-FIELD option set. Since step 2 (editor-parity) it renders ENTIRELY
// from the option registry (`options/registry.ts`) via `OptionGroups`: the standard options
// (displayName/unit/decimals/min/max/noValue/color-scheme/thresholds/value-mappings/data-links) plus
// the current view's per-field `custom.*` options (timeseries graph styles + axis). Adding a field
// option is a registry edit, not a change here. The unit picker is searchable+grouped; thresholds carry
// a mode toggle + swatches; value mappings and color scheme finally have editors. One responsibility:
// render the field-scoped registry options.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { canonicalView, type View } from "@/lib/dashboard";
import { optionsForView } from "../options/registry";
import { OptionGroups } from "../options/OptionGroups";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  /** The options-search query (shared with the per-viz tab), filters the rendered option rows. */
  search?: string;
}

export function FieldTab({ state, patch, search }: Props) {
  const view = canonicalView((state.view || "timeseries") as View);
  // The Field tab owns the `fieldConfig`-scoped options for this view (standard + per-field custom).
  const defs = optionsForView(view).filter((d) => d.scope === "fieldConfig");
  return (
    <div aria-label="field tab">
      <OptionGroups defs={defs} state={state} patch={patch} search={search} />
    </div>
  );
}
