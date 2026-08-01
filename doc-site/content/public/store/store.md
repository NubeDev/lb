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
    "error": null,
    "skipped": null          // set instead when boot DECLINED the pass — see the memory guard below
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

The driver is **threshold-driven, never compaction-on-a-tick**. Past the node's threshold it logs
an advisory (same posture as the sample-cap warnings) and `store.status` carries the same string.
What happens next depends on one piece of config:

- **No `LB_STORE_MAX_BYTES`** (the default): threshold is a flat 256 MiB, and a pass runs **only**
  when an authorized caller enqueues one. Advisory in, operator out — unchanged.
- **A disk budget set**: the threshold becomes 80% of the allowance and the node enqueues **one**
  `store.compact` job of its own when it is crossed (`requested_by: "system:store-budget"`), at most
  once an hour, with the 95% hard mark exempt from that interval. If a pass stops reclaiming
  (`after_bytes > 0.9 × before_bytes`) the node stops auto-compacting and says the budget is too
  small for the workload rather than pausing writes for nothing.

A pass on a 2.06 GiB log measured **771 ms** (reclaiming it to 16 MiB) — the number the automatic
trigger was approved on. See [Upgrading](../upgrading/upgrading.md) and the disk-budget scope.

## The boot memory guard — open without OOMing the box (issue #128)

Compaction bounds bytes on **disk**. It says nothing about **RAM**, and boot is the one place the
node predictably doubles its memory demand: the pass resolves every live value into memory, and the
open then replays the log again to build the index. On a 959 MB edge box with a 617 MB live set that
peaked at 879 MB RSS, and the kernel's **global** OOM killer took `sshd` down with the node —
`Restart=on-failure` then re-ran the same spike every 5 s until someone drove to the site.

Boot is now memory-aware. Three decisions, all arithmetic over this machine's own numbers
(`MemAvailable` from `/proc/meminfo`) — identical code on a 64 GB cloud node and a 959 MB edge node:

1. **The pass runs only if the machine can afford it.** `log_bytes > 0.5 × available RAM` ⇒ skipped.
2. **…and only if it is expected to pay.** If the *persisted* last pass reclaimed essentially
   nothing (`after > 0.9 × before`) and the log has not grown past `1.25 × after` since, the pass is
   skipped: re-compacting a log that **is** the live set is the most expensive possible no-op.
3. **A hopeless open is refused, not attempted.** `log_bytes > 1.0 × available RAM` ⇒ the node exits
   nonzero with a diagnostic instead of allocating. A restart loop then costs a `stat`, not 90 s of
   allocation — ssh stays usable, which is the difference between a remote fix and a site visit.

A skip is **loud**: one WARN line with all three numbers, and `store.status` serves
`last_compaction.skipped` with the reason, so "this node has stopped compacting at boot, and why"
is one MCP call away. A skip never suspends the *online* `store.compact` path.

A refusal looks like this on stderr / in `journalctl`:

```
store at /var/lib/lb/store will not fit in memory: the commit log is 943718400 bytes and only
841154560 bytes of RAM are available, so replaying it would likely OOM this machine. Refusing to
open (this is a heuristic guard). Remedies: add RAM or swap; compact the store on a larger machine;
lower retention so the next compaction shrinks the live set; or, if you know it fits, set
LB_STORE_OPEN_UNGUARDED=1 to force the open.
```

The node **never** falls back to an empty `mem://` store on refusal: a node silently serving a
workspace that "lost" its data is worse than a down node with a legible reason.

**Both guards fail open** where `/proc/meminfo` cannot be read (non-Linux, odd container mounts) —
today's behaviour, byte for byte — and `LB_STORE_OPEN_UNGUARDED=1` disables the *open* guard for an
operator who added swap or measured the headroom. (Nothing overrides the compaction skips: skipping
a pass is always safe.) Embedders can supply a truer ceiling than the host figure — a cgroup limit —
via `BootConfig::store_available_ram_bytes`.

The outcome of every pass that actually ran is persisted next to the store as
`<store dir>/../last-compaction.json` (atomic write, best-effort in both directions: a missing or
corrupt file simply means "no information" and the node compacts as it always did). It is a sibling
of the engine directory on purpose — `log_bytes` and the disk budget's marks never count it. That
file is what lets a skip decision, and the disk budget's "compaction stopped paying here"
suspension, survive the restart at which they matter.

The complementary layer belongs on the unit, not in the node: `MemoryMax` + `OOMPolicy=stop` +
`RestartSec`/`StartLimitBurst` is what keeps *every* workload on the box from taking the machine
down. The guard makes the node well-behaved even under a naked unit; deploy both.

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
- The memory guard: `docs/scope/store/boot-memory-guard-scope.md` (issue #128) · Session:
  `docs/sessions/store/boot-memory-guard-session.md` · Incident:
  `docs/debugging/store/boot-compaction-oom-kills-the-box.md`
- The session mutex this leans on: `docs/scope/store/session-concurrency-scope.md`
