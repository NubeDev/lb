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

- ~~Private registry credentials: config-file creds per registry in
  `/etc/rubixd/config.toml`, or force everything private through rartifacts archives?
  Recommendation: archives-only v1 (one trust model), creds later.~~
  **RESOLVED (slice 5): archives-only v1, as recommended.** Slice 5 does
  anonymous pulls only (`pull.rs` → `bollard::create_image`, no auth config).
  Creds deferred; revisit if a fleet needs a private registry that rartifacts
  archives can't cover.

### Answered in slice 5

- **Rootless docker: works, no special handling needed.** `dockerd::ensure` uses
  `bollard::Docker::connect_with_defaults`, which honours `$DOCKER_HOST` — the
  same posture as the systemd sandbox honouring `XDG_RUNTIME_DIR`. Verified
  against dockerd 29.5.3 at `/var/run/docker.sock`; a rootless socket is reached
  by exporting `$DOCKER_HOST` with no rubixd-side change.

### Raised and resolved in slice 5

- **Full image hygiene / GC — DONE.** `prune` now implements the
  `Backend::prune` contract against image tags exactly as systemd implements it
  against release dirs: sort descending, keep the top `keep_n`, always keep
  `instance.version` ∪ `instance.kept_previous`, return the removed versions so
  `commit` can splice them out of `kept_previous`. The host-wide
  reference-count that made this look hard is a `list_containers(all: true)`
  sweep: anything a container references — rubixd's or not, running or not — is
  retained regardless of `keep_n`, and the engine's 409 backstops the race.
  Tags are removed, never image ids, so a shared id keeps its layers until its
  last reference goes. Tests:
  `docker_prune_test.rs::prune_removes_unreferenced_tags_and_protects_retained`,
  `::prune_protects_kept_previous_outside_keep_window`,
  `::prune_spares_images_referenced_by_foreign_containers`.

- **Operator rollback to a pruned version — DONE.** `rollback` now recreates
  from the retained `rubixd/<pkg>:<v>` tag when `.prev` is absent or names a
  different version, so every version `kept_previous` advertises is reachable —
  the same guarantee systemd gives by flipping the `current` symlink to any
  retained release dir. `.prev` is still preferred when it IS the target (a
  rename is cheaper and keeps the container's identity). Retaining `.prev`
  *containers* per `keep_n` was considered and rejected: the retained tag is the
  durable handle, and holding N stopped containers per instance costs far more
  than N tags. Tests:
  `docker_operator_rollback_test.rs::rollback_recreates_from_retained_tag_when_prev_is_pruned`,
  `::rollback_from_tag_keeps_named_volume_data`.

- **Streaming-from-file import — DONE.** `import_archive` feeds
  `bollard::import_image_stream` a `ReaderStream` over a `tokio::fs::File`, so a
  multi-GB tarball never lands in RAM; resident cost is one chunk regardless of
  size. This did not need to wait for slice 6's fetcher — the buffering was in
  the backend, so the fix belonged here.

- **`Backend::rollback` gained a `previous_version` param.** The trait passed
  only `previous_release`, a backend-specific location: systemd's is a
  release-dir path whose tail IS the version, docker's is a container name that
  cannot name one. Docker could not learn its target. Every caller already
  computed `previous_version` and stopped at the transaction layer, so the fix
  was to pass it through; each backend now uses the handle it can actually use
  and ignores the other. (Reading the target from `current.kept_previous`
  instead was tried and is WRONG — on the auto-rollback path the runner has not
  committed yet, so it is empty and every gate-failure reports Failed. The
  trait doc records this.)

### Raised by slice 5 (open)

- **Engine wiring for `docker-image`.** `resolve_version` skips non-`Exact`
  specs, so a bundle's `image:` Channel-pin never reaches `backend.install`
  through the transaction engine — docker-image instances are unreachable via
  the reconcile loop today. This is the engine's channel/range resolution, not a
  docker gap: it is slice 6's poller by design, and building a resolver now
  would front-run that design and be rewritten when the remote half lands. Slice
  5 exercises the path at the backend seam directly (`docker_image_test.rs`);
  the seam is proven, only the wiring is missing.

## Related

[`rollback-health-scope.md`](rollback-health-scope.md) (the wrapping transaction) ·
[`bundles-scope.md`](bundles-scope.md) (fetcher + poller) · parent scope §package model.
