# Bus — subject-scoped `bus.watch` grants + revoke-terminates-stream (session)

- Date: 2026-07-13
- Scope: ../../scope/bus/bus-watch-subject-scope-scope.md
- Stage: library-posture (lb consumed via `lb-node`); this is a platform data-isolation fix.
- Status: what-shipped (both gaps built + tested green; released as node-v0.4.3)
- Issue: NubeDev/lb#49. Downstream consumer: NubeDev/cc-app `care.feed.watch` (milestone 08/10).

## Goal
Close two data-isolation gaps in the generic `bus.watch` motion plane so an embedder can stream a
per-entity feed safely (cc-app's `care.feed.<child>`, rule 7 — a guardian sees only their own child):

- **Gap 1 — subject-scoped grants.** `bus.watch` authorized only the workspace-wide
  `mcp:bus.watch:call`; the subject never entered the cap check. Add a `bus:<subject>:watch` scoped
  grant the authorize path honors, converging with the channel `bus:chan/*:sub` subject-cap grammar.
- **Gap 2 — revoke-terminates-stream.** The subscribe gate ran once; a `grants.revoke` didn't close
  an already-open SSE stream. Add a bounded re-check tick that ends a stream when its grant is revoked.

Additive only — no WIT/ABI/SDK change (grant-grammar + authorize-path + stream-lifecycle, host-side).

## What shipped

### The convergence decision (prior art)
Two grant idioms existed: the channel **subject-cap** (`Surface::Bus`, resource = the thing,
`bus:chan/*:sub`, via `lb_caps::check`) and the entity-scoped **`{table,ids}`** record selector
(`Scope::Ids`, via `check_scoped`). A bus subject is a *string*, not a `{table,id}` pair, so the
subject-cap idiom is the right prior art — the new cap is `bus:<subject>:watch`. Rejected reusing
`Scope::Ids{table:"bus"}`: it can't express the `/`·`.`-segmented wildcard a per-entity feed needs
(`care.feed.*`) and would overload a record-row selector with subject-string meaning.

### Gap 1 — subject-scoped bus grants
- **`Action::Watch`** added to the caps grammar (`crates/caps/src/request.rs`) — additive; distinct
  from `Sub` so a generic watch grant and a channel `:sub` grant never alias.
- **`crates/host/src/bus/scoped.rs`** (new, ≤140 lines, one responsibility): `authorize_subject_scoped`
  reads the caller's **live** caps (`resolve_caps_live` — fresh store read, not the token), then:
  - no `bus:*:watch` grant held ⇒ `WatchMode::Open` (back-compat: today's behaviour, any subject);
  - holds one AND it matches this subject ⇒ `WatchMode::Scoped`;
  - holds one but none matches ⇒ `BusError::Denied` (Gap 1 closed).
  Matching reuses the exact `lb_caps::matches` grammar, so `bus:care.feed.*:watch` authorizes
  `care.feed.leo` but not `other.feed.x`.
- **`crates/host/src/bus/watch.rs`**: `bus_watch` now takes `&Store` and calls the scoped gate AFTER
  the coarse `mcp:bus.watch:call` gate. Workspace wall (Gate 1) still runs first in `authorize_bus`.
- Callers threaded the store: the dedicated `GET /bus/{subject}/stream` route and the mux hub's
  `bus:` subject arm (`role/gateway/src/routes/bus.rs`, `session/events/subject.rs`).

### The stickiness fix (found mid-build — an isolation hole)
A naive "scoped mode = caller currently holds any `bus:*:watch` grant" rule re-opens a subject when
the caller's **last** grant is revoked (drops to open mode → Gap 2 could never close the stream, and
a fresh re-subscribe would succeed). Fixed by anchoring the stream-lifetime check to the *grant*:
`still_scoped_authorized(store, principal, ws, subject)` returns whether a matching
`bus:<subject>:watch` grant STILL exists. A `WatchMode::Scoped` stream requires it to persist, so a
last-grant revoke **denies**, never re-opens. Regression:
`revoking_the_only_grant_denies_the_subject_it_does_not_reopen`.

### Gap 2 — revoke-terminates-stream
- **`role/gateway/src/session/events/recheck.rs`** (new): `WatchRecheck` wraps an open stream's recv
  loop with a bounded tick (`RECHECK_INTERVAL = 3s`; `with_interval` is the ms test seam). On each
  tick it re-authorizes; mode-sticky (a `Scoped` stream needs its grant to persist, an `Open` stream
  must not become `Denied` — so a newly-added non-matching grant also tightens an open stream). On
  denial it ends the stream (`None`). `next_authorized(&sub)` is folded by the dedicated route;
  `guard_stream(recheck, inner)` wraps the mux hub's `bus:` arm so a revoke closes just that one
  multiplexed subscription while the connection lives on.
- Node-local (no cross-node signal) ⇒ symmetric-node-safe; a synced revoke closes the stream on the
  next tick wherever it lives. Bounded latency matches the ask and the rest of authz's freshness.

### Why a tick, not a push
Rejected a `grants.revoke → hub` push signal: it needs a workspace-scoped fan-out channel and couples
the revoke site to the stream registry, for no better guarantee than a short tick. A tick is local to
the stream that must close and reads the local store — symmetric and simple.

## Tests (real infra, seeded via the real write path — CLAUDE §9)

`crates/host/tests/bus_test.rs` (12 tests, all green — the 4 pre-existing + 8 new/updated):
```
running 12 tests
test no_scoped_grant_means_backward_compatible_open_watch ... ok      # back-compat (load-bearing)
test a_scoped_grant_confines_the_holder_to_its_subject ... ok         # Gap 1 deny
test a_wildcard_scoped_grant_matches_its_prefix_only ... ok           # wildcard match
test a_scoped_grant_in_another_workspace_does_not_authorize_here ... ok  # workspace isolation
test a_grant_assigned_after_login_is_honored_on_next_watch ... ok     # store-read freshness
test revoking_the_only_grant_denies_the_subject_it_does_not_reopen ... ok  # stickiness regression
test watch_without_the_cap_is_denied ... ok                          # coarse deny (unchanged)
... (wall/round-trip/cross-ws pre-existing) ...
test result: ok. 12 passed; 0 failed
```
`role/gateway/tests/bus_watch_revoke_test.rs` (Gap 2, real node + bus + gateway):
```
test revoking_the_scoped_grant_closes_the_open_stream_within_a_tick ... ok
test an_open_backcompat_stream_is_not_closed_by_an_unrelated_revoke ... ok
test result: ok. 2 passed; 0 failed
```
`crates/host/src/bus/scoped.rs` unit (grammar): 5 passed. `lb-caps`/`lb-authz`/gateway `bus_routes`
regression suites: green (no regressions). `cargo fmt` clean; `cargo clippy` clean on the new files.

Mandatory categories covered: **capability-deny** (Gap 1 + coarse), **workspace-isolation**
(cross-ws grant does not authorize). No mocks — `mem://` store, real Zenoh bus, real gateway, grants
written through `grant_assign`/`grant_revoke`.

## Files touched
- `crates/caps/src/request.rs` — `Action::Watch` (additive).
- `crates/host/src/bus/scoped.rs` — NEW: `authorize_subject_scoped`, `still_scoped_authorized`, `WatchMode`.
- `crates/host/src/bus/watch.rs` — `bus_watch` takes `&Store`, calls the scoped gate.
- `crates/host/src/bus/mod.rs`, `crates/host/src/lib.rs` — re-exports.
- `role/gateway/src/session/events/recheck.rs` — NEW: `WatchRecheck` + `guard_stream`.
- `role/gateway/src/session/events/{mod,subject}.rs`, `routes/bus.rs` — wire the re-check + store.
- `role/gateway/Cargo.toml` — `lb-store` dep + `lb-authz` dev-dep.
- Tests: `crates/host/tests/bus_test.rs`, `role/gateway/tests/bus_watch_revoke_test.rs`.

## Downstream
cc-app `care.feed.watch` (its `docs/debugging/authz/bus-watch-unscoped-and-no-midstream-revoke.md`)
upgrades from reach-check-at-subscribe to platform stream isolation + unlink-terminates once it bumps
its pin to node-v0.4.3. cc-app mints/revokes the grant through the generic `grants.assign`/`revoke`
MCP verbs on link/unlink — the cap string is opaque data (rule 10), no core knowledge of cc-app.
