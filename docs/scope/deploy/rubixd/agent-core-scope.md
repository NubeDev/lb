# rubixd scope — agent core (skeleton, ledger, reconcile shell)

Status: scope (the ask). Slice 1 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

The foundation every other slice builds on: the `rubix-fleet` workspace, the shared
`fleet-spec` types (package/bundle/artifact + digest/signing), rubixd's config, its
**installed-state ledger** (embedded SurrealDB), and a **reconcile loop** that diffs
desired state (applied bundles) against installed state and plans transitions — with
backends stubbed as a trait, not yet implemented.

## Goals

- `rubix-fleet` cargo workspace: `crates/fleet-spec`, `crates/fleet-auth` (empty shell,
  slice 2 fills it), `crates/rubixd`, `crates/rartifacts` (shell). FILE-LAYOUT rules
  apply from the first file (≤400 lines hard, one verb per file).
- `fleet-spec`: the `Package` metadata model (kinds `systemd | docker-image |
  docker-archive | bundle`; `[config]` schema with `per_instance`; `[health]`;
  `[preserve]`), the `Bundle` YAML model (serde), semver/channel version specs, and the
  digest/signing module (Ed25519 over length-prefixed SHA-256 — port the convention
  from lb `rust/crates/registry/src/digest.rs`, do not depend on the lb crate).
- rubixd config: `/etc/rubixd/config.toml` (override `RUBIXD_CONFIG`) — **any number
  of `[[remote]]` entries** (each: `name`, `url`, optional `token_path` — an agent
  token registered on that rartifacts; omitted = anonymous, public packages only,
  plus per-remote `trusted_pubkeys`), poll interval, bind addr (default
  `127.0.0.1:9420`), data root (default `/var/lib/rubixd`). Remote *names* are what
  bundles reference (`remote: <name>`); config validation rejects duplicates.
- **Installed-state ledger** in embedded SurrealDB at `<data>/state/`: one record per
  instance — package, version, backend, release path / image id, health, kept previous
  versions, bad-version marks, last transition + error. Ledger is the *only* durable
  memory; everything else is re-derivable.
- **Reconcile loop**: `desired (bundles.d) − installed (ledger) → plan[Transition]`.
  Transitions execute through a `Backend` trait (`install/update/remove/status`); this
  slice ships the trait + a `plan-only` mode that logs the plan.
- CLI (clap): `rubixd status`, `rubixd apply <bundle.yaml>` (validate + persist to
  `/etc/rubixd/bundles.d/`), `rubixd reconcile [--plan]`, `rubixd version`.

## Non-goals

- No backend execution (slices 3/5), no rollback engine (4), no HTTP server or auth
  (2), no UI (7), no rartifacts network calls (6 — the poller; this slice resolves
  desired state from local YAMLs only).

## Intent / approach

- One binary, daemon mode (`rubixd run`: reconcile loop + poll timer) and one-shot CLI
  verbs sharing the same library core: `crates/rubixd/src/{config/, ledger/,
  reconcile/, backend/mod.rs (trait), cli/}`.
- Plan is a pure function: `fn plan(desired: &[InstanceSpec], installed:
  &[InstanceState]) -> Vec<Transition>` — trivially unit-testable, no I/O.
- Bundle *validation* here is structural only (parse, unique instance names,
  `per_instance` collisions); trust/health semantics land with their slices.
- Alternative rejected: JSON state file instead of embedded SurrealDB — the ledger
  needs concurrent read (status/UI) while a transition writes, crash-consistent
  history, and queries; and house consistency says SurrealDB (rule 2 spirit).

## How it fits the core

Not an lb node: tenancy/caps/bus/MCP N/A (parent scope records the translation —
ownership + trust are the walls). SurrealDB embedded = house datastore. One
responsibility per file enforced from day one.

## Example flow

1. `rubixd apply site-alpha.yaml` → validated, copied to `bundles.d/`, desired state
   loaded.
2. `rubixd reconcile --plan` → prints `install timescaledb/tsdb-main (docker-image)`,
   `install rubix-ai/rubix-main (systemd)` in `needs` order — executes nothing.
3. Restart rubixd → ledger + bundles reload; plan is identical (idempotent).

## Testing plan

- Unit: plan function (install/update/noop/remove/ordering cases); bundle validation
  (dup instance, colliding `per_instance` port); version-spec resolution (exact, range,
  channel *shape*).
- Integration (real embedded store): apply → restart → status identical; ledger
  transition write + concurrent read.
- Deny/isolation (mandatory categories, translated): a malformed bundle is rejected
  with a precise error and desired state is unchanged; `apply` refuses a bundle whose
  name collides with an existing one unless `--replace`.

## Risks & hard problems

- Getting the `Transition` vocabulary right now (install/update/remove/rollback/
  quarantine) — the rollback slice extends it, so keep it an enum in one file.
- Semver-vs-channel resolution split: local plan resolves ranges only against ledger
  knowledge; network resolution is slice 6 — don't smear it in early.

## Open questions

- `rubixd run` as the systemd-managed daemon: unit file authored here or in slice 3
  (which owns unit generation)? Recommendation: here, hand-written, as
  `packaging/rubixd.service`.

## Related

[`README.md`](README.md) roadmap · parent scope · lb `registry/src/digest.rs` (the
ported convention) · [`token-auth-scope.md`](token-auth-scope.md) (next slice).
