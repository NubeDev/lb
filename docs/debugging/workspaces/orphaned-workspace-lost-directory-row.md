# A created workspace became permanently unreachable after a restart (`not a member of that workspace`)

Date: 2026-07-30 · Area: workspaces / store · Status: **fixed** (lb#121)

## Symptom

A rubix-ai setup wizard created workspace `nube` via `workspace_create`; the node restarted before
the segment was durable. Afterwards the store's namespace index still carried `/!nsnube`, but the
`_lb_workspaces/workspace/nube` directory row was gone. `login_workspaces` iterates that directory,
so every attempt to enter returned a permanent `not a member of that workspace` — and because an
unlisted workspace is invisible to `workspace.archive`/`purge`, there was no route to list, repair,
or delete it. The verb had returned `Ok(record)`, so the caller was told it succeeded.

## Root cause

`workspace_create` was four independent best-effort writes behind one verb: directory row FIRST,
then membership, role seeds/grants, and skill grants — each error discarded (`let _ =`). Any
interruption after the directory write (or, as observed, loss of a later-durable early write)
produced a workspace that exists, has no members, and is unreachable by anyone. The doc comment
promised "never orphaned"; the code could not keep it. Directory-row-first is specifically why the
orphan was *listable-then-lost* rather than simply absent.

## Fix

`workspace.provision` (+ `workspace.reconcile`, and `workspace_create` as a thin delegation):
the in-namespace bootstrap (membership, role records, grants, skills) applies as ONE
`lb_store::write_batch` transaction, and the directory row is written **LAST** — a torn provision is
now invisible and retryable, never a listable-but-memberless orphan. Pre-existing orphans are
repairable with `workspace.reconcile` (strictly limited to memberless workspaces). The `let _ =`
best-effort writes are deleted. Note: `lb_store` still exposes no explicit flush point (flagged in
`../../scope/store/store-scope.md`); durability is the engine's per-transaction commit.

## Regression tests

`rust/crates/host/tests/workspace_provision_test.rs` —
`provision_survives_unclean_restart_listable_and_enterable` (real on-disk SurrealKV store, dropped
without clean shutdown, reopened ⇒ listable AND enterable),
`torn_provision_is_never_listable_and_retry_completes_it`, and
`reconcile_repairs_a_memberless_orphan_but_never_a_populated_workspace`.

## Lessons

- **`let _ = <bootstrap write>` behind an `Ok` is a promise generator** — the verb reports a state
  it never verified. If the bootstrap is part of the contract, it must be part of the failure domain.
- **Write ordering is a durability tool:** when two writes can't share a transaction (different
  namespaces), write the *discoverability* row last so a torn state is invisible rather than
  half-visible.
- A repair verb for the broken state class must ship WITH the fix — the old path's orphans predate it.
