# jobs: `list_kind` ORDER BY fails to parse — SurrealDB requires the order idiom in the selection

- **Date:** 2026-07-15 · **Area:** jobs (`lb-jobs::list_kind`, feeds `rules.runs.list`) ·
  **Status:** fixed + covered

## Symptom

`rules.runs.list` returned
`store backend error: Parse error: Missing order idiom `data.ts` in statement selection` —
two host integration tests (`ws_b_cannot_see_or_control_a_ws_a_run`,
`progress_and_result_surface_in_get_and_list`) failed on the first call.

## Root cause

SurrealDB (pinned engine) rejects `SELECT data FROM … ORDER BY data.ts` — an ORDER BY idiom must
appear in the statement's selection. The `pending` drain never hit this (no ORDER BY); the new
kind-scoped observe read did.

## Fix

Project the sort key alongside the body and order by the alias
(`crates/jobs/src/list_kind.rs`):

```sql
SELECT data, data.ts AS ts FROM type::table($tb)
 WHERE data.kind = $kind … ORDER BY ts DESC LIMIT …
```

The decode drops the extra `ts` field (it exists only to satisfy the parser).

## Covered by

`crates/host/tests/rules_longrun_test.rs::progress_and_result_surface_in_get_and_list` (asserts
ordering-backed list output) and `::ws_b_cannot_see_or_control_a_ws_a_run` (list on the empty side
of the wall).

## Lesson

Any new SurrealDB query that sorts on a nested field must project that field (or its alias) in the
selection — grep for `ORDER BY data.` when adding list verbs.
