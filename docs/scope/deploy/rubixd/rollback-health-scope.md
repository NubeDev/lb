# rubixd scope — update transaction, health gate, rollback

Status: scope (the ask). Slice 4 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

The reason rubixd exists instead of a shell script: every install/update is a
**transaction** — download → verify → stage → swap → **health gate** → commit, and a
failed gate **rolls back automatically** to the previous version, marks the new version
bad for that instance, and reports honestly. Rollback is also a first-class operator
verb.

## Goals

- `transaction/` engine, backend-agnostic (drives the slice-3/5 `Backend` verbs):
  `run.rs` (the state machine), `health.rs`, `commit.rs`, `rollback.rs`, `marks.rs`.
- States, each persisted to the ledger *before* the effect (crash = resume or safe
  abort, never a half-known state): `Staged → Swapped → Gating → Committed |
  RolledBack | Failed`.
- **Health gate** per the package's `[health]` spec: `systemd-active` (unit active and
  not restart-looping for the grace period), `tcp:<port>` connect, `http:<url>` 2xx —
  each with `timeout` + `grace` (ignore failures for the first N seconds). Instance
  overrides in the bundle win over package defaults.
- **Auto-rollback** on gate failure: swap back to previous version (symlink flip /
  restore kept container), start, re-run the *same* health gate on the old version
  (rollback-that-also-fails → `Failed`, instance flagged red, loud), record
  `bad_versions += v` for that instance.
- **Bad-version marks**: reconcile/poller skip a marked version for that instance;
  cleared by `rubixd clear-bad <instance> <version>` (CLI + REST) or superseded by a
  newer resolvable version.
- Operator verbs: `rubixd rollback <instance>` (to previous kept version),
  `rubixd versions <instance>` (kept releases + marks) — CLI and
  `POST /api/instances/{name}/rollback`, both gated by the slice-2 bearer token.
- Crash recovery: on boot, any instance found mid-transaction is resolved (re-gate if
  `Swapped/Gating`, abort-and-restore if `Staged`).

## Non-goals

- No bundle-level (multi-package) transactions — per-instance only; a bundle reports
  partial results (parent scope's stated v1 line). No canary/percentage rollouts. No
  automatic *data* rollback — data dirs are never versioned by rubixd (that's the
  product's store's job); the guarantee is only that rubixd never touches them.

## Intent / approach

The transaction is a small explicit state machine over ledger writes, not a saga
framework. Health checks are pure probe functions polled on an interval; the gate is
`deadline > now && !healthy → keep polling`, one place. Serial per machine: one
transaction at a time (a global transaction lock in the ledger) — updates are rare and
serial keeps failure stories tellable. Alternative rejected: blue/green double-instance
switchover — needs port juggling the packages don't model; symlink-swap + rollback
covers the edge-box reality.

## How it fits the core

Durability translated: state persisted before effect (the outbox discipline). The deny
path: a rollback/clear-bad REST call without the token 401s; a rollback for an
instance with no kept previous version fails typed (`NothingToRollBackTo`), not
half-executed.

## Example flow

The parent scope's flow 4–6: `rubix-main` gates green → `Committed`; `rubix-lab`'s
0.4.6 fails `http` gate at 30 s → auto swap-back to 0.4.5 → old version gates green →
`RolledBack`, `0.4.6 ∈ bad_versions[rubix-lab]`, `rubixd status` shows it red-flagged;
poller skips 0.4.6 for rubix-lab until cleared/superseded.

## Testing plan

Real systemd (user scope) + a purpose-built tiny test binary (real HTTP server with a
`FAIL_HEALTH` env switch — a real program, not a mock of anything):

- gate pass → `Committed`, old release pruned to keep-N.
- gate fail → auto-rollback, old version active, `bad_versions` recorded, data-dir file
  written by v1 still present.
- rollback-also-fails (both versions forced unhealthy) → `Failed`, both attempts in
  ledger, instance red in status.
- crash mid-transaction (kill rubixd between swap and gate; restart) → resumes gate and
  lands in a terminal state.
- operator verbs: `rollback` flips a healthy instance back; `clear-bad` re-allows;
  both 401 without bearer.
- marks respected: reconcile with a marked version plans `noop`, not `update`.

## Risks & hard problems

- Flappy gates: `grace` + requiring K consecutive healthy probes before commit;
  defaults (grace 5 s, K=3, timeout 30 s) tunable per package.
- systemd restart-looping *looks* active — `systemd-active` must check
  `NRestarts`/state over the grace window, not one snapshot.
- Crash-recovery correctness is the hard 20% — the ledger write-before-effect
  discipline is non-negotiable in review.

## Open questions

- Should `Failed` (both versions down) attempt older kept releases automatically?
  Recommendation: no — page a human; auto-archaeology hides fires.

## Related

[`systemd-backend-scope.md`](systemd-backend-scope.md) /
[`docker-backend-scope.md`](docker-backend-scope.md) (the verbs this wraps) ·
[`bundles-scope.md`](bundles-scope.md) (the poller that respects marks).
