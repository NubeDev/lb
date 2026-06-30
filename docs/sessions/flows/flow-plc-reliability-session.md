# Flows — PLC-grade reliability (unique run ids, conflict-safe writes, idempotent seed)

- Area: flows
- Status: shipped (green) — reliability (items 1–4,6) + reactive cron firing (item 5) live; only a
  canvas armed-state affordance remains (UI polish, not a bug)
- Scope: [`scope/flows/flow-plc-reliability-scope.md`](../../scope/flows/flow-plc-reliability-scope.md).
- Extends: [`flow-runtime-control-scope.md`](../../scope/flows/flow-runtime-control-scope.md) (the
  async-drive/live-watch slice), [`flow-run-scope.md`](../../scope/flows/flow-run-scope.md) (the engine).
- Debug:
  [`debugging/flows/frozen-gw-now-collides-run-ids.md`](../../debugging/flows/frozen-gw-now-collides-run-ids.md),
  [`debugging/flows/run-store-rev-conflict-under-concurrency.md`](../../debugging/flows/run-store-rev-conflict-under-concurrency.md).
- Precedent reused: [`debugging/observability/capped-insert-overgrows-cap-under-concurrency.md`](../../debugging/observability/capped-insert-overgrows-cap-under-concurrency.md).

## The ask

"A flow must run like a PLC — 100% reliable, backend-driven" and "like Node-RED — a reactive engine."
Three live failures on `chain4` (`:8080`, ws `acme`): a store `Invalid revision` / `read or write
conflict` banner; `flows.run` apparently re-firing ~2×/s; Stop/Resume flickering with no values.

## Reproduced first (HOW-TO-CODE)

Seeded a real 4-node `count` chain `chain4` through the live `flows.save`, then fired 8 concurrent
`POST /flows/chain4/run`. Result: **all 8 (and a later isolated run) returned the identical**
`{"run_id":"chain4-run-1782811850"}` plus 6 `read or write conflict … can be retried` errors. This
confirmed the scope's diagnosis verbatim: **the run id is constant for the node's whole uptime**
(`gw.now` frozen at boot → `default_run_id(flow_id, now)` constant), which both re-drove one terminal
run (the churn/flicker) and made overlapping runs race the run-store's monotonic `rev` RMW (the
store errors).

## What was built (backend reliability — items 1–4 + 6)

1. **Unique run id per manual run.** `flows.run` mints a `lb_store::new_ulid()` when no `run_id` is
   supplied (`crates/host/src/flows/mod.rs`); a caller-supplied id is still honored (resume/subflow/
   retry). `default_run_id` kept only for the deterministic inject/cron path.
2. **Unfroze the gateway clock.** `Gateway::now` is now an accessor — live `SystemTime` in
   production, injected `fixed_now: Option<u64>` for tests (`role/gateway/src/state.rs`); the 35
   `gw.now` field reads became `gw.now()`. The fixed-clock test seam (`Gateway::new(node, key, NOW)`)
   is preserved, so token-expiry tests stay deterministic.
3. **Conflict-safe store write.** New store-level `lb_store::write_locked`
   (`crates/store/src/write_locked.rs`): per-`(ws,table,id)` async lock + bounded retry-on-conflict,
   the proven `capped_insert` shape. Adopted by `run_store.rs` (alias `write_locked as write`) and
   `lb-jobs` `create`/`update` (the `job:{run_id}` row is written by both seed and `complete`).
   Store-level placement chosen over localizing in run_store so **every** same-record writer is safe
   (the scope's preferred option; rejected alternative documented in the scope + debug entry).
4. **Idempotent seed under race.** `create_run` does create-if-absent under a per-`(ws,run_id)` lock,
   so a racing second `start` no-ops instead of re-seeding `pending` over an in-flight run.
6. **UI hygiene — already correct, verified.** `FlowCanvas` reattach effect already keys on
   `[flow.id]` with a stable `reattach` callback (fires once per opened flow); `useFlowRun`'s SSE
   effect already keys only on `[runId]`; no effect calls `runFlow`. The churn's *fuel* was the
   backend constant id (now fixed), exactly as the scope predicted. No UI change was needed — noted
   as a verified non-deviation rather than a silent skip.

## Item 5 (reactive cron) — completed in the same session after a live failure

I initially deferred item 5, but the user immediately hit it: "added a cron trigger every minute, the
count never goes up" + a trigger node stuck on "DONE" + a 403 on Save node. Reproduced and found
**two disconnected gaps** (a third compounding one):
- **No production driver ticked the reactor.** `react_to_flows_cron` / `reconcile_flows` (and
  `react_to_reminders`) were only ever called from tests — the node binary never spawned a tick. So a
  `mode:cron` flow had no clock to fire it, ever.
- **UI cron never reached the field the reactor scans.** The canvas writes the schedule to the
  trigger node's `config.cron`; the reactor reads top-level `flow.cron`; `flows.save` never derived
  one from the other (so `flow.cron` stayed `null` and the reactor skipped the flow).
- **Compounding:** the production gateway was built with `Gateway::new(.., now)` — the *fixed-clock*
  constructor — so even after items 1–4 the live node was still frozen (it only escaped the run-id
  collision because the id no longer derives from `now`). Switched `node/src/main.rs` to
  `Gateway::new_live`.

Fixes: **`spawn_flow_reactors`** (`crates/host/src/flows/reactor_loop.rs`) — a detached per-node tick
(reconcile + cron) every 5s over the configured ws under a node-internal `Principal::routed`, live
clock — wired into node boot; **`derive_cron_from_trigger`** in `flows.save` (cron trigger node →
canonical `flow.cron`, reset `next_attempt_ts` on change, no clobber when absent); **`Gateway::new_live`**
in the node binary. Debug: `debugging/flows/cron-trigger-never-fires-no-reactor-driver.md`.

The 403 on Save node was a stale-token artifact of the frozen clock; with the live clock,
`POST /flows/node/chain4/trigger-5` returns 200. The "DONE" trigger is correct per-run (each minute is
one finite run; the flow keeps minting new runs) — a clearer armed-vs-last-run canvas affordance is a
UI follow-up, the engine is right.

## Remaining UI follow-up (not a bug)

- **Canvas armed-state affordance** — for a cron/source flow, show "armed · next fire HH:MM" and a
  Stop=disarm, rather than the last finite run's "DONE". Engine behavior is correct and headless;
  this is presentation. The full "Run button reads Deploy/Stop by descriptor kind" polish rides here.

## Tests (real store/caps/jobs — no mocks)

- `crates/store/tests/write_locked_test.rs::concurrent_same_record_writes_never_conflict` — 16
  concurrent same-record writes, no error, coherent `rev == 16`.
- `crates/host/tests/flows_plc_reliability_test.rs`:
  `concurrent_same_run_id_never_conflicts_and_settles_once` (the MANDATORY regression — fails on
  pre-fix code, green after), `manual_run_mints_unique_run_id`, `run_denied_without_capability`
  (cap-deny), `run_isolated_across_workspaces` (ws-isolation).
- Regression-clean: `flows_run_test`, `flows_runtime_control_test`, `flows_triggers_test`,
  `lb-store`, `lb-jobs` all green. UI unit `pnpm test` 186/186.
- `pnpm test:gateway`: the 4–6 failures are **pre-existing** (system/admin/routing flakes; clean
  master fails *more* of them), unrelated to flows.

## Live verification

After `cargo build -p node` + relaunch with the prior `LB_*` env: 8 concurrent
`POST /flows/chain4/run` returned **8 distinct ULID ids**, **zero** store errors, the node log clean
of `Invalid revision`/`conflict`, and each run settled `success` with all nodes ok. The before/after
is unambiguous.
