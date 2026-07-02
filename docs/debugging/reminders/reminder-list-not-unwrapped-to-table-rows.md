# `/reminders` table renders ONE JSON-blob row, not N reminders (`reminder.list` plural not in ROW_KEYS)

Status: **resolved**.

**Symptom (real-gateway list render):** the channel `/reminders` interactive table rendered a **single**
row whose one cell held the whole reminders JSON array (header `reminders` + `actions`), instead of **N**
rows (one per reminder). Because there was no per-reminder row, each row control's `${id}` bound
nothing, so the rendered pause switch drove no real reminder (`enabled` stayed `true`). The
DOM-driven interactive list was inert even though the descriptor render, the posted envelope, and the
control arg-building were all correct.

## Root cause

A `rich_result` table `source`-d at `reminder.list` resolves through `viz.query`, whose **row-unwrap**
turns a tool result into table rows. That unwrap looks for the rows under a fixed set of plural keys —
and `reminder.list` returns `{reminders:[…]}`, whose key `reminders` was **not in the set**:

- host: `rust/crates/host/src/viz/frame.rs` — `ROW_KEYS = ["samples","items","rows","templates","dashboards"]`
- client mirror: `ui/src/features/dashboard/builder/useSource.ts::toRows` — the same list

With `reminders` absent, `toRows`/`result_to_rows` fell through to "a single object" and returned
`[{reminders:[…]}]` — one row. (Other list verbs work because their plural is already listed:
`dashboard.list`→`dashboards`, `render.templates`→`templates`, series→`samples`.)

Surfaced only by the real-gateway list render — the Rust reminder tests read `{reminders}` directly (they
never route through `viz.query`'s row-unwrap), and the UI unit tests assert the envelope shape, not the
mounted DOM.

## Fix

Add `"reminders"` to both mirrored `ROW_KEYS` lists (host + client). Additive and consistent with the
existing per-verb plurals; the locked render contract (`source:{tool:"reminder.list"}`) is unchanged.

```rust
// viz/frame.rs
const ROW_KEYS: &[&str] = &["samples","items","rows","templates","dashboards","reminders"];
```
```ts
// useSource.ts toRows
for (const k of ["samples","items","rows","templates","dashboards","reminders"]) { … }
```

Now `reminder.list`'s `{reminders:[r1,r2,…]}` unwraps to N rows; each row is a real reminder object, so a
row control's `${id}` binds the row's id and drives the real write verb.

## Regression test

`viz/frame.rs` unit: `result_to_rows({"reminders":[{…},{…}]})` returns the 2 reminder rows (not a
one-element `[{reminders:[…]}]`) — the host-side parity. UI: the `useSource.toRows` mirror unwraps
`{reminders:[…]}` to N rows. (The real-gateway list-render + DOM-control assertion is unblocked by this
fix — the interactive list renders N rows and a control's `${id}` binds.)
