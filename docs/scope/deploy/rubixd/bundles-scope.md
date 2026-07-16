# rubixd scope — bundles end to end (apply, multi-instance, secrets, poller)

Status: scope (the ask). Slice 6 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md) (owns the bundle YAML
contract — the worked example lives there).

Everything between a bundle YAML and a running machine: full validation
(multi-instance, `per_instance` collisions, `needs` ordering), `${secret:...}`
resolution, the **artifact fetcher** (download from rartifacts, verify, cache), and the
**update poller** that re-resolves channel/range pins on an interval. Exit of this
slice = the parent scope's example flow passes for real.

## Goals

- `bundle/` verbs: `validate.rs` (schema + cross-instance uniqueness + `needs` graph
  is a DAG referencing declared instances), `order.rs` (topo sort; deterministic
  tie-break by name), `secrets.rs`, `diff.rs` (bundle re-apply → per-instance
  add/change/remove set).
- **Secrets**: `${secret:<path>}` resolves from `/etc/rubixd/secrets.toml` (root,
  0600) at *render* time — resolved values go into env files/container env, never into
  the ledger, logs, status output, or the REST/UI surface (redacted as `•••`).
  Missing secret = validation error at apply, not a boot-time surprise.
- **Fetcher** (`fetch/`): resolve `(name, version-spec, arch)` across the configured
  **remotes** — a package pinning `remote: <name>` resolves only there; otherwise
  remotes are tried in config order, **first match wins** (deterministic; later
  remotes are never consulted once one answers). Download blob by digest with HTTP
  Range resume into `/var/cache/rubixd/blobs/<sha256>` (content-addressed — shared
  across instances, retries, *and remotes*), verify SHA-256 + Ed25519 against that
  remote's `trusted_pubkeys` before the blob is ever handed to a backend; cache
  pruned LRU above a size cap.
- **Per-remote auth**: requests carry that remote's agent token when configured;
  a token-less remote can still serve **public** packages (anonymous). A `401` from
  a remote (agent revoked server-side) is surfaced as `remote <name>: access revoked`
  in `status` — running instances are untouched, only updates stop.
- **Poller**: every `poll_interval` (default 15 m, jittered), re-resolve every
  channel/range-pinned instance; a higher resolvable version (not bad-marked for that
  instance) enqueues an update transaction; exact pins never move. Offline rartifacts
  → log, keep running, retry with backoff — running services never degrade because
  the artifact server is away.
- Bundle lifecycle verbs: `rubixd apply` (now executes, health-gated, in `needs`
  order), `rubixd remove-bundle <name>` (tears down its instances, data dirs kept),
  `GET /api/bundles` + `POST /api/bundles/apply` on the REST surface.

## Non-goals

- No fleet push (server-initiated) — pull-only v1. No bundle-level transactions
  (parent scope line: per-package + ordering; partial results reported honestly). No
  secret *distribution* — the secrets file arrives on the box out of band.

## Intent / approach

`needs` gates on the slice-4 health verdict: a dependent's install/update starts only
after its dependency's gate is green (fresh probe if already running). Re-apply diffs
rather than recreates: only changed instances transition (changed env/ports/version →
update; removed → remove). Alternative rejected: cron-shaped "check script" —
the poller must respect bad-version marks, jitter, and backoff, which is reconcile-loop
logic, not a crontab.

## How it fits the core

Trust wall (verify-before-execute) and secrets posture (values never at rest outside
the 0600 file, never in logs) are the two hard lines. Sync/authority: rartifacts owns
packages, rubixd owns its machine; disconnected = degrade updates, never services.

## Example flow

The parent scope's §Example flow, now executable end to end: publish 0.4.6 → poller
resolves `stable` → fetch once (two instances share the blob) → serial per-instance
transactions in `needs` order → `rubix-main` commits, `rubix-lab` auto-rolls-back →
`rubixd status` + `GET /api/status` tell the truth.

## Testing plan

Real rartifacts (spun up in-test from the sibling crate), real systemd/dockerd:

- validation: colliding `per_instance` port across instances rejected; `needs` cycle
  rejected; unknown `needs` target rejected; missing secret rejected — each with a
  precise error, desired state untouched.
- secrets: rendered env file contains the value; ledger/status/REST/log capture
  contains only redaction (grep the artifacts).
- fetcher: download → verify → cache hit on second instance (one network fetch,
  asserted); tampered blob refused; interrupted download resumes (kill mid-stream).
- poller: channel promote on the real rartifacts → update enqueued; bad-marked version
  skipped; exact pin never updates; rartifacts down → backoff logged, instances
  untouched.
- multi-remote: two real rartifacts instances — `remote:` pin resolves only there;
  unpinned package present on both resolves from the first in config order (asserted
  by digest); same digest from either remote = one cache entry.
- auth tiers: public package fetches anonymously from a token-less remote; private
  package on that remote → 401 surfaced in `status`; agent revoked mid-flight
  (server-side revoke on the real rartifacts) → next poll reports `access revoked`,
  running instances untouched.
- E2E: the full two-instance example flow, both backends, green.

## Risks & hard problems

- Ordering vs long gates: a slow dependency gate blocks its dependents (serial
  machine) — acceptable; surface *what* is being waited on in status.
- Re-apply diff correctness (env-only change must not look like a no-op) — hash the
  rendered instance spec into the ledger and compare.
- Blob-cache growth on small edge disks — size cap + LRU, and `docker-archive` blobs
  removable post-import.

## Open questions

- Poll interval / jitter defaults for very large fleets (thundering herd on a promote)
  — 15 m + full-interval jitter proposed; revisit with real fleet numbers.
- `remove-bundle` also removing *packages* with zero remaining instances (release
  dirs/images): keep-N prune now, or leave until disk pressure? Recommendation: prune
  on remove.

## Related

Parent scope (YAML contract, example flow) ·
[`rollback-health-scope.md`](rollback-health-scope.md) ·
[`../rartifacts/resolve-scope.md`](../rartifacts/resolve-scope.md) (the server half of
resolution) · [`../rartifacts/token-auth-scope.md`](../rartifacts/token-auth-scope.md)
(agent registration + revocation, public/private tiers).
