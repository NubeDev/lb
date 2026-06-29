# `rules.list` / `chains.list` return an empty roster even with saved records

- **Area:** host (`crates/host/src/rules/get.rs`, `crates/host/src/chains/get.rs`)
- **Status:** resolved
- **Found by:** the rules-workbench frontend slice
  ([rules-workbench-session.md](../../sessions/frontend/rules-workbench-session.md)) — the Playground
  rail (`rules.list`) and the chain rail (`chains.list`) came back empty for a workspace that had just
  saved a rule/chain through the real `rules.save`/`chains.save` path.

## Symptom

`rules.save` then `rules.list` → `{rules: []}`. `rules.get {id}` on the same id returns the record
fine. Identical for `chains.save` → `chains.list` → `{chains: []}` while `chains.get` works. No error
is logged — the roster is silently empty.

## Root cause

Records written via `lb_store::write` are stored wrapped in a `{ data: <record>, rev: N }` **envelope**.
`lb_store::read` (used by `*_get`) unwraps it and returns the inner record. But `lb_store::scan` (used
by `*_list`) returns the **whole** envelope row. Both list verbs decoded `row.data` directly:

```rust
// rules_list (before)
if let Ok(rule) = serde_json::from_value::<SavedRule>(row.data) { ... }
```

`row.data` is the `{data, rev}` envelope, not a `SavedRule`, so `from_value` fails for **every** row —
and because the result is swallowed by `if let Ok(...)`, the failure is silent and the roster is always
empty. `chains_list` had the same bug *and* read its `deleted` tombstone flag off the envelope
(`row.data.get("deleted")`), which is never present at that level, so the tombstone check never fired
either. The working `scan_dashboards` in `dashboard/store.rs` already unwraps the envelope — the two
list verbs simply didn't follow it.

## Fix

Unwrap the `{ data: ... }` envelope before decoding (mirroring `scan_dashboards`):

```rust
let inner = match row.data {
    serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
    other => other,
};
if let Ok(rule) = serde_json::from_value::<SavedRule>(inner) { ... }
```

`chains_list` reads `deleted` and decodes the `Chain` from the unwrapped `inner`.

## Regression test

`rust/role/gateway/tests/rules_routes_test.rs::rules_crud_round_trip_over_the_gateway` and
`chains_routes_test.rs::chains_crud_round_trip_over_the_gateway` now assert the roster **contains** the
saved id (`ids.contains(&"hot")` / `&"pipe"`) — fails-before (empty roster), passes-after. The host
crate's existing `rules`/`chains` tests stay green.

## Note

This was a **shipped host bug**, not new backend work — the rules-workbench scope's "no host changes"
boundary is about not *building* new backend, and the CRUD/rail round-trip it specifies is impossible
with a list verb that always returns empty. Fixing the one-line envelope unwrap (with a regression
test) is the correct long-term call (HOW-TO-CODE §3.8 — fix the scope/code when building reveals it was
wrong; don't silently diverge).
