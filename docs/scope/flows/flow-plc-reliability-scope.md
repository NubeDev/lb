# Flows scope — PLC-grade reliability + the reactive (Node-RED) run model

Status: scope (the ask). Promotes to `public/flows/` once shipped. Extends
[`flow-runtime-control-scope.md`](./flow-runtime-control-scope.md) (the async-drive / live-watch /
node-config slice that shipped just before this) and [`flow-run-scope.md`](./flow-run-scope.md) (the
run engine). Sibling: [`triggers-lifecycle-scope.md`](./triggers-lifecycle-scope.md) (the headless
cron/reconcile/source machinery this slice finally wires "Run" onto).

## The ask (from the user, against the live node)

> "A flow must run like a PLC — 100% reliable, backend-driven, and when you hit Run it runs until you
> stop it." … "It needs to be like Node-RED — it's a reactive engine."

Three concrete failures were seen on the running canvas (`:8080`, ws `acme`, flow `chain4`):

1. **Store revision conflict — the run never settles.** The banner showed
   `store backend error: Versioned error: A deserialization error occured: Invalid revision '174'
   for type 'Value'` (and, under a burst, `Failed to commit transaction due to a read or write
   conflict. This transaction can be retried`).
2. **`flows.run` re-fires ~2×/second for the same flow** — not user clicks — churning
   `flows.get` + `flows.runs.list`.
3. **Stop/Resume flash for ~0.5 s then vanish; "chain4 runs but no values."** The controls flicker
   and the live per-node values never stick.

Plus the product goal (#4): a run must be **durable and long-running** — the reactive-engine posture
the user named.

## Root cause (REPRODUCED end-to-end on the live node)

**One bug explains #1, #2, and #3: the run id is constant across every run of a flow, because the
gateway clock is frozen at startup.**

- `POST /flows/{id}/run` sends `ts: gw.now` (`role/gateway/src/routes/flows.rs:114`).
- `gw.now` is computed **once**, at gateway construction (`role/gateway/src/state.rs:62-65`,
  `try_from_env`), and stored as a field — it never advances for the life of the node process.
- The host mints the run id as `default_run_id(flow_id, now)` →
  `"{flow_id}-run-{now}"` (`crates/host/src/flows/run.rs:222-225`,
  `crates/host/src/flows/mod.rs:121`).

Therefore **every** `flows.run` for `chain4`, for the entire uptime of the node, resolves to the
**same** run id `chain4-run-<startup-secs>`. Consequences, each observed live:

- Re-running (or any re-fire) **re-drives the same, already-terminal run record** instead of starting
  a fresh run — the snapshot churns and the controls flicker (#3), and a watcher/poller keeps seeing
  the same id "active then done then active" (#2's churn).
- Two `flows.run` calls that overlap (rapid clicks, a reattach race, or the re-fire) both run
  `coordinator::start` → `run_store::create_run` for the **same** `flow_run` / `flow_step:*` records
  at once. Each `lb_store::write` does a server-side **read-modify-write of the monotonic `rev`**
  (`crates/store/src/write.rs:31-42`). Under the durable engine (`kv-surrealkv`) two writers read the
  same prior `rev` and both write `rev+1`, so a later read deserializes the wrong revision →
  `Invalid revision '…'` (#1); the burst form is the retryable transaction conflict.

**Live repro (captured this session):** firing 8 concurrent `POST /flows/chain4/run` returned, for
every caller, the identical `{"run_id":"chain4-run-1782809451"}` and a wall of
`store backend error: … read or write conflict. This transaction can be retried`. A single,
isolated run settles `success` cleanly — the failure is purely the **shared-run-id-under-concurrency**
shape, exactly the precedent fixed for the capped ring
(`debugging/observability/capped-insert-overgrows-cap-under-concurrency.md`).

> Note on #2's "2×/second": the React layer does **not** call `runFlow` from any effect — it is only
> the `handleRun` onClick (`FlowCanvas.tsx:240`). The churn the user saw is the **constant run id**
> making one finished run look perpetually re-runnable plus the `reattach([flow.id])` →
> `runs.list` → re-`watch` interplay folding over a never-fresh id. Fixing the run id removes the
> churn's fuel; the `reattach` dependency hygiene (below) removes the rest.

## The run model decision — reactive engine (Node-RED), not one-shot-only

Per the user ("like Node-RED — it's a reactive engine"): **a flow is *deployed* (armed), and then it
reacts.** This slice fixes the reliability bugs **and** aligns the run model:

- **Manual `Run` on a flow with no trigger/source = one fire-once injection.** It produces a single
  durable run with a **unique** id, drives to terminal, streams values, can be stopped. (A finite
  count-chain is naturally one-shot — re-running starts a *new* run, never re-drives the old one.)
- **A flow with a trigger/source = deploy/arm it.** `Run` enables it (`flows.enable` +
  `start_on_boot`) and the **reconciler owns it** — it reacts to its source/cron headless, survives
  browser close **and** node restart (`flows.resume` re-drives in-flight runs from durable state on
  boot). **Stop disarms** (`flows.enable{enabled:false}` + cancel the live run). This is the existing
  `react_cron` / `reconcile` / `source` machinery (`crates/host/src/flows/`), finally wired to the
  button — no new engine (the spine's load-bearing rule).

The canvas decides which mode by whether the open flow has an armable trigger/source node (the
descriptor `kind`), so the button reads "Run" (one-shot) or "Deploy"/"Stop" (continuous) honestly.

## Goals

- **Unique run id per manual run.** `flows.run` mints a fresh, collision-proof id when the caller
  supplies none — a ULID (the `lb_store::new_ulid` already used by the capped ring), not a
  coarse/frozen-clock derivation. Two runs of the same flow are two distinct `flow_run` records,
  always. *(Caller-supplied `run_id` is still honored for idempotent retries — that path is unchanged
  and is what resume/subflow rely on.)*
- **The frozen gateway clock is fixed at the source.** `gw.now` must advance per request (a live
  `SystemTime` read at the route, or a `now()` accessor), so `ts` and any derived id are real wall
  time — not a value frozen at boot. Tests that inject a fixed clock keep doing so (the seam stays).
- **Run-store writes cannot corrupt `rev` under concurrency.** Even with unique ids, a single run's
  records are touched by the drive loop and (for triggered flows) by concurrent branch settles; the
  store write path must be conflict-safe: **serialize per-`(ws,table,id)` + retry on the retryable
  conflict**, the exact design proven for `capped_insert` (per-key async lock + bounded
  retry-on-conflict). This hardens the primitive so it CANNOT race even under legit concurrency.
- **`coordinator::start` / `create_run` is idempotent under concurrency**, not just on a clean
  re-call: a second concurrent `start` of the same run id must no-op the seed (CAS / "create if
  absent"), never double-write the seed records.
- **Reactive deploy wired to the button**: `Run` on a triggered flow arms it via `flows.enable` +
  `start_on_boot`; `Stop` disarms; the reconciler drives it headless and resume re-drives on boot.
- **UI churn removed**: `reattach` runs once per opened flow (stable identity / guarded effect), the
  SSE effect does not re-subscribe on every render, and a finished run's controls retire cleanly and
  stay retired.

## Non-goals

- **Cross-node owner failover** for a deployed run stays the `node-roles` deferral — on restart the
  same node resumes from durable state via `flows.resume` (unchanged).
- **A new clock abstraction across the whole gateway** — fix the frozen `gw.now` minimally (live read
  / accessor); do not refactor all 35 `gw.now` call sites into a new time service in this slice.
- **Per-node step-level streaming** — a node remains the unit of motion (one settle event per node).
- **Replacing the run engine** — this is reliability + wiring, not a rewrite (flows spine rule).

## Intent / approach

1. **Kill the shared run id.** At the `flows.run` dispatch arm, when no `run_id` is supplied, mint a
   ULID-based id instead of `default_run_id(flow_id, now)`. Keep `default_run_id` only for the
   deterministic paths that *want* a stable id (cron `cron_run_id`, subflow `child_run_id`) — those
   already derive stable ids on purpose and are not the manual path.
2. **Unfreeze `gw.now`.** Make the route read a live time (an accessor `gw.now()` or a per-request
   `SystemTime` at the flows routes at minimum), so `ts` is real. Preserve the fixed-clock test seam.
3. **Harden the store write.** Port the `capped_insert` discipline into the run-store's write path:
   an in-process per-`(ws,table,id)` async lock around the rev-bumping `write`, plus a bounded
   retry-on-`is_retryable_conflict`. Decide placement — either a new conflict-safe `write` variant in
   `lb-store` reused by the run-store, or the lock/retry localized in `run_store.rs`. Prefer the
   store-level variant so every same-record writer is safe, not just the flows caller.
4. **Idempotent seed.** `create_run` uses create-if-absent (CAS) for the run + step rows so a racing
   second `start` no-ops rather than re-seeding.
5. **Wire reactive deploy.** The canvas detects a triggered/source flow and maps `Run`→`flows.enable`
   (arm + `start_on_boot`), `Stop`→disable + cancel; the reconciler owns the headless drive.
6. **UI hygiene.** Guard `reattach` so it fires once per flow open (not per render), and ensure the
   SSE `useEffect` keys only on `runId`.

## Testing plan (mandatory categories — real `mem://` + a real durable store + real bus/jobs/caps)

- **Regression — store rev conflict under concurrent run writes (MANDATORY):** spawn N concurrent
  drives / writes of the **same** run id; assert **no** `Invalid revision` / transaction-conflict
  error escapes and the run settles **once** (the run record ends terminal exactly once). This fails
  on today's code (reproduced live) and passes after the hardening. Mirror
  `capped_test::concurrent_inserts_past_cap_leave_exactly_cap`.
- **Unique run id:** two back-to-back `flows.run` on the same flow (same wall second) return **two
  distinct** run ids and two independent terminal runs (neither re-drives the other).
- **Live clock:** two runs separated by a real interval carry **different** `ts` (the frozen-clock
  regression) — or assert the minted id is unique regardless of clock granularity.
- **Idempotent seed under race:** two concurrent `start` of the *same* (caller-supplied) run id leave
  exactly one seed (step rows not duplicated/garbled), and the run still settles once (resume
  exactly-once preserved).
- **Reactive deploy:** arming a triggered flow via the Run→enable path makes it react to its
  source/cron headless (a source event drives a run with nobody watching); Stop disarms (no further
  runs); restart + `flows.resume` re-drives an in-flight run from durable state.
- **Capability deny + workspace isolation** for any new/changed verb surface (per
  `scope/testing/testing-scope.md`).
- **Frontend (Vitest, real spawned gateway):** a single Run yields one fresh run that settles with
  values rendered and the controls staying put for the run's duration (not flickering); `reattach`
  does not re-fire `flows.run`; export still round-trips `needs`.

## Risks & hard problems

- **The frozen-clock fix has blast radius** — `gw.now` feeds token `iat`/`exp` and 35 call sites.
  Keep the change minimal and the fixed-clock **test seam intact** (tests construct `Gateway::new`
  with an explicit `now`); only the *production* default must advance.
- **Conflict-safe write placement** — a store-level locked+retry `write` is the cleanest (every caller
  benefits) but touches the load-bearing primitive; an alternative is to localize it in the run-store.
  Decide and document the rejected alternative.
- **Reactive deploy semantics** — "Run = deploy" must not silently keep a manual one-shot flow armed;
  the canvas must read the flow's trigger/source presence correctly so the button means what it says.

## Debugging entries to log (this session)

- `debugging/flows/frozen-gw-now-collides-run-ids.md` — the constant-run-id root cause (#1/#2/#3).
- `debugging/flows/run-store-rev-conflict-under-concurrency.md` — the rev RMW race + the
  serialize+retry fix (cross-link the capped-ring precedent).

## Related

- README `§3` (state vs motion, capability-first, the wall), `§6.1`/`§6.10` (batch-as-job).
- `flow-runtime-control-scope.md` (the slice this hardens), `flow-run-scope.md` (the engine),
  `triggers-lifecycle-scope.md` (the reactive cron/reconcile/source machinery `Run`→`enable` uses).
- Precedent fix reused verbatim in shape:
  `debugging/observability/capped-insert-overgrows-cap-under-concurrency.md`
  (per-key async lock + bounded retry-on-conflict over a SurrealDB rev/trim race).
- Promotes to `public/flows/flows.md`.

## Resolution (shipped 2026-06-30 — see the session doc)

Backend reliability (Goals/Intent items 1–4 + 6) shipped and **verified live**; session:
[`sessions/flows/flow-plc-reliability-session.md`](../../sessions/flows/flow-plc-reliability-session.md).

Decisions made:
- **Conflict-safe write placement:** chose the **store-level** `lb_store::write_locked`
  (per-`(ws,table,id)` async lock + bounded retry, the `capped_insert` shape) over localizing in
  `run_store.rs`. Rejected the localized variant because it leaves every other same-record writer
  (jobs, future callers) exposed — the primitive is the correctness boundary. Adopted by `run_store`
  + `lb-jobs::{create,update}` (the `job:{run_id}` row races too — found via the regression test).
- **Frozen clock:** minimal fix — `Gateway::now` accessor (live read / injected `fixed_now`), the 35
  `gw.now` sites became `gw.now()`. Did **not** introduce a gateway-wide time service (a Non-goal);
  the fixed-clock test seam (`Gateway::new(..., NOW)`) is intact.
- **Unique id:** ULID at the `flows.run` dispatch arm; `default_run_id` retained for the deterministic
  inject/cron path only.
- **Idempotent seed:** create-if-absent in `create_run` under a per-`(ws,run_id)` lock.
- **UI hygiene (item 6):** no change required — `reattach` already keys on `[flow.id]` and the SSE
  effect on `[runId]`; the churn was backend-fueled (the constant id). Verified, not skipped.

**Item 5 (reactive cron firing) — DONE in-session** after a live failure ("added a cron trigger,
count never goes up"). Root cause was two disconnected gaps + a compounding one:
- **no production driver** ticked `react_to_flows_cron`/`reconcile_flows` (only tests called them) →
  added `spawn_flow_reactors` (a detached per-node tick, live clock) wired into node boot;
- **UI cron ≠ reactor field**: the canvas writes `trigger.config.cron`, the reactor scans
  `flow.cron` → `flows.save` now **derives** `flow.cron` from a `mode:cron` trigger node
  (`derive_cron_from_trigger`), resetting `next_attempt_ts` on change (no clobber when absent);
- **frozen production clock**: the node binary built the gateway with `Gateway::new(.., now)` (the
  fixed-clock seam) → switched to `Gateway::new_live`.

Verified live: a `* * * * *` trigger fires every minute headless, runs settle `success` with real
values; `next_attempt_ts` advances per fire. Debug:
`debugging/flows/cron-trigger-never-fires-no-reactor-driver.md`; e2e test
`flows_triggers_test::cron_trigger_node_derives_flow_cron_and_fires_a_run`.

**Remaining (UI polish, not a bug):** a canvas *armed-state* affordance — show "armed · next fire" +
Stop=disarm for a cron/source flow, instead of the last finite run's "DONE"; the full "Run reads
Deploy/Stop by descriptor kind" presentation rides here. The engine fires headless correctly today.
