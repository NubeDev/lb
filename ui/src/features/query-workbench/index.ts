// The query-workbench feature barrel (query-workbench-view scope, slice 3). Re-exports only; no
// component bodies (FILE-LAYOUT frontend rule: `index.ts` is a barrel, exactly like Rust's `mod.rs`).

export { QueryWorkbench, type QueryWorkbenchProps } from "./QueryWorkbench";
export { SURREAL_LOCAL, useQueryRun, runKindFor, type QueryRunState } from "./useQueryRun";
