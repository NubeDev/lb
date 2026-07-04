// The Template options editor — the panel-builder glue that mounts the (orphaned-until-now)
// `TemplateSourceField` for a `view:"template"` cell (render-template-inprocess scope, "editable in Data
// Studio"). It is the ONLY template-authoring surface in the builder: the Inline↔Saved toggle + the
// CodeMirror HTML body live in `TemplateSourceField`; this file is the ONE bridge between that field's
// `TemplateValue` and the editor's `EditorState`.
//
// Where the values ride: `code`/`templateId` are NOT `OWNED_OPTION_KEYS`, so they round-trip through
// `state.carry.extraOptions` verbatim (no serializer change — render-template-inprocess scope, "How it
// fits the core → Dependencies"). `TemplateView` reads `options.code ?? options.templateId`. We patch
// `carry.extraOptions`: inline mode writes `code` (and drops a stale `templateId`); saved mode writes
// `templateId` (and drops a stale `code`) — so the two never coexist ambiguously.
//
// The live preview is the real in-process `TemplateView` (PreviewPane → WidgetView → TemplateView); an
// edit-the-code loop re-renders against the frames already fetched, no `viz.query` re-fetch (the
// fetch/shape split already gives this). One responsibility: TemplateValue ↔ EditorState.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { TemplateSourceField, type TemplateValue } from "@/features/dashboard/builder/editors/TemplateSourceField";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

/** Read the current template value off the editor state's carried extras. `templateId` wins if set
 *  (Saved mode); else `code` (Inline mode); else default to Inline with the shipped starter snippet. */
function readValue(state: EditorState): TemplateValue {
  const ex = (state.carry.extraOptions ?? {}) as Record<string, unknown>;
  if (typeof ex.templateId === "string" && ex.templateId) {
    return { mode: "saved", templateId: ex.templateId };
  }
  if (typeof ex.code === "string") {
    return { mode: "inline", code: ex.code };
  }
  return { mode: "inline", code: "" };
}

export function TemplateOptionsEditor({ state, patch }: Props) {
  const value = readValue(state);
  const onChange = (next: TemplateValue) => {
    // Write ONLY the active mode's key; drop the other so TemplateView's inline-wins resolution is
    // never ambiguous across a mode switch.
    const extraOptions: Record<string, unknown> = { ...state.carry.extraOptions };
    delete extraOptions.code;
    delete extraOptions.templateId;
    if (next.mode === "inline") extraOptions.code = next.code;
    else extraOptions.templateId = next.templateId;
    patch({ carry: { ...state.carry, extraOptions } });
  };
  return <TemplateSourceField value={value} onChange={onChange} />;
}
