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

import { useEffect } from "react";

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { CopyTemplatePrompt } from "../../CopyTemplatePrompt";
import {
  DEFAULT_INLINE_CODE,
  TemplateSourceField,
  type TemplateValue,
} from "@/features/dashboard/builder/editors/TemplateSourceField";

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

  // A freshly-picked template cell has NEITHER code nor a templateId — without this seed the editor
  // is empty and the preview says "no template" (the starter used to appear only after a redundant
  // click on the already-active Inline tab). Seed the shipped example ONCE so the panel renders the
  // moment the view is picked; guarded so a user-cleared editor ("" is a string) is never overwritten.
  const unset = value.mode === "inline" && typeof (state.carry.extraOptions as Record<string, unknown> | undefined)?.code !== "string";
  useEffect(() => {
    if (unset) {
      patch({ carry: { ...state.carry, extraOptions: { ...state.carry.extraOptions, code: DEFAULT_INLINE_CODE } } });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fire only while the value is unset
  }, [unset]);
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
  // The draft's active data binding, for the AI prompt's provenance section: a federation target
  // carries its SQL in args; a SQL-builder draft stows raw SQL in state.sql; structured reads
  // (series/flows) just name the tool.
  const target = state.targets?.[0];
  const args = (target?.args ?? {}) as Record<string, unknown>;
  const query = {
    tool: target?.tool,
    source: typeof args.source === "string" ? args.source : undefined,
    sql: typeof args.sql === "string" ? args.sql : state.sql?.rawSql,
  };

  return (
    <div className="grid gap-2">
      <CopyTemplatePrompt query={query} />
      <TemplateSourceField value={value} onChange={onChange} />
    </div>
  );
}
