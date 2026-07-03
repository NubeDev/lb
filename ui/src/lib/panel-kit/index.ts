// panel-kit — the HEADLESS panel-building + source-querying logic layer (data-studio scope v2).
// Extracted from the dashboard's editor so ANY surface can author panels with its own views: the
// panel-spec editing state machine, the source→draft-cell mapping, save-as-library, the SQL builder
// model, and the GenUI authoring hook. Strictly logic: no JSX, no `@/components`, no `@/features`
// imports — only `@/lib/*` + `@nube/genui`. (Package-shaped on purpose: promoting this to a
// `packages/@nube/panel-kit` workspace lib is the named follow-up; the blocker is the `@/lib/dashboard`
// type graph, not this code.) Views live with their consumers: `features/panel-builder` (the tabbed
// option surface Data Studio mounts), `features/data-studio` (the FlexLayout workbench).

export { cellToEditorState, editorStateToCell, type EditorState } from "./cellEditorState";
export { defaultCell } from "./defaultCell";
export { draftFromSelection } from "./draftFromSelection";
export { saveDraftAsPanel, slugify } from "./saveAsLibrary";
export {
  usePanelEditor,
  PLOTTABLE_VIEWS,
  type PanelEditorMachine,
  type UsePanelEditorOptions,
} from "./usePanelEditor";
export { useGenUiAuthor, GENUI_SKILL, type GenUiAuthorState } from "./useGenUiAuthor";
export {
  emptyQuery,
  emptySqlSource,
  type SqlBuilderQuery,
  type SqlColumn,
  type SqlFilter,
  type SqlAggregation,
  type SqlEditorMode,
  type SqlFormat,
  type SqlOperator,
  type SqlSourceState,
} from "./sql/query";
export { toSurrealQL } from "./sql/toSurrealQL";
