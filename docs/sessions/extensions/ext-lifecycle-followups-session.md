# Session — the three #64 follow-ups: coherence gate, `ext.start`, multi-workspace boot

**Date:** 2026-07-15 · **Area:** extensions / lifecycle-management · **Follows:** [#64](https://github.com/NubeDev/lb/issues/64)
([`native-boot-respawn-session.md`](native-boot-respawn-session.md))

## The ask

Three gaps left open by the #64 fix — two surfaced by an adversarial review of it, one flagged in the
issue itself. All three are the **same failure shape as #64**: durable state that says an extension
should be running, and something that quietly doesn't act on it.

## A — an incoherent artifact published fine and stranded the extension at boot

`ext.publish` accepted an artifact whose `Artifact.ext_id`/`version` **contradicted the manifest they
carry**. The two are keyed apart and independently controlled: the **catalog** is addressed by the
artifact's copy (`CatalogEntry::of`), the **install record** by the manifest's (`Install::new`). And
`digest()` commits to exactly `(manifest_toml, wasm)` — it does **not** cover `ext_id`/`version` — so
`verify_artifact` cannot see a disagreement: those fields ride unsigned, and nothing reconciled them.

Publish succeeded; the next boot resolved the catalog by the *install record's* version, found
nothing, and reported the extension missing. Fail-closed, so no privilege consequence — but a silent
strand with a reason pointing at an evicted cache that was never the problem. Both tiers resolve this
way, so both were affected.

**Fix:** a third gate in `ext_publish`, `coherent()`, running **before** anything is stored (so the
verify-before-store guarantee extends to it). Validating the unsigned copies against the *already
signed* manifest — rather than extending the digest to cover them — is deliberate: it closes the gap
with no new trust, where a digest change would break every existing signed artifact for no extra
safety.

**A2 — the reason was also wrong.** Boot collapsed two different faults into `no-cached-artifact`.
They now report apart: `no-catalog-entry (looked for <ext>@<ver>)` (the records disagree) vs
`no-cached-bytes` (the real eviction). Different faults, different fixes; one name for both sent
operators to the wrong one.

## B — `ext.start`: nothing could start a stopped extension

The lifecycle had **no start verb at all**:

- `ext.enable` flips durable intent and spawns nothing — its own doc said "the boot reconciler / next
  start brings it up", a *start* that had no verb;
- `native.restart` / `native.reset` both need an existing handle in the `SidecarMap` → `NotRunning`;
- `reset`'s doc pointed the operator at "`ext.enable`/install to start a stopped extension" — **which
  does not do that.**

So every documented recovery path led somewhere that could not start anything, and republishing the
artifact was the only way back. That is *why* the #64 boot gap was so expensive: the workaround was
re-uploading a binary already on disk.

**Fix:** `ext.start` (`mcp:ext.start:call`, workspace-first) + `POST /extensions/{ext}/start`. It
reuses boot's exact path — the per-extension body of `spawn_enabled` factored out as `spawn_one` — so
"start this extension" means precisely what boot does for it, whoever asks. `enable`/`start` stay
distinct (the split `disable`/`stop` already has): a start **refuses a disabled extension** rather
than override the intent `disable` exists to express. Returns the outcome row, in the boot log's own
`spawned` + `reason` vocabulary, so an operator reads one language in both places. Both false doc
pointers (`reset`, `enable`) now name `ext.start`.

**Not wired into the `ext.*` MCP bridge** — `call_ext_tool` has no caller and `"ext."` is absent from
`HOST_NATIVE_PREFIXES`, so that whole surface is currently unreachable. Adding `start` there would
imply a reachability that does not exist; wiring it up is its own change, for every verb at once.
(Recorded in `start.rs` so the next person doesn't repeat the detour.)

## C — boot brought up only `cfg.workspace`

A node can serve many workspaces (`workspace.create` is a verb, the UI has a switcher), but both boot
halves were called with a single `ws`. Every other workspace's extensions stayed dead after a restart
— silently. Same shape as #64, one level up.

**Fix:** `boot_workspaces(store, boot_ws)` → `cfg.workspace` **∪** every **Active** registered
workspace, deduped. Both tiers loop it.

The union is load-bearing in **both** directions, and the revert-check proved it:
- **directory-only** would bring up *nothing* on a normal node — the boot workspace (`acme`, every
  test, an embedder provisioning its own identities) is never `workspace.create`d, so it has no row.
  Reverting the union yielded `["tenant-a"]`, with the real boot workspace missing.
- **no status filter** would spawn an **Archived** workspace's sidecars — resurrecting exactly the
  activity a soft-delete suppressed.

Precedent, not invention: `migrate_active_persona::known_workspaces` already unions the workspace
registry with the reactor directory, for the same reason neither alone is complete.

## Tests — 8 in `ext_boot_spawn_test.rs`, 8 in `ext_publish_test.rs`

New: `ext_start_brings_back_a_stopped_extension_and_it_answers` (publish → disable → **start refused
while disabled** → enable → **enable spawns nothing** → start → the child answers a real tool call,
with no node restart and no republish anywhere in the test), `ext_start_is_denied_without_the_grant_
and_nothing_spawns` (the mandatory capability-deny), `a_missing_catalog_entry_is_reported_distinctly_
from_evicted_bytes`, `boot_covers_every_active_workspace_not_just_the_configured_one`, and three
publish-coherence tests (version mismatch, ext_id mismatch, **plus the honest case still publishes** —
without which "reject everything" would pass).

**Every one revert-checked.** The start test fails at the "starts on demand" assert; both coherence
tests fail while the coherent-case stays green (proving the gate is precise, not blanket); both halves
of `boot_workspaces` fail independently with the reverts above.

### A real bug found in my own tests, not papered over

Adding the two `ext_start` tests made the whole binary die with **SIGTERM before any test reported**.
Not flakiness: 7 tests each booting a real SurrealDB node + spawning OS children, at libtest's default
of one thread per core (28 here). Bisected — each half passes alone at full parallelism, and all 7
pass at `--test-threads=4`. The file now caps **itself** with a shared `static` semaphore (3 slots,
held by the `Scratch` guard) rather than relying on a flag nobody will pass in a `--workspace` sweep.
Confirmed effective by timing (~28s bounded vs ~13s unbounded), not just by going green.

## Gates

`cargo fmt --check` clean; no new clippy warnings from these files (the two that fire are pre-existing).
`lb-host` lib 245/245; `ext_boot_spawn_test` 8/8, `ext_publish_test` 8/8, `ext_lifecycle_test` 4/4;
no regressions across `native_*`, `registry_*`, `install_record`, `hot_reload`, `admin_crud`, `authz`.

## Live verification

Verified in the running product, per the standing bar (a green suite proved nothing about #64):
publish a native sidecar, **stop it**, then start it over `POST /extensions/{ext}/start` — it comes
back with no node restart and no republish, and the polling e2e passes. See the PR notes.
