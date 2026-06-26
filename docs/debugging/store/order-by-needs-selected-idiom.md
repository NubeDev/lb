# `ORDER BY data.ts` fails: "Missing order idiom `data.ts` in statement selection"

- Area: store
- Status: resolved
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/bus/messaging-session.md
- Regression test: rust/crates/host/tests/messaging_test.rs (`history_survives_independent_of_the_bus` asserts oldest→newest order)

## Symptom

The first messaging test to read channel history panicked from the store layer:

```
Store(Backend("Parse error: Missing order idiom `data.ts` in statement selection
 --> [1:72]
  | ... ORDER BY data.ts ASC
  |              ^^^^^^^^^^^
1 | SELECT data FROM type::table($tb) WHERE data.channel = $value ORDER BY data.ts ...
"))
```

## Reproduce

Run any channel `history`/`list` read with the original generic store query:

```sql
SELECT data FROM type::table($tb) WHERE data.channel = $value ORDER BY data.ts ASC
```

against the embedded SurrealDB (`kv-mem`, surreal 2.x).

## Investigation

- Ruled out a binding problem (`$tb`/`$value` resolved fine — the error is a *parse* error
  about the ORDER idiom, not a runtime/type error).
- The projection selects the wrapper field `data`; SurrealDB requires the `ORDER BY` idiom to
  resolve against a *selected* field. `data.ts` is nested inside the selected `data` object,
  not a top-level selected idiom, so the planner rejects it.
- Options weighed: (a) project `data.ts AS ts` and order by `ts` — pollutes the generic store
  verb with a messaging-specific column; (b) `SELECT VALUE data ... ORDER BY data.ts` — still
  couples ordering to the caller's value shape; (c) keep the generic store `list` a pure
  *filter* and sort in the layer that owns the ordered shape.

## Root cause

Ordering was placed in the **generic** store `list` verb, which only knows it returns opaque
`data` JSON — it has no business knowing the order key lives at `data.ts`. The SurrealDB
idiom-in-projection rule surfaced that mis-layering as a parse error.

## Fix

Fix at the right layer (debugging method §5): the generic `lb_store::list` now only filters
(`SELECT data FROM ... WHERE data.<field> = $value`, no ORDER BY). The **inbox** `list` verb —
which owns the `Item` shape and knows `ts` is the order key — sorts by `ts` in Rust after
fetch. Deterministic (the `ts` is caller-injected, testing §3), and the generic store verb
stays shape-agnostic.

- `rust/crates/store/src/list.rs` — dropped `ORDER BY`.
- `rust/crates/inbox/src/list.rs` — `items.sort_by_key(|i| i.ts)`.

## Verification

`cargo test -p lb-host --test messaging_test` — `history_survives_independent_of_the_bus`
asserts `["first","second","third"]` in order and passes; the original panic is gone.

## Prevention

The regression test asserts ordering at the channel layer, so a regression (re-introducing the
broken ORDER BY, or losing the sort) fails loudly. Guardrail: keep the generic store verbs
shape-agnostic — ordering/typing of `data` belongs to the crate that defines the record shape.
