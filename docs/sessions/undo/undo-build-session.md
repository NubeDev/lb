# Undo journal — build session (store `rev` + `lb-undo` + host verbs)

- Date: 2026-06-28
- Scope: ../../scope/undo/undo-scope.md
- Stage: S10 (cross-cutting retrofit), per STAGES.md
- Status: core shipped end-to-end (capture → conditional restore → host MCP verbs), green

## Goal

Build the undo scope into shipped code + tests in one session, bottom-up, prioritising the
load-bearing floors the scope itself flags: (1) a **store-managed monotonic `rev`** (the
optimistic-concurrency token the scope names as an in-scope prerequisite), and (2) the
**conditional restore** enforced against that `rev` (the correctness core — a stale undo is
*refused*, never a forced LWW clobber). Then the `lb-undo` crate's journal/classification, and the
capability-gated host MCP verbs (`undo`/`redo`/`history.list`/`history.compensations`).

## What changed

### Store seam — the `rev` foundation (shared, touches the shipped store contract)

- New record envelope field `rev` (`crates/store/src/record.rs`): a store-managed monotonic
  revision, stamped server-side on every write. **Forward-compatible:** the plain `read` path still
  returns only the host `data`; legacy rows default to `rev = 1`.
- `write` / `write_tx` now bump `rev` inside the same UPSERT statement (server-side, never a racy
  read-modify-write): `rev: (type::thing($tb,$id).rev ?? ($first-1)) + 1`.
- New `read_versioned` (`Versioned { value, rev }`, absence = rev 0) — the unit the conditional
  predicate works on; absence is first-class (create-undo asserts "still absent", delete-undo
  restores from absence).
- New `write_journaled` — the atomic before-image seam (the undo analogue of `write_tx`): a domain
  change + its journal entry commit in ONE transaction.
- Probe-first de-risking: `crates/store/tests/rev_probe_test.rs` validated the rev SurrealQL before
  anything was built on it (see the debugging entry below for the one iteration it took).

### `lb-undo` crate (new) — the journal mechanism

- `model.rs` — `JournalEntry` (immutable event: before/after/per-record `rev`/group/kind/class),
  `StackState` (mutable per-(ws,actor[,surface]) cursor: undoable/redoable seqs), `Class`
  (Reversible | Irreversible | Compensable) with the `combine` **max-composition** rule.
- `record_change` — reversible `do`: snapshot before-image, apply + journal atomically via
  `write_journaled`, record the produced `rev`, push onto the stack (truncating redo).
- `record_irreversible` — a not-undoable marker (no before-image) so history is complete and a
  grouped undo can refuse up front.
- `restore.rs` — the **conditional restore**: a one-transaction guarded write that re-asserts every
  touched record's expected `rev` (THROW → rollback on mismatch) → `UndoError::Stale`, never a
  clobber. Returns the produced revs so the inverse op (redo↔undo) has its predicate.
- `apply_undo` / `apply_redo` — peek → conditional restore → journal the undo/redo → move cursor.
- A `undo_live:{seq}` companion holds the **live predicate revs**, updated each undo/redo cycle, so
  repeated undo↔redo cycles guard against the *actual* current rev (not a stale capture-time rev).
- `classify.rs` — runtime-taint → class (reached-outbox ⇒ irreversible, derived not trusted; a
  declared compensation only *adds* a handle, never downgrades).
- `peek.rs` — peek the next undo/redo target so the host can run the no-escalation cap check first.

### Host layer — the capability-gated MCP surface

- `crates/host/src/undo/` — `undo`/`redo`/`history_list`/`history_compensations`, each gating
  `mcp:<verb>:call`, plus the **no-escalation** check (caller must hold the original tool's cap) and
  **`undo.any`** for another actor's stack. The surfaced refusals (`Stale`, `NotUndoable` + any
  compensation) are distinct from opaque `Denied`.
- Wired into `tool_call.rs` dispatch (`undo`/`redo`/`history.*` host-native), returning UI-shaped
  JSON outcomes (`ok:false, reason:"stale"|"not_undoable"|"empty"`).

## Decisions made (no questions asked — user was away)

- **`rev` is a monotonic per-record counter**, not a content hash — true monotonicity is needed to
  distinguish A→B→A, which a hash cannot; the scope allowed either.
- **Single-record capture in `record_change` for v1**; the model already carries a `group` and
  `touched: Vec<…>` so multi-record/grouped undo is a forward extension, not a redesign.
- **Live predicate revs in a side record** (`undo_live`) rather than mutating the "immutable" entry —
  keeps the audit-stable entry immutable while the predicate state evolves across cycles.
- **Deletes/creates via `None` value semantics** through the same upsert seam (record-as-absent), per
  the scope's "instrumented before-image" floor.

## Tests (all green — real store/node, no mocks per rule #9)

- `crates/store/tests/rev_probe_test.rs` — rev monotonic + per-record.
- `crates/undo/tests/undo_test.rs` (9) — reversible round-trip; create→delete-to-absence; **stale
  undo refused, intervening write survives**; workspace-wall; irreversible refused; compensable
  surfaces its compensation; redo-truncation; capture bumps rev; classification max-composition.
- `crates/host/tests/undo_test.rs` (5) — **capability-deny**, **no-escalation**, **undo.any**,
  **workspace-isolation**, round-trip over the gate (the mandatory host-surface categories).
- `cargo test --workspace` — 175 test binaries green; `cargo fmt --check` clean; `cargo clippy`
  clean on the new crates.

## Debugging

- `docs/debugging/store/rev-subquery-always-returns-first.md` — the rev bump silently stayed at 1
  (a `SELECT VALUE … FROM ONLY … [0]` scalar-vs-array SurrealQL footgun); fixed with scalar field
  access; regression-tested by the rev probe.
- One **flaky** `offline_sync_test` failure appeared under full parallel `--workspace` load and did
  not reproduce in isolation (5/5) or with instrumentation — a pre-existing bus subscription-vs-publish
  timing race, **not** a rev regression (no record errors; the rev path is functionally untouched by
  channel sync). Noted, not "fixed", to avoid masking it; worth a separate readiness-barrier fix.

## Follow-ups (decided in the scope; not built this session)

- Grouped/transaction undo (reverse-order, all-or-nothing, refuse if any step irreversible) — the
  `group` field + `touched` vec are in place to carry it.
- The host **capture wiring**: classify at dispatch from real outbox taint and call
  `record_change`/`record_irreversible` automatically for every mutating tool (this session ships the
  mechanism + verbs; auto-capture-on-dispatch is the next slice).
- A manifest `compensation` field (additive WIT change) when an extension first needs to declare one.
- File/blob undo via record-as-content versions (buckets degraded on the shipped engine).
- The `rev` stamp is now a shared store-contract change — audit/observability ride the same seam and
  can reuse it.
