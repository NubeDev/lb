# Query builder (public) — TODO

Status: **TODO stub.** Filled when the query-builder 10x slices ship (see
`docs/scope/frontend/query-builder/`).

This will document the shipped truth of:

- the **visual canvas builder** — drag tables, connect columns to make joins, per-column
  aggregation/alias, WHERE/HAVING with AND/OR, multi-sort, over the typed `SqlBuilderQuery` + `emitSql`
  dialect seam (slice 1);
- the **schema-aware SQL editor** — CodeMirror completion fed by `store.schema`/`federation.schema`, the
  statement splitter, and the Format button (slice 2);
- the **Query workbench** — the `/t/$ws/query` standalone surface that also opens as a Data Studio pane,
  runs `store.query`/`federation.query`, and renders results (slice 3).

Until then, the shipped precedent it extends is documented under **"Query builder — common across dialects"**
in [`data-studio.md`](../frontend/data-studio.md).
</content>
