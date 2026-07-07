# Store `scan` returns the data envelope, not the record — insights reads decode `undefined` fields

- Area: insights/store
- Status: resolved
- First seen: 2026-07-05
- Resolved: 2026-07-05
- Session: ../../sessions/insights/insights-session.md
- Regression test: `rust/crates/host/tests/insights_test.rs::list_in_one_workspace_never_returns_another_workspaces_insights` + `::digest_reactor_is_idempotent_on_rerun`

## Symptom

`insight.list` and the digest reactor's `all_notify` scan returned **zero rows** (or rows that
failed to decode into `Insight`/`NotifyState` with `serde` ignoring the unknown `data` wrapper),
so the Insights page rendered empty and the reactor never found due digests — even though
`insight.get` (a single-row `read`) returned the record fine.

## Reproduce

Raise an insight, then `insight.list {}` → `{items: []}`. A direct `read` of the same id returns
the full record. The list/scan path and the read path disagreed on the row shape.

## Investigation

Two store read paths, two shapes:

- `lb_store::read` / `lb_store::list` select the **inner `data` field** of a `write`-based record
  (see `store::record`) — the host value, ready to decode.
- `lb_store::scan` (the id-cursor paging API the list/admin-lens/reactor use to walk a *whole*
  table) selects the **whole record** — `{data: {...}, rev}`.

`scan_all` was passing `scan`'s rows straight to `serde_json::from_value::<Insight>`, so every row
had a top-level `data` object and none of `Insight`'s fields → decode failed silently
(`filter_map(|v| from_value(v).ok())` swallowed it) → empty list. Ruled out the workspace wall
(the scan IS ws-scoped) and the write path (`get` round-tripped). The shape mismatch between `scan`
and `read`/`list` was the crack.

## Root cause

`scan_all` (insights internal helper) didn't unwrap the `data` envelope that `lb_store::scan`
returns for `write`-based rows. The occurrence ring is unaffected because capped rows are stored
**flat** by `lb_store::capped_insert` and read by their own direct query — which is exactly why
`occurrences` worked while `list` didn't.

## Fix

`rust/crates/insights/src/table_scan.rs:36-39` — `scan_all` unwraps the inner `data` before
returning each row: `obj.remove("data").unwrap_or(Value::Object(obj))`. Callers (`list`,
`sub_list` admin lens, `all_notify`) now get the same shape `read`/`list` return, so
`serde_json::from_value::<Insight>` decodes cleanly. The cap-rows exception is documented at the
top of `table_scan.rs` so no one re-routes `occurrences` through it.

## Verification

`cargo test -p lb-host --test insights_test` — `list_in_one_workspace_never_returns_another_workspaces_insights`
(1 row round-trips through the scan path) and `digest_reactor_is_idempotent_on_rerun` (the reactor
finds due rows via `all_notify`) both green.

## Prevention

The shape contract is now stated in `table_scan.rs`'s doc comment: "`scan` returns the whole
record; a `write`-based row wraps the host value under `data`. Unwrap it." A future typed
`scan`-returns-record API in `lb_store` would make the class impossible; until then the unwrap +
the comment are the guard.
