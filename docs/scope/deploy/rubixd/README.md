# rubixd — scope index + coding-session roadmap

`rubixd` is the per-machine install/update agent of the fleet package plane. Parent
scope: [`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md) — read it first;
it owns the shared decisions (out-of-tree `rubix-fleet` repo, signed-artifact envelope,
package model, bundle YAML contract). The docs here decompose rubixd into codeable
slices, one scope per slice, sized for one long-running AI session each.

**Code home**: the `rubix-fleet` repo (`crates/rubixd/`, shared `crates/fleet-spec/`,
`crates/fleet-auth/`). These docs live in lb because lb is the architecture home and the
conventions being reused are lb's; sessions/debugging entries follow the lb rules
(`docs/HOW-TO-CODE.md`, `docs/scope/testing/testing-scope.md`).

## Slices

| # | Scope | What ships |
|---|---|---|
| 1 | [`agent-core-scope.md`](agent-core-scope.md) | Crate skeleton, config, installed-state ledger, reconcile loop shell, CLI verbs |
| 2 | [`token-auth-scope.md`](token-auth-scope.md) | Boot-generated admin token, one-time UI claim, Bearer auth on every REST route |
| 3 | [`systemd-backend-scope.md`](systemd-backend-scope.md) | Release dirs + `current` symlink, unit/env generation, `service-manager` driver |
| 4 | [`rollback-health-scope.md`](rollback-health-scope.md) | The update transaction: stage → swap → health gate → commit / auto-rollback, bad-version marks |
| 5 | [`docker-backend-scope.md`](docker-backend-scope.md) | `bollard` driver: pull/load, labeled containers, recreate-with-keep, volume safety |
| 6 | [`bundles-scope.md`](bundles-scope.md) | Bundle YAML apply/validate, multi-instance, `needs` ordering, `${secret:...}` resolution, rartifacts poller |
| 7 | [`embedded-ui-scope.md`](embedded-ui-scope.md) | Small Bootstrap UI (rust-embed, no build step): claim page, status, instance detail, rollback button |

## Coding-session roadmap (long-running AI sessions)

One session per slice, in order — each slice's exit gate is real and green before the
next starts. Every session follows `docs/HOW-TO-CODE.md`: read the slice scope + this
README, build, test for real (no mocks — real `systemd --user`, real dockerd, real
HTTP), write `docs/sessions/deploy/rubixd-<slice>-session.md`, log any breakage under
`docs/debugging/deploy/`, update `docs/STATUS.md`.

1. **Session 1 — agent core.** Scaffold `rubix-fleet` (workspace, `fleet-spec` types,
   `rubixd` binary). Config load (`/etc/rubixd/config.toml`), embedded-SurrealDB ledger
   (`state/`), reconcile loop that diffs desired-vs-installed and logs planned
   transitions (no backends yet), CLI: `status`, `reconcile`, `apply` (parse+persist
   only). **Exit gate**: `apply` of a bundle YAML persists desired state; `status`
   renders it; ledger survives restart; `cargo test` green.
2. **Session 2 — token + REST auth.** `fleet-auth` crate + the local axum server:
   `POST /api/claim` (one-time), Bearer extractor on all other routes, `--reset-token`.
   **Exit gate**: claim returns the token exactly once (second call 410); unauthenticated
   REST 401; authenticated `GET /api/status` 200 — all against the real server.
3. **Session 3 — systemd backend.** The `backend/systemd/` folder-of-verbs against
   real user-scope systemd. **Exit gate**: install→active, update→symlink flipped +
   active, uninstall clean; ownership test (won't touch a foreign unit) green.
4. **Session 4 — rollback + health.** The transaction engine + health checks
   (`systemd-active` | `tcp` | `http`), bad-version marks, `rollback` CLI/REST verb.
   **Exit gate**: a v-next that fails its health gate auto-rolls-back to v-prev (real
   binary, real unit) and the data dir is untouched; bad mark prevents re-try until
   cleared.
5. **Session 5 — docker backend.** `backend/docker/` via bollard. **Exit gate**: same
   transaction suite as session 4 but containerized (image archive load, recreate,
   rollback restores old container, volume survives); suite skips-and-reports when no
   dockerd.
6. **Session 6 — bundles end to end.** Multi-instance validation (`per_instance` key
   collisions), `needs` ordering, secrets file resolution, the rartifacts update poller
   (channel/range re-resolve). **Exit gate**: the parent scope's example flow (two
   rubix-ai instances, one rolls back) passes against a real rartifacts instance.
7. **Session 7 — embedded UI.** Bootstrap pages over the existing REST surface only.
   **Exit gate**: claim → dashboard → instance rollback all work in a browser against
   a live rubixd; no new server verbs added for the UI.

Renumber nothing: later slices (fleet control plane, Windows service backend, rubixd
self-update hardening) get new scope files here when asked for.
