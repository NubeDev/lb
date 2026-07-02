# Extensions (native tier) — recoverable sidecar restart-budget exhaustion (session)

- Date: 2026-07-02
- Scope: ../../scope/mcp/native-tier-scope.md (native tier); resilience gap logged here
- Stage: S7/S8 — native Tier-2 supervision (STAGES.md)
- Status: done

## Goal
Make a native sidecar's restart-budget exhaustion **recoverable without bouncing the node**. A
transient crash (a node restart flap, a boot-key race) burned all 5 restarts and left every
subsequent `datasource.test`/`federation.query` failing with `restart budget exhausted after 5
restarts` — a permanent dead-end cleared only by a full `make dev`. Ship both an operator reset
control (UI/MCP) and an automatic decay after sustained health.

## What changed
Two layers, on top of the existing on-demand supervision (no new background loop):

**Supervisor (`crates/supervisor`)**
- `spec.rs`: new `Backoff.cooloff` (default 30s) — the healthy window after which the restart count
  may decay. `Backoff::default` sets it; `Spec::new` inherits it.
- `sidecar.rs`: `Sidecar::rearm` — a budget-**ignoring** respawn (kill any live channel, relaunch
  from the same spec, re-handshake, zero the counter); works even when the sidecar is already dead
  (`channel == None`, the exhausted state). `Sidecar::reset_restarts` — zero the counter without a
  respawn (the decay primitive). `Sidecar::cooloff` accessor.

**Host (`crates/host/src/native`)**
- `status.rs`: `NativeStatus.healthy_since: Option<u64>` — the cool-off clock (set on install + on
  each restart; `#[serde(default)]` so legacy records load as `None`).
- `lifecycle.rs`: `reset_native` (gated `mcp:native.reset:call`) — `rearm`s the handle + resets the
  durable `restart_count` to 0. `decay_if_healthy` — on a healthy sidecar past the cool-off window,
  zeroes the in-memory counter + the durable count. `bump_restart_count` now re-opens `healthy_since`
  on each restart.
- `tool.rs`: `call_sidecar` calls `decay_if_healthy` on the success branch (best-effort).
- `mod.rs`/`lib.rs`: export `reset_native` + `record_status`.

**Gateway (`role/gateway`)**
- `routes/ext.rs`: `reset_extension` → `POST /extensions/{ext}/reset` (uses `OsLauncher`), registered
  in `server.rs`.
- `session/credentials.rs`: dev-admin gets `mcp:native.reset:call`.
- `bin/test_gateway_seed.rs`: `/_seed/extension` accepts an optional `restart_count` (writes a
  `native_status`) so a UI test can surface the Reset affordance without a live child.

**UI (`ui/src`)**
- `lib/ext/ext.api.ts`: `resetExtension` (→ `ext_reset`). `lib/ipc/http.ts`: `ext_reset` →
  `POST /extensions/{ext}/reset`. `features/extensions/useExtensions.ts`: `reset`.
  `features/extensions/ExtensionsView.tsx`: a **Reset** button, shown for native rows with
  `restart_count > 0`.

## Decisions & alternatives
- **Decay on the call path, not a background health-poll loop.** There is no timer driving
  `Sidecar::health` today — the whole tier is on-demand (fault-during-call → restart+retry). A
  per-sidecar health task would add lifecycle surface (cancellation on stop/uninstall/reset) that
  fights the stateless-process grain. Decaying on a successful call is coherent and heals the exact
  live scenario (queries flowing again clears the count). Trade-off: a fully idle sidecar never
  decays — but an idle sidecar also can't exhaust, so it's moot.
- **New `native.reset` verb, not overloading `restart`.** `restart` is bounded (counts toward the
  budget, refuses when exhausted); `reset` is an unbounded rescue (re-arms the budget). They are
  genuinely different authorities — separate caps keep the deny/isolation model auditable.
- Rejected: clearing the budget inside `call_once_or_restart`'s fault path (would mask real crash
  loops); a `process:` capability surface (the MCP gate already expresses the authority).

## Tests
Mandatory categories both covered; all real (no mocks — real supervised OS child for the proofs).

- `cargo test -p lb-supervisor` — 8 passed, incl. `rearm_recovers_an_exhausted_sidecar`,
  `reset_restarts_zeroes_the_counter_without_respawning`, `rearm_refuses_a_never_policy`.
- `cargo test -p lb-host --test native_test --test native_deny_test --test native_isolation_test`:
  - `native_test` (real `echo-sidecar`): `exhausted_budget_is_recovered_by_reset_without_bouncing_the_node`,
    `sustained_health_decays_the_restart_count` — plus the existing restart proof.
  - **capability-deny**: `native_deny_test::denies_reset_without_grant`.
  - **workspace-isolation**: `native_isolation_test::ws_b_cannot_see_or_control_ws_a_sidecar`
    (extended to `reset`).
- `pnpm test:gateway ExtensionsView` — 6 passed (Reset affordance shown only for a restarted native;
  hidden at count 0; workspace-isolation).

Green output pasted in the session log / PR; see the debugging entry for the command list.

## Debugging
- [extensions/sidecar-restart-budget-exhausted-permanent.md](../../debugging/extensions/sidecar-restart-budget-exhausted-permanent.md)
  — the exhaustion-is-permanent bug, root cause, and the four regression tests. README row added.

## Public / scope updates
- Promoted the drivable surface to `public/extensions/extensions.md` (the `native.reset` verb, the
  `/extensions/{ext}/reset` route, the Reset button, and the auto-decay behavior).

## Skill docs
- Updated `skills/extensions/SKILL.md` with the reset + auto-decay recovery path (the new
  agent-/operator-drivable surface). Grounded in the real-gateway run above.

## Dead ends / surprises
- The task context described branch `ce-v3` with an in-progress `grant_ui_scope_to_admin` build
  break; the actual repo was on `master` at the `ce-node-wiring` merge (that work already landed and
  builds clean). Confirmed with the user, stayed on `master`.
- `restart_native` already existed and was exported but **not wired to any route/UI** — only the
  new `reset` path reaches the console.

## Follow-ups
- Deeper OS hardening (cgroups/seccomp) for the sidecar remains a noted native-tier non-goal.
- If a background health-poll loop is ever added, decay could also run there for idle sidecars.
- STATUS.md: native-tier resilience note — not a stage gate; left to the maintainer to fold in.
