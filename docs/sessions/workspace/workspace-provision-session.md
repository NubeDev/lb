# Session — workspace.provision + workspace.reconcile (atomic provision, lb#121)

Date: 2026-07-30. Scope: `docs/scope/workspace/workspace-provision-scope.md`. Issue: NubeDev/lb#121.
Consumer: NubeIO/rubix-ai#64 (new-workspace wizard, built in the same session against a local
`[patch]`).

## Blocking questions, answered first

1. **Does `lb_store` expose a multi-row batched append with an explicit flush point?**
   **Batch: YES — flush: NO.** `lb_store::write_batch` (store/src/write_batch.rs) applies up to 256
   upserts+deletes in ONE SurrealDB `BEGIN…COMMIT` transaction — but only within ONE namespace, and
   there is no exposed flush/fsync anywhere in the store crate (durability is SurrealKV's implicit
   per-transaction commit; surrealdb 2.x exposes no inner kv handle — the same limitation that forced
   compaction out-of-band). Since the directory row lives in `_lb_workspaces` and the bootstrap rows
   live in the target namespace, they **cannot share a transaction**. The delivered guarantee is
   therefore honestly: **atomic in-namespace bootstrap (one `write_batch`) + directory row written
   LAST + reconcile** — not one cross-namespace transaction, and not an explicit flush. This removes
   the observed orphan class (listable-but-memberless) entirely; the one remaining torn intermediate
   (bootstrap landed, directory row lost) is invisible, harmless, and retryable (see carve-out
   below). The missing flush primitive is flagged in `../../scope/store/store-scope.md` open
   questions.
2. **Reconcile gating:** `mcp:workspace.reconcile:call`, bundled into `ADMIN_ONLY_CAPS` (so
   `role:workspace-admin`) like provision. Super-admin-only (scope OQ4) stays open — `super-admin`
   is still a reserved, unseeded role, so there is nothing to gate on yet. The hard limit is
   enforced instead: reconcile refuses any workspace with ≥1 live member.

## What shipped

- `host/src/workspaces/bootstrap.rs` — the shared write-set builder (role records seed-if-absent,
  membership, admin role grants, skill edges write-if-absent so a revoked edge never resurrects);
  `apply_bootstrap` = one `write_batch`. Row builders added upstream of host so batched rows cannot
  drift from the single-write verbs: `lb_authz::grant_row`, `lb_authz::membership_row`,
  `lb_assets::relation_row`.
- `host/src/workspaces/provision.rs` — `workspace_provision`: authorize against the CALLER's
  workspace, tombstone→`Purged`, existing-active→idempotent no-op report, then bootstrap batch, then
  directory row LAST. `ProvisionFailed { stage }` (`plan|bootstrap|directory`). Carve-out: a
  directory-less namespace whose ONLY live member equals the requested admin is a torn provision's
  own residue and may be re-completed; any other populated namespace is refused (that would grant
  admin into a populated workspace — `workspace.adopt` territory, still open as scope OQ3).
- `host/src/workspaces/reconcile.rs` — `workspace_reconcile`: directory row must exist, tombstone
  refused, **any live member refused**; re-runs the shared bootstrap, reports `fixed`.
- `create.rs` shrunk to a thin delegation (admin = caller, default skills); **all `let _ =`
  best-effort writes deleted**. Behaviour delta (deliberate): re-creating an existing active
  workspace is now a pure no-op returning the current record — it no longer upserts name/ts (rename
  owns that) and no longer bootstraps a non-member caller into an existing workspace (which was a
  quiet escalation: any `workspace.create` holder could self-admin any listed workspace by
  "re-creating" it).
- Caps: `mcp:workspace.provision:call` + `mcp:workspace.reconcile:call` in `ADMIN_ONLY_CAPS`.
- MCP arms in `workspaces/tool.rs`; gateway routes `POST /workspaces/{ws}/provision` +
  `/reconcile` (`role/gateway/src/routes/workspace_provision.rs`; 403/422/409/500 typed mapping).
- No credential/email/password anywhere (invites owns that seam).

## Tests (all real store, no mocks)

`host/tests/workspace_provision_test.rs` (8): cap-deny with zero residue; torn-intermediate never
listable + retry completes it + foreign-populated namespace refused; **crash durability** (on-disk
SurrealKV, drop without clean shutdown, reopen ⇒ listable AND enterable — the regression test for
the observed `nube` orphan); hand-crafted orphan → roster excludes → reconcile → roster includes,
and reconcile refused on a populated ws with zero residue for the caller; admin-other-than-caller
(caller not membered, session ws unchanged); idempotent re-provision does NOT re-grant a revoked
role; purged tombstone never resurrected (provision AND reconcile); create-parity.
`role/gateway/tests/workspace_provision_route_test.rs` (3): report reply carries **no token**;
member 403 with zero residue; reconcile over HTTP repairs an orphan.

Note on the scope's "inject a failure at each bootstrap stage": with the bootstrap collapsed into
one transaction there are no per-stage intermediates left to inject between — the only torn point is
between the batch and the directory write, which is exactly the state the torn-intermediate test
hand-crafts and proves harmless. The scope's stage-injection list described the old five-write shape.

`cargo fmt --all --check` clean; clippy clean on all touched files (a pre-existing, unrelated
`lb-frame` `clippy::min_max` error exists on master — not from this session).

## Release

Needs a `node-v*` tag before rubix-ai can bump its pin (currently `node-v0.4.5`); the rubix-ai
wizard was built and tested against the live local `[patch]` (which already carries two other
unreleased slices — the next tag carries all three).
