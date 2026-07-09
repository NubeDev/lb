// Read/write a registered option's value against the editor state (editor-parity scope, step 2). An
// option's `scope` picks the root — `fieldConfig` → `state.fieldConfig.defaults`, `options` →
// `state.options` — and its `path` addresses the (possibly nested) value there. Writing prunes empty
// groups so an unset option never materializes (`setPath` handles the leaf; here we also drop an empty
// `fieldConfig` back to absent, matching the cellEditorState round-trip contract). Pure — no React.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { FieldConfig } from "@/lib/dashboard";
import type { OptionDef } from "./types";
import { optionPath } from "./registry";
import { getPath, setPath } from "./path";

/** Read the current value of `def` from the editor state (undefined if unset). */
export function readOption(state: EditorState, def: OptionDef): unknown {
  const path = optionPath(def);
  if (def.scope === "fieldConfig") return getPath(state.fieldConfig?.defaults as Record<string, unknown> | undefined, path);
  return getPath(state.options, path);
}

/** Return the state patch that sets `def` to `value` (undefined clears it). Prunes empty groups so the
 *  round-trip stays clean — an all-empty `fieldConfig` collapses back to absent. */
export function writeOption(state: EditorState, def: OptionDef, value: unknown): Partial<EditorState> {
  const path = optionPath(def);
  if (def.scope === "fieldConfig") {
    const fc: FieldConfig = state.fieldConfig ?? { defaults: {}, overrides: [] };
    const defaults = setPath(fc.defaults as Record<string, unknown>, path, value);
    const next: FieldConfig = { ...fc, defaults };
    // Collapse an entirely-empty fieldConfig (no defaults, no overrides) back to absent.
    const empty = Object.keys(defaults).length === 0 && (!next.overrides || next.overrides.length === 0);
    return { fieldConfig: empty ? undefined : next };
  }
  return { options: setPath(state.options, path, value) };
}

/** Return the state patch that sets a picked geocoded place onto the weather tile's location options —
 *  `options.label`, `options.lat`, `options.lon` in ONE patch (the geo-search control writes all three
 *  atomically; the single-path `writeOption` can't set sibling options). Pure. */
export function writeGeoPlace(
  state: EditorState,
  place: { label: string; lat: number; lon: number },
): Partial<EditorState> {
  let options = state.options;
  options = setPath(options, "label", place.label || undefined);
  options = setPath(options, "lat", place.lat);
  options = setPath(options, "lon", place.lon);
  return { options };
}
