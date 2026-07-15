# rules: pause/cancel aborts classified as author errors — the abort token is not in the error's Display

- **Date:** 2026-07-15 · **Area:** rules (long-running runs) · **Status:** fixed + regression-tested

## Symptom

`rules.runs.suspend`/`cancel` on a live run aborted the eval, but the engine classified the abort
as `RuleError::Eval("Script terminated (line 1, position 1)")` instead of `Paused`/`Cancelled` —
so the worker would have recorded a *failure* where the operator asked for a *pause*.
`longrun_test::pause_request_aborts_and_maps_to_paused` failed with exactly that `Eval` value.

## Root cause

The cage's `on_progress` governor aborts a run by returning `Some(token)`; rhai wraps it as
`EvalAltResult::ErrorTerminated(token, pos)`. The engine's `map_eval_error` matched on
`e.to_string()` — but **`ErrorTerminated`'s `Display` does not include the token payload**, only
the fixed text "Script terminated". String-matching the message can never see the token.

## Fix

`map_eval_error` (crates/rules/src/engine.rs) now matches the **variant** and downcasts the token
`Dynamic` to a string before any message-text classification:

```rust
if let rhai::EvalAltResult::ErrorTerminated(token, _) = &e {
    match token.clone().into_string().as_deref() {
        Ok(ABORT_PAUSED) => return RuleError::Paused,
        Ok(ABORT_CANCELLED) => return RuleError::Cancelled,
        _ => {} // the deadline token stays an Eval (author feedback, unchanged)
    }
}
```

## Regression tests

`crates/rules/tests/longrun_test.rs::pause_request_aborts_and_maps_to_paused` and
`::cancel_request_aborts_and_outranks_pause` (fail-before / pass-after), plus the host-level
`rules_longrun_test.rs` suspend/cancel suites which depend on the typed mapping end to end.

## Lesson

An rhai abort token is *data on the variant*, not part of the rendered message — any consumer of
`on_progress` aborts must match `ErrorTerminated` structurally. (The pre-existing time-budget path
happened to work only because it never needed to distinguish its token.)
