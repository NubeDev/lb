# `push_muted` was silently dropped by the SCHEMAFULL prefs table (quiet hours never held)

- Area: inbox-outbox (push target) / prefs
- Status: fixed
- First seen: 2026-07-11 (the new relay-driven quiet-hours test failed: the muted member was sent)
- Resolved: 2026-07-11
- Session: ../../sessions/inbox-outbox/push-target-review-fixes-session.md
- Regression test: `rust/crates/host/tests/push_deliver_test.rs::quiet_hours_suppresses_muted_member`

## Symptom

`set_user_prefs(…, Prefs { push_muted: Some(true), .. })` returned `Ok`, but the push target's
quiet-hours gate never fired — `get_user_prefs` came back with `push_muted: None` and the muted
member's device was sent anyway.

## Root cause

The push-target slice added the `push_muted` axis to `lb_prefs::Prefs` but not to the store
layer's two mirrors in `crates/prefs/src/store/schema.rs`:

1. the `DEFINE FIELD` list — the table is **SCHEMAFULL**, so SurrealDB silently discards an
   undefined field on write;
2. `PREFS_COLUMNS` — the explicit read projection, so even a stored value would never be
   selected back.

The known "closed struct, not KV" prefs pattern has a third mirror the memory note didn't
capture: struct axis + schema `DEFINE FIELD` + `PREFS_COLUMNS`. Two of three were missed and
nothing failed — SCHEMAFULL drops are silent, and no test read the axis back through the store.

## Fix

Additive, both tables (`user_prefs` + `workspace_prefs`):
`DEFINE FIELD IF NOT EXISTS push_muted … TYPE option<bool>;` and `push_muted` appended to
`PREFS_COLUMNS`. Idempotent `IF NOT EXISTS` — existing stores heal on next touch.

## Lesson

A new `Prefs` axis is **three** mirrors (struct, `DEFINE FIELD` ×2 tables, `PREFS_COLUMNS`) —
and the only test that proves the axis exists is one that writes it and reads it back through
the real store, not one that constructs the struct in memory.
