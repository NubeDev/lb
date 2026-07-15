# Store — the commit log stays bounded (online compaction)

The embedded engine (SurrealDB on SurrealKV) is **append-only**: every write — each superseded
version, every tombstone — stays in the commit log, and boot replays all of it. Two mechanisms
bound it, and both now run without a restart in the loop:

- **Boot-time compaction** — `Store::open` rewrites the log down to the live set before the
  engine opens it (measured incident: a 1.5 GB log over a ~23 MB live set booted in 13–14 s).
- **Online compaction** (issue #67) — a long-running node no longer waits for a reboot: the
  `store.compact` job rewrites the log while the node serves, and `store.status` makes growth
  visible before it becomes a boot-time surprise.

## The verbs

### `store.status` → snapshot (gated `store:status:read`, admin-tier MCP call)

```jsonc
{
  "persistent": true,
  "log_bytes": 48123904,     // what the next boot will replay
  "segment_count": 3,
  "threshold_bytes": 268435456,
  "advisory": null,          // the warning string once log_bytes crosses the threshold
  "last_compaction": {       // boot or online, most recent in this process
    "at_epoch_ms": 1752576000000,
    "ok": true,
    "before_bytes": 1500000000,
    "after_bytes": 24000000,
    "duration_ms": 8400,
    "error": null
  }
}
```

Cheap by construction — file metadata only, below the namespace wall; it never reads a record.

### `store.compact` → `{ job_id }` (gated `store:compact:run`, admin-only)

**Always a job, never inline**: a pass is whole-log I/O with no upper bound. The verb enqueues a
durable `store-compact` job; the reactor drains it off the request path and records
`{before_bytes, after_bytes, duration_ms}` on the job record. `store:compact:run` is a distinct
`run` action — the broad author `store:*:write` wildcard can never trigger a node-pausing pass.

## How the pass works (and why it is safe)

1. Writes quiesce behind the store's **global session mutex** (the same lock that makes the
   workspace wall hold — here as an asset). Acquiring it means no operation is in flight.
2. The live `Surreal<Db>` handle is swapped out and dropped; the pass waits for the old engine
   to **provably quiesce**: full file release when possible (74–240 ms observed), else file
   size/mtime stability across a 2 s window — needed because a `DEFINE INDEX` at the pinned
   engine version leaks an inert engine reference forever
   (`docs/debugging/store/define-index-leaks-engine-blocks-release.md`). On timeout it *skips*
   the pass rather than ever compact under an engine that might still write.
3. The boot-time `compact_log` runs (shared implementation), then the store reopens and the
   handle swaps back in. Queued writers proceed — none lost, none duplicated (tested under
   16-way concurrency).
4. Crash-safe: a kill at any point leaves either the old or the new log, never a corrupt one;
   the next open completes or discards the interrupted pass.

The driver is **threshold-informed, operator-triggered**: past 256 MiB of log the node logs an
advisory (same posture as the sample-cap warnings) and `store.status` carries the same string —
but a pass runs only when an authorized caller enqueues one. No compaction-on-a-tick.

## The engine bug this work found (P0, fixed)

At the pinned `surrealkv 0.9.3`, `compact()` stages the live set in `.merge/` and the swap
happens at the *next* open — which applied it **after** the append-log was already open, so
that session's appends went into unlinked inodes and vanished at close. On the shipped boot
path this meant: **every boot from the third onward silently destroyed all writes made since
the previous boot.** `compact_log` now completes every merge with a throwaway, non-writing
open before any writing session touches the directory. Full story:
`docs/debugging/store/compaction-merge-eats-next-sessions-writes.md`.

## Related

- Scope: `docs/scope/store/online-compaction-scope.md` · Session:
  `docs/sessions/store/online-compaction-session.md` · Skill: `docs/skills/store-compact/SKILL.md`
- The session mutex this leans on: `docs/scope/store/session-concurrency-scope.md`
