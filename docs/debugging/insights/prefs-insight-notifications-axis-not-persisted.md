# The `insight_notifications` prefs axis didn't persist — the kill switch silently did nothing

- Area: insights/notify + prefs
- Status: resolved
- First seen: 2026-07-05
- Resolved: 2026-07-05
- Session: ../../sessions/insights/insights-session.md
- Regression test: `rust/crates/host/tests/insights_test.rs::member_kill_switch_off_skips_all_deliveries`

## Symptom

Setting `insight_notifications: false` in prefs had no effect — the kill switch was documented as
"the member-global off switch for all insight deliveries", but a member with it off still got
channel posts on a matched subscription.

## Reproduce

`prefs.set {prefs:{insight_notifications:false}}`, then raise an insight that matches one of the
member's subs → a delivery still landed in the channel inbox.

## Investigation

`lb_prefs::Prefs` had the new `insight_notifications: Option<bool>` field (serde default `true`),
and the host delivery path read it at fire time. So the read path was right; the suspicion fell on
the write. `lb_prefs` is SCHEMAFULL (the prefs scope: each axis NULLABLE so unset → inherit is
structural) — a SCHEMAFULL table silently drops a field that isn't DECLARED. Reading
`prefs/src/store/schema.rs`: the `PREFS_COLUMNS` projection and the `DEFINE FIELD` list both
enumerated every axis explicitly, and `insight_notifications` was in neither. So `prefs.set` wrote
the field in the record body, SurrealDB's SCHEMAFULL table dropped it (no matching FIELD), and the
`PREFS_COLUMNS` projection didn't even select it on read-back → the axis round-tripped as
`None` (the serde default), kill switch never engaged.

Ruled out: the host delivery-time read (correct), the serde model (field present + defaulted), and
the verb wiring (the axis deserialized). The schema declaration was the gap.

## Root cause

A new nullable prefs axis was added to the Rust `Prefs` model without the matching SCHEMAFULL
declaration (`DEFINE FIELD`) AND the matching read projection (`PREFS_COLUMNS`). SCHEMAFULL means
"drop unknown fields silently"; the two lists must stay in lock-step with the `Prefs` struct, or an
axis persists as a no-op.

## Fix

`rust/crates/prefs/src/store/schema.rs` — `PREFS_COLUMNS` + the `DEFINE FIELD` block for BOTH
tables (`user_prefs` + `workspace_prefs`) now include
`insight_notifications TYPE option<bool>` (lines 16, 40, 54). (`agent_persona` was added in the
same edit by a concurrent session — kept.)

## Verification

`cargo test -p lb-host --test insights_test::member_kill_switch_off_skips_all_deliveries` —
sets the axis false via the real `prefs.set` path, raises a matched insight, asserts the channel
inbox is empty (delivery skipped, accounting continues). Green.

## Prevention

The contract is now: every new `Prefs` axis MUST be added to (a) the Rust struct, (b)
`PREFS_COLUMNS`, (c) both `DEFINE FIELD` blocks. A future test that asserts the column list +
DEFINE list == the `Prefs` fields (derive the list from the struct) would make the class
impossible; until then, the prefs-scope doc carries the rule.
