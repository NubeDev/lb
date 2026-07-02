# A native sidecar's restart budget exhausts and never recovers — every call returns "restart budget exhausted" until the node is bounced

- Area: extensions (native tier)
- Status: resolved
- First seen: 2026-07-02
- Resolved: 2026-07-02
- Session: ../../sessions/extensions/native-sidecar-restart-budget-recovery-session.md
- Regression test: rust/crates/host/tests/native_test.rs (`exhausted_budget_is_recovered_by_reset_without_bouncing_the_node`, `sustained_health_decays_the_restart_count`); rust/crates/supervisor/tests/sidecar_test.rs (`rearm_recovers_an_exhausted_sidecar`, `reset_restarts_zeroes_the_counter_without_respawning`)

## Symptom
The federation datasources sidecar (native Tier-2, supervised by the host) becomes a permanent
dead-end after transient crashes: `datasource.test` and otherwise-valid `federation.query` calls all
fail with `supervisor: restart budget exhausted after 5 restarts`, even though nothing is actually
wrong with the query or the store. Only a full `make dev` (whole-node) restart clears it — there was
no way to recover from the UI.

## Reproduce
1. Install a native sidecar (`echo-sidecar` reproduces it; the real trigger was the datasources
   sidecar during a node restart / boot-key race).
2. Cause the child to crash `max_restarts` (default **5**) times — e.g. call its `crash` tool then a
   normal call, five times. Each crash is recovered by an on-demand restart (the supervision path in
   `crates/host/src/native/call.rs::call_once_or_restart`).
3. Crash it once more. The next call tries to restart, hits `restarts >= max_restarts`, and returns
   `RestartExhausted`.
4. **Every** subsequent call now fails the same way. The child is dead and the counter only ever
   increments — nothing re-arms it.

## Investigation
- `Sidecar::restart` (`crates/supervisor/src/sidecar.rs`) refuses once `self.restarts >=
  self.spec.backoff.max_restarts` and returns `SupervisorError::RestartExhausted`, leaving the child
  dead (`channel == None`) but the handle still in the `SidecarMap`.
- The `restarts` counter is monotonic: `restart` only ever does `self.restarts += 1`. There was no
  decay and no operator re-arm path.
- The existing operator `restart_native` couldn't rescue it: it calls the same budget-checked
  `restart()`, so once exhausted it also returns `RestartExhausted`.
- A transient cause (a boot-key race, a node-restart flap) can burn all 5 restarts in seconds, then
  the sidecar is poisoned indefinitely for a fault that no longer exists.

## Root cause
The restart budget is **monotonic and terminal**: it caps crash-looping (correct) but never
recovers, and no verb re-arms it. A bounded budget with no decay and no reset turns a *transient*
crash into a *permanent* outage — the budget can only be cleared by restarting the whole node
process (which re-derives a fresh `Sidecar` with `restarts = 0`).

## Fix
Make exhaustion recoverable at two layers, without bouncing the node:

1. **Operator reset (`native.reset`).** New `Sidecar::rearm` (supervisor) does a budget-**ignoring**
   respawn: kill any live channel (works even when the sidecar is already dead), relaunch from the
   same spec, re-handshake, and zero the counter. New host verb `reset_native`
   (`crates/host/src/native/lifecycle.rs`) gates it on `mcp:native.reset:call` (workspace-first),
   `rearm`s the handle, and resets the durable `restart_count` to 0. Wired to
   `POST /extensions/{ext}/reset` and a **Reset** button in the Extensions console (shown when
   `restart_count > 0`).
2. **Auto-decay after sustained health.** New `Backoff.cooloff` (default 30s). On the *successful*
   branch of a sidecar call, `decay_if_healthy` (host) checks the durable `healthy_since` timestamp;
   once the sidecar has served cleanly for the cool-off window since its last restart, it zeroes both
   the in-memory counter (`Sidecar::reset_restarts`) and the durable `restart_count`. A transient
   crash that then serves calls cleanly for the window self-heals — the exact live scenario.

Decay lives on the **call path**, not a new background health-poll loop, because the native tier is
deliberately on-demand (no timer drives `Sidecar::health` today); hanging decay off successful calls
keeps the model coherent and avoids per-sidecar task lifecycle.

## Verification
- `cargo test -p lb-supervisor` — `rearm_recovers_an_exhausted_sidecar` (exhaust → `rearm` →
  serves again, full budget re-armed) + `reset_restarts_zeroes_the_counter_without_respawning`.
- `cargo test -p lb-host --test native_test` — with a **real OS child**:
  `exhausted_budget_is_recovered_by_reset_without_bouncing_the_node` (5 crashes exhaust → 6th call
  returns `RestartExhausted` → `reset_native` → the same call answers again, count back to 0) and
  `sustained_health_decays_the_restart_count` (a crash then a call past the 30s cool-off decays the
  count to 0; a call within the window does not).
- Mandatory categories: `native_deny_test.rs::denies_reset_without_grant` (capability-deny) and
  `native_isolation_test.rs::ws_b_cannot_see_or_control_ws_a_sidecar` (workspace-isolation, extended
  to cover `reset`).

## Prevention
The four regression tests above make the class impossible to reintroduce silently: an exhausted
budget that could not be recovered by `reset`, or a healthy sidecar whose count never decayed, would
fail them. The `Reset` affordance gives operators a UI recovery path so a transient crash never again
requires a whole-node restart.
