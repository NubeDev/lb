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

// UI — the props-driven grouped <select>.
export { SourcePicker } from "./SourcePicker";
export type { SourcePickerProps } from "./SourcePicker";

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
  NodeDescriptor,
  DatasourceRow,
} from "./types";
