// `@nube/source-picker` — public surface.
//
// The Lazybones "pick a value from the DB / datasources / Zenoh (live series) / flows / extension
// widgets" machinery, extracted from the dashboard so any surface reuses ONE picker. Transport-
// agnostic: the host injects a `SourceLoaders` (how to reach the node) — the shell delegates to its
// gateway/Tauri clients; an extension delegates to its host bridge. Self-themed via `--sp-*` tokens
// scoped to `.sp-root`, host-overridable.
//
// Consumed two ways:
//   - `workspace:*` from the lb `ui/` app (the dashboard),
//   - imported by a standalone extension UI (e.g. thecrew) — `import { ... }` + `import
//     '@nube/source-picker/style.css'`.
//
// Three layers, adopt what you need: the MODEL (pure), the LOADER hook, the UI component.

import "./source-picker.css";

// Model (pure) — build entries from loader results, fold an entry to a selection.
export {
  buildSourceEntries,
  selectionOf,
  widgetIdOf,
  seriesEntries,
  liveEntries,
  extensionEntries,
  extWidgetEntries,
  flowsEntries,
  rulesEntries,
  queryEntries,
  sqlSourceEntry,
  SQL_SOURCE_ID,
} from "./sourcePicker";
export type { SourceEntry, SourceInputs } from "./sourcePicker";

// Loader hook — orchestrates the injected reads into entries (deny-tolerant, ws-keyed).
export { useSourcePicker } from "./useSourcePicker";
export type { SourcePickerData } from "./useSourcePicker";

// Pure loader — the async assembly (no React), for a host that drives it through its own cache layer.
export { loadSourcePicker } from "./loadSourcePicker";
export type { SourcePickerResult } from "./loadSourcePicker";

// Catalog — the workspace system catalog: sections AS DATA + a per-section loader orchestration
// (system-catalog scope). The picker folds the same per-section state into flat entries; the
// explorer skin renders it as a tree. One orchestration (`loadCatalog`/`useCatalog`), two
// projections (`loadSourcePicker`/`useSourcePicker` collapse; `CatalogExplorer` surfaces).
export {
  CATALOG_SECTION_SPECS,
  channelEntries,
  datasourceEntries,
  inboxEntries,
  insightEntries,
  queryCatalogEntries,
  schemaColumnEntries,
  schemaTableEntries,
  seriesCatalogEntries,
} from "./catalog";
export type {
  CatalogEntry,
  CatalogSectionKind,
  CatalogSectionSpec,
} from "./catalog";
export { loadCatalog } from "./loadCatalog";
export type { CatalogSectionData, CatalogSections } from "./loadCatalog";
export { useCatalog } from "./useCatalog";

// UI — the props-driven grouped <select>, plus the shared grouping primitive + canonical group lists a
// host reuses when it renders its own <select> (shadcn Select / a token-classed native select).
export { SourcePicker, PickerGroup, READ_SOURCE_GROUPS, BUILDER_SOURCE_GROUPS } from "./SourcePicker";
export type { SourcePickerProps, SourceGroup } from "./SourcePicker";

// UI — the SEARCHABLE combobox variant (type-to-filter grouped popover). Optional richer alternative to
// the `<select>`; same model + tokens. A host that wants a plain select keeps `SourcePicker`.
export { SourceCombobox } from "./SourceCombobox";
export type { SourceComboboxProps } from "./SourceCombobox";

// UI — the CATALOG EXPLORER (the tree skin over `useCatalog`; system-catalog scope). The combobox is
// "pick a source by typing"; the explorer is "browse the workspace's subsystems as a tree, click to
// insert." Three pieces: the explorer panel, the per-section renderer, and the schema table→column
// tree (moved in from the shell so the package owns ONE schema tree, two consumers).
export { CatalogExplorer } from "./CatalogExplorer";
export type { CatalogExplorerProps } from "./CatalogExplorer";
export { CatalogSection, CatalogEmpty } from "./CatalogSection";
export type { CatalogSectionProps } from "./CatalogSection";
export { CatalogSchemaTree } from "./CatalogSchemaTree";
export type { CatalogSchemaTreeProps } from "./CatalogSchemaTree";

// Types — the vocabulary + the injected seam.
export type {
  Source,
  Action,
  SourceSelection,
  SourceLoaders,
  ExtUi,
  ExtRow,
  Flow,
  FlowNode,
  FlowSummary,
  RuleSummary,
  RuleParam,
  ParamKind,
  QuerySummary,
  NodeDescriptor,
  DatasourceRow,
  Schema,
  SchemaTable,
  SchemaColumn,
  ChannelRow,
  InsightRow,
  InboxRow,
  SectionState,
} from "./types";
