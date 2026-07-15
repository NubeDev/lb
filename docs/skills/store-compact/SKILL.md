---
name: store-compact
description: >-
  Bound a node's SurrealKV commit log at runtime: read `store.status` (log bytes, segment
  count, last-compaction outcome, threshold advisory), trigger a `store.compact` job, watch it
  complete, and confirm the shrink. Read this when a task involves "the store/disk keeps
  growing", "slow boot / long replay", "commit log size", "compaction", or the
  `store:status:read` / `store:compact:run` capabilities. IMPORTANT: `store.compact` is a JOB
  (whole-log I/O) — it enqueues and returns `{job_id}`; the reactor executes the pass, and
  writes pause behind the store's session mutex while it runs.
---

# Compacting the store's commit log (`store.status` → `store.compact`)

The engine is append-only: superseded versions and eviction tombstones stay in the commit log
until a compaction rewrites it down to the live set. Boot compacts automatically; this surface
is for a **running** node (online-compaction scope, issue #67).

## 1. Check status

```jsonc
// POST /mcp/call   { "tool": "store.status", "args": {} }
// needs store:status:read (admins have it via store:*:read)
{
  "persistent": true,
  "log_bytes": 57308,         // what the next boot replays
  "segment_count": 2,
  "threshold_bytes": 268435456,
  "advisory": null,           // set once log_bytes crosses the threshold — same string the node logs
  "last_compaction": { "ok": true, "before_bytes": 58230, "after_bytes": 11024, ... }
}
```

A healthy quiet store has `advisory: null`. The node also logs the advisory from its reactor
(30 s tick) once the log crosses `threshold_bytes` — visibility first; nothing auto-compacts.

## 2. Trigger the pass (admin)

```jsonc
// POST /mcp/call   { "tool": "store.compact", "args": {} }
// needs store:compact:run (ADMIN_ONLY — a pass pauses every writer briefly)
{ "job_id": "store-compact-01K02V4EYD3M6AV0F7DGFJ0S5J" }
```

The verb only **enqueues** a durable `store-compact` job — compaction is unbounded whole-log
I/O and never runs on a request path. The reactor picks it up within its tick.

## 3. Watch the job

The job record (`job:{id}` in your workspace) completes with the outcome on its payload:

```jsonc
{
  "status": "done",
  "payload": {
    "requested_by": "user:ada",
    "outcome": { "ok": true, "before_bytes": 58230, "after_bytes": 11024, "duration_ms": 22 }
  }
}
```

`status: failed` carries `payload.error` instead — a failed pass leaves the log exactly as it
was (best-effort contract; the store keeps serving on a fresh handle either way).

## 4. Confirm the shrink

Call `store.status` again: `log_bytes` now tracks `after_bytes` (plus a little engine
bookkeeping), and `last_compaction` records the pass.

## Grounding (live run, 2026-07-15)

From the real end-to-end run in `crates/host/tests/store_admin_test.rs::compact_job_enqueues_drains_and_records_outcome`
(a real SurrealKV dir, real churn, the reactor's own drain fn):

```text
GROUNDING: status before: log_bytes=57308 advisory=none
GROUNDING: store.compact -> job store-compact-01KXJCC803WX7WGVPV3DJXFTJ5
GROUNDING: job record payload: {"requested_by":"user:ada","outcome":{"ok":true,
  "before_bytes":58230,"after_bytes":11024,"duration_ms":22,"error":null}}
GROUNDING: status after: log_bytes=12464, last_compaction.ok=true
```

## What to know before leaning on it

- **Writers pause, briefly.** The pass holds the global session mutex: concurrent writes block
  and land after the swap (tested at 16-way concurrency — none lost, none duplicated). Expect
  a pause proportional to log size; that is why it's operator-triggered and threshold-informed.
- **Memory stores refuse** (`memory store has no commit log to compact`).
- **A pass takes a few seconds of quiesce-wait** on any store that ever defined an index
  (i.e. every real node): the old engine leaks an inert reference upstream, so the pass
  proves quiescence by file stability instead of full release. Expected, not a failure.
- **Crash-safe**: a kill mid-pass leaves either the old or the new log; the next open
  completes or discards the interrupted pass (never a corrupt store, never lost writes —
  see `debugging/store/compaction-merge-eats-next-sessions-writes.md` for the engine bug the
  implementation works around).
- Caps: reading is `store:status:read`; running is `store:compact:run` — a distinct `run`
  action, deliberately NOT implied by `store:*:write`.
