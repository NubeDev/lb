# Five failed queries took federation dark — an error REPLY was treated as a child crash

**Area:** federation / native-tier supervision
**Date:** 2026-07-05
**Symptom:** After a handful of failed `federation.query` calls (bad table name, planner error),
every further federation call answered `supervisor: restart budget exhausted after 5 restarts` —
the datasource was dead for the session. Reproduced deterministically: 6 queries against a
non-existent relation → the last returned budget-exhausted.

## Root cause

`native/call.rs::call_once_or_restart` treated `SupervisorError::Child(_)` as "the child died
mid-call" and ran the restart recovery. But `Child(_)` is the child's ordinary **error reply over a
healthy control line** — a failed SQL query, a bad arg. Every failed query therefore burned one
supervised restart of a perfectly healthy child; five failed queries exhausted the budget and every
subsequent call short-circuited on it. (The child itself never crashed: its `handle_call` returns
`Reply::err` on every engine error.)

## Fix

- Only `SupervisorError::Transport(_)` (no reply came back — the line/child actually broke) enters
  the fault/restart path; a `Child` error surfaces to the caller as the tool error it is.
- Defense in depth in the sidecar (`extensions/federation/src/main.rs`): each `call` is fenced in
  its own tokio task, so a genuine PANIC deep in an engine/connector becomes an error reply instead
  of a dead child.
- Related steering (`extensions/federation/src/query.rs`): a bare `COUNT(*)`/`COUNT(1)` plans a
  zero-column scan the pushdown provider mis-schemas (upstream datafusion-table-providers bug,
  "Physical input schema should be the same…"); the execute error is rewritten to
  `COUNT(*) over a whole table is not supported … count a concrete column instead, e.g. COUNT(id)`
  so the (usually AI) caller can self-correct in one turn.

## Regression tests

`native_test` suite green over the fault-path change; verified live — 8 consecutive failed queries
all returned clean errors and the 9th (healthy) query succeeded on the same child, and the
`COUNT(*)` steer returns verbatim.
