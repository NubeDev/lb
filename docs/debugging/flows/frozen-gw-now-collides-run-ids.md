# Frozen gateway clock collides every flow run id (one run id for the node's whole uptime)

- Area: flows (role/gateway clock → host run-id minting)
- Status: resolved
- First seen: 2026-06-30
- Resolved: 2026-06-30
- Session: ../../sessions/flows/flow-plc-reliability-session.md
- Scope: ../../scope/flows/flow-plc-reliability-scope.md
- Regression tests:
  - rust/crates/host/tests/flows_plc_reliability_test.rs::manual_run_mints_unique_run_id
  - rust/crates/host/tests/flows_plc_reliability_test.rs::concurrent_same_run_id_never_conflicts_and_settles_once

## Symptom

On the live canvas (`:8080`, ws `acme`, flow `chain4`), three failures at once: a store banner
`Invalid revision '174' for type 'Value'` / `read or write conflict … can be retried`; `flows.run`
appearing to re-fire ~2×/second for one flow; and the Stop/Resume controls flashing ~0.5 s then
vanishing with "chain4 runs but no values."

**Live repro:** firing 8 concurrent `POST /flows/chain4/run` returned, for *every* caller, the
**identical** `{"run_id":"chain4-run-1782811850"}` plus a wall of `read or write conflict` errors. A
single isolated run settled `success` cleanly.

## Reproduce

```
TOK=$(curl -s -X POST :8080/login -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
for i in $(seq 1 8); do curl -s -X POST :8080/flows/chain4/run -H "authorization: Bearer $TOK" -d '{}' & done; wait
```

Before the fix: 8× the same `chain4-run-<startup-secs>` id + transaction-conflict errors.

## Root cause

The run id was constant for the **life of the node process**. `POST /flows/{id}/run` sent
`ts: gw.now` (`role/gateway/src/routes/flows.rs`), and `gw.now` was a `u64` **field computed once at
gateway construction** (`Gateway::boot`, `role/gateway/src/state.rs`) — it never advanced. The host
then minted the run id as `default_run_id(flow_id, now)` = `"{flow_id}-run-{now}"`
(`crates/host/src/flows/run.rs`). So every `flows.run` for a flow resolved to the *same* run id.

Consequences: re-running re-drove the same already-terminal run (snapshot churn + flickering
controls + a poller seeing one id "active→done→active"); and two overlapping `flows.run` calls both
seeded the **same** `flow_run`/`flow_step:*` records, racing the run-store's monotonic `rev` RMW (see
the sibling entry `run-store-rev-conflict-under-concurrency.md`).

## Fix

Two changes, minimal blast radius:

1. **Unique run id per manual run** — at the `flows.run` dispatch arm, when no `run_id` is supplied,
   mint a ULID (`lb_store::new_ulid`) instead of `default_run_id` (`crates/host/src/flows/mod.rs`).
   A caller-supplied `run_id` is still honored verbatim (the idempotent-retry / resume / subflow
   path). `default_run_id` is kept for the deterministic inject/cron path (`triggers.rs`).
2. **Unfreeze the gateway clock** — `Gateway::now` is now an accessor: a live `SystemTime` read in
   production, or an injected fixed clock when a test pinned one (`fixed_now: Option<u64>`). The
   field-read `gw.now` became the call `gw.now()` at all 35 sites. The fixed-clock **test seam is
   preserved**: `Gateway::new(node, key, NOW)` pins the clock exactly as before.

## Verification

Live: 8 concurrent `POST /flows/chain4/run` returned **8 distinct ULID ids** and **zero** store
errors (node log clean of `Invalid revision` / `conflict`); each run settled `success`, all nodes ok.
Unit: `manual_run_mints_unique_run_id` (two no-id runs → distinct ids, both terminal) and the
concurrency regression below, green.

## Prevention

`manual_run_mints_unique_run_id` stands guard on the unique-id property; the concurrency regression
guards the no-conflict-under-shared-id property. The fixed-clock seam keeps token-expiry tests
deterministic so the live-clock change cannot regress auth.
