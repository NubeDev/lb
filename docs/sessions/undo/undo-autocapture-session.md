# Undo — auto-capture-on-dispatch (session)

- Date: 2026-06-28
- Scope: ../../scope/undo/undo-scope.md ("Intent": classification is runtime transaction taint)
- Stage: S10 (cross-cutting retrofit)
- Status: shipped & green

## Goal

Wire the host so **every mutating tool call is journaled automatically** at the dispatch seam, with
the reversible/irreversible class derived from **real runtime outbox taint** — not from manifest
metadata. The `lb-undo` mechanism + the host `undo`/`redo`/`history.*` verbs already shipped
(undo-build-session.md); until now nothing called `record_change`/`record_irreversible` for a normal
tool call — capture was manual. This slice closes that.

## What changed

### Runtime taint primitive (`lb-store`)

- New `crates/store/src/taint.rs` — a `tokio::task_local!` cell tracking, for the in-flight tool
  call, whether its transaction **reached the outbox** and whether it **wrote any store record**.
  `taint_scope(fut) -> (output, TaintVerdict)` opens the cell at the outermost call;
  `mark_outbox_reached()` / `mark_store_written()` set it; both are silent no-ops outside a scope.
  Because nested host-callback calls are `.await`ed on the **same task**, they share the enclosing
  cell — so a nested outbox reach taints the *enclosing* action (the composition `max` rule, enforced
  by scoping rather than a manifest field). `tokio` moved dev-dep → dep for the macro.
- Marks wired at the real seams: `mark_store_written()` in `write` + `write_tx`; `mark_outbox_reached()`
  in `lb_outbox::enqueue` (after a successful commit — a rolled-back tx does not taint). `enqueue`
  is the one chokepoint every outbox effect passes (host `enqueue_outbox` + the workflow job path).

### Post-hoc capture verb (`lb-undo`)

- New `crates/undo/src/record_captured.rs` — `record_captured`: journals a reversible single-record
  change that the **tool already applied** (the dispatch seam cannot re-apply). The before-image is
  snapshotted *before* the call; this reads the produced after-image/`rev` and writes the entry +
  pushes the stack. Distinct from `record_change` (which applies + journals in one tx). `group` is
  threaded through (grouped-undo groundwork).

### Dispatch-seam wiring (`lb-host`)

- New `crates/host/src/undo_capture/` (`plan.rs` + `capture.rs`). `plan_capture` classifies a call:
  **Reversible** (single-record `inbox.record` — the v1 floor, `(table,id)` derivable from args),
  **NonGeneric** (raw `store.query`, outbox verbs, arbitrary `<ext>.<tool>` — touched set unknowable),
  or **NotMutating** (reads — skipped). `capture_dispatch` wraps the real dispatch in `taint_scope`
  and journals: **taint wins** → `record_irreversible` (classified via `lb_undo::classify`);
  reversible+capturable → `record_captured`; non-generic but *wrote the store* → not-undoable marker;
  pure read → nothing. Journaling failures are swallowed (capture must never fail a good tool call).
- `tool_call.rs`: split the dispatch body into `dispatch_at_depth`; `call_tool_at_depth` wraps it with
  `capture_dispatch` **only at depth 0** (one tool call = one step; nested hops just contribute taint).
  `undo`/`redo`/`history.*` are exempt (they journal their own `kind:undo` entries — capturing them
  would double-journal/recurse). An optional `undo_group` input arg threads a batch/job group id.

## Decisions & alternatives (made solo — user away)

- **Taint lives in `lb-store`, set at the write/outbox seams.** Both the outbox crate (sets it) and
  the host (scopes + reads it) already depend on `lb-store`, and the taint is a property of the
  in-flight store transaction. *Rejected:* a separate `lb-taint` crate (needless), or a manifest
  `reversible` flag (the footgun the whole scope exists to prevent — a nested outbox call makes it a
  lie). *Rejected:* tainting inside `write_tx` keyed on "is the table the outbox" — `write_tx` is
  table-agnostic; `lb_outbox::enqueue` is the honest, single chokepoint.
- **A second `wrote_store` taint flag**, not just outbox. Without it the seam cannot tell a
  non-capturable *mutation* (mark not-undoable) from a pure read (don't journal) for an arbitrary
  extension tool — exactly the "non-generic → not-undoable, never partial" rule. *Rejected:* journal
  every non-read as not-undoable (would spam the stack with pure reads).
- **Capture at depth 0 only.** Nested host-callback calls share the scope and bubble taint up; only
  the outermost call is the user-facing "step". *Rejected:* capturing each hop (N entries per call,
  wrong granularity).
- **`record_captured` is best-effort, not atomic with the change.** The tool already committed its
  change before we journal; the entry is an *after* record. A crash between them leaves a committed
  change with no undo entry (it is simply not-undoable) — never an orphan entry for a change that did
  not land. That is the correct failure direction for dispatch capture (the atomic seam,
  `write_journaled`, remains available for callers that hand us the value up front).
- **v1 reversible floor = single-record `inbox.record`.** Generic before-image capture for an
  arbitrary tool needs per-tool touched-set knowledge the seam doesn't have; the scope explicitly
  makes `non-generic` not-undoable rather than partially captured. More reversible verbs are added by
  extending `plan.rs`'s allowlist — auditable in one place — not by guessing.

## Tests (real store/node, no mocks — rule #9)

```text
cargo test -p lb-store --test taint_test          # 4: untainted-clean, marked-observed,
                                                  #    NESTED-reach-taints-enclosing (max rule),
                                                  #    marks-outside-scope-are-no-ops
cargo test -p lb-host --test undo_autocapture_test # 4: reversible→auto-undoable,
                                                  #    outbox→auto-irreversible (taint not metadata),
                                                  #    capability-DENY → not journaled,
                                                  #    workspace-ISOLATION of the auto journal
```
Both green. The mandatory categories (capability-deny, workspace-isolation) are covered in the host
suite. The composition `max` rule (nested outbox reach taints the whole) is proven at the taint unit
level against the real task-local mechanism. `cargo test --workspace` green, `cargo fmt` clean,
`cargo clippy` clean on the new files.

## Follow-ups (scope-decided, not this slice)

- **Full grouped undo** (reverse-order, all-or-nothing, refuse if any step irreversible) — the
  `group` id is now threaded end to end (`undo_group` arg → `record_captured`/`record_irreversible`);
  the reversal logic over a group is the remaining piece.
- **Reversible capture beyond the single-record floor** — declared touched-set/inverse for
  multi-record/derived-state tools; more entries in the `plan.rs` allowlist.
- **Manifest `compensation` field** (additive WIT change) → `declared_compensation` is already plumbed
  into `capture_dispatch` (passed `None` until the field exists), so a compensable class lands the day
  the manifest carries it.
