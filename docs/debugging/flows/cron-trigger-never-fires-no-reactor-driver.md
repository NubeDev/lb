# A `mode:cron` flow trigger never fires — no reactor driver + node-config/flow.cron disconnect

- Area: flows (cron reactor wiring + save-time cron derivation)
- Status: resolved
- First seen: 2026-06-30
- Resolved: 2026-06-30
- Session: ../../sessions/flows/flow-plc-reliability-session.md
- Scope: ../../scope/flows/flow-plc-reliability-scope.md (item 5, reactive deploy)
- Regression test: rust/crates/host/tests/flows_triggers_test.rs::cron_trigger_node_derives_flow_cron_and_fires_a_run

## Symptom

On the live canvas the user added a **cron trigger (every minute)** to `chain4` and **the count never
went up** — no runs fired. The trigger node also showed a confusing terminal "DONE".

## Reproduce

Save a flow whose trigger node has `config: {mode:"cron", cron:"* * * * *"}` and wait. Pre-fix:
`GET /flows/chain4` shows top-level `"cron": null`, `next_attempt_ts` never advances, and no
`chain4-cron-*` runs ever appear — for the node's whole uptime.

## Root cause — TWO disconnected gaps

1. **No driver ticked the reactor.** `react_to_flows_cron` (and `reconcile_flows`, and even
   `react_to_reminders`) is a stateless scan over the durable flow set — correct by design (rule 4:
   no long-lived in-process timer owns state). But **nothing called it on a cadence in production**:
   a workspace-wide grep showed the only call sites were *tests*. The node binary booted the gateway
   and the github/federation roles but never spawned a reactor tick. So an armed cron flow had no
   clock to fire it.
2. **The UI's schedule never reached the field the reactor scans.** The canvas writes the cron spec
   into the **trigger node's `config.cron`**; `react_to_flows_cron` reads the **top-level
   `flow.cron`**. `flows.save` never derived one from the other, so `flow.cron` stayed `null` and the
   reactor skipped the flow (`let Some(schedule) = flow.cron else { continue }`).

(Compounding it, the production gateway was built with `Gateway::new(.., now)` — the *fixed-clock*
constructor — so even derived timestamps were frozen; see
`frozen-gw-now-collides-run-ids.md`. Fixed here too by switching the node binary to
`Gateway::new_live`.)

## Fix

1. **`spawn_flow_reactors`** (`crates/host/src/flows/reactor_loop.rs`) — one detached owner per node
   that ticks `reconcile_flows` + `react_to_flows_cron` every few seconds over the configured
   workspace(s), under a node-internal system principal (`Principal::routed("node:reactor", ws, …)`),
   reading a **live** wall clock per tick. Wired in `node/src/main.rs` after boot. Errors are logged,
   never fatal — one bad flow can't stop the heartbeat. On restart the scan resumes from durable
   `next_attempt_ts` (fire-once-then-skip; no backfill).
2. **`derive_cron_from_trigger`** in `flows.save` (`crates/host/src/flows/save.rs`) — when a
   `mode:"cron"` trigger node is present, its `config.cron` becomes the canonical `flow.cron` (a
   cleared spec disarms); `next_attempt_ts` resets to 0 on a change so the reactor recomputes the
   slot. When no cron trigger exists, `flow.cron` is left as supplied (a direct API caller / test can
   still set it — no silent clobber).
3. **`Gateway::new_live`** in `node/src/main.rs` so production runs on a live clock.

## Verification

Unit: `cron_trigger_node_derives_flow_cron_and_fires_a_run` — saves the canvas-shaped flow, asserts
`flow.cron` is derived, drives the reactor, and asserts the fired run settles `success` with the count
node's real `{"count":4}` value. **Live:** saved `chain4` with a `* * * * *` trigger; `GET` showed
`cron:"* * * * *"` derived; over ~80s two cron runs fired **on their own** at consecutive minute
boundaries (`chain4-cron-1782813960`, `…4020`), each `success` with `a→{count:4}`, `b→{count:1}`;
`next_attempt_ts` advanced 60s between fires.

## Prevention

The e2e regression locks both halves (derive + fire). The reactor tick is now part of node boot, so
any cron/source flow on any node fires headless — the "run like a PLC, fire until you stop it" posture.

## Note — the trigger "DONE"

A cron flow produces one **finite run per firing**; within that run the trigger node legitimately
reaches `done`. "DONE" is the *last run's* terminal state, not the flow's — the flow keeps minting new
runs every minute. (A cleaner canvas affordance for "armed/next-fire vs last-run" is a UI follow-up;
the engine behavior is correct.)
