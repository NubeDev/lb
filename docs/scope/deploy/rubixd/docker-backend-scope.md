# rubixd scope — docker backend (bollard)

Status: scope (the ask). Slice 5 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

The second `Backend`: `docker-image` (pull a public/registry image directly) and
`docker-archive` (`docker save` tarball from rartifacts, imported locally) packages,
driven through `bollard`, slotting under the *same* slice-4 transaction engine —
identical rollback semantics, containerized.

## Goals

- `backend/docker/` folder of verbs: `pull.rs` (registry pull w/ digest pin when
  given), `import.rs` (archive → `docker load`), `create.rs`, `swap.rs` (stop+rename
  old, create+start new), `remove.rs`, `status.rs`, `labels.rs`.
- Container contract per instance: name `<pkg>-<instance>`; labels
  `rubixd.package/version/instance/bundle/managed=true`; env/ports/volumes/restart
  policy rendered from bundle config.
- **Update = keep-then-replace**: old container stopped and renamed
  `<name>.prev` (kept, not removed); new created + started; slice-4 gate decides:
  commit → remove `.prev`; rollback → remove new, rename+start `.prev`.
- **Ownership wall**: destructive verbs require `rubixd.managed=true` **and** matching
  instance label; anything else → `ForeignContainer`, refused. rubixd **never removes a
  volume**, ever — data safety is absolute, orphaned volumes are listed by `status`.
- Image hygiene: keep the previous image (rollback target) per keep-N; prune older
  rubixd-labeled images on commit. Archives verified (SHA-256 + signature) before
  import; imported image id recorded in the ledger.
- Daemon detection: no reachable dockerd → docker-kind transitions become typed
  `BackendUnavailable` plan entries (status shows why), never a panic.

## Non-goals

- No compose/multi-container packages — one container per instance; multi-container
  stacks are a *bundle* of packages with `needs`. No swarm/k8s. No registry auth
  flows in v1 beyond anonymous pulls + rartifacts archives (private-registry creds =
  open question). No building images.

## Intent / approach

Same shape as slice 3: dumb total verbs, policy upstream. `swap.rs` mirrors the
symlink flip so the slice-4 engine treats both backends uniformly (`Backend::swap` /
`Backend::swap_back`). Health `tcp/http` gates hit **host-published ports** (edge
posture: host networking or explicit port maps — no docker-network DNS assumptions).
Alternative rejected: recreate-in-place without keeping the old container — rollback
would depend on re-pull/re-import working during an incident, exactly when the network
may be the problem.

## How it fits the core

Ownership labels = the isolation wall; verify-before-import = the trust wall; volumes
sacrosanct = the data-preservation rule. lb-specific rows N/A.

## Example flow

1. Bundle declares `timescaledb` (`docker-image`, `timescale/timescaledb:2.15.2-pg16`)
   instance `tsdb-main` → pull, create `timescaledb-tsdb-main` with labels, volume
   `tsdb-main-data`, port `5433:5432`, start, gate `tcp:5433` → commit.
2. Bump to `2.16.0-pg16` → pull, stop+rename old to `.prev`, create+start new, gate
   fails → remove new, restore `.prev`, mark `2.16.0` bad. Volume untouched throughout.

## Testing plan

Real local dockerd via bollard (suite **skips and reports** when absent — never fakes):

- image flow: pull tiny real image, create/start, status healthy.
- archive flow: `docker save` a real image in test setup, publish shape → import →
  create; tampered archive refused pre-import.
- update commit: new running, `.prev` removed, old image pruned per keep-N.
- update rollback (slice-4 gate, `FAIL_HEALTH` test image): new removed, `.prev`
  restored + running, volume file written by v1 present.
- **Ownership deny**: hand-run unlabeled container named like a target →
  `ForeignContainer` on every destructive verb.
- volume safety: remove instance → container gone, volume present, status lists it
  orphaned.
- no dockerd: plan shows `BackendUnavailable`, exit code still 0 for `status`.

## Risks & hard problems

- Port-collision at swap: old must be *stopped* before new starts (same host port) —
  the keep-then-replace order is load-bearing; gate timeout must cover the stop/start
  gap.
- bollard API drift vs dockerd versions — pin bollard, integration tests are the
  canary.
- Archive size (multi-GB Timescale) — ranged/resumable download is slice 6's fetcher;
  this slice streams import from file, never buffers in RAM.

## Open questions

- Private registry credentials: config-file creds per registry in
  `/etc/rubixd/config.toml`, or force everything private through rartifacts archives?
  Recommendation: archives-only v1 (one trust model), creds later.

## Related

[`rollback-health-scope.md`](rollback-health-scope.md) (the wrapping transaction) ·
[`bundles-scope.md`](bundles-scope.md) (fetcher + poller) · parent scope §package model.
