---
name: store-compact
description: >-
  Diagnose a node whose store keeps growing on disc. Read `store.status` (bytes on disc, SSTable
  count, disc-budget headroom, threshold advisory) and understand what the node can and cannot do
  about it. Read this when a task involves "the store/disk keeps growing", "compaction", "the
  store won't shrink after deleting rows", the disc budget (`LB_STORE_MAX_BYTES`), or the
  `store:status:read` / `store:compact:run` capabilities. IMPORTANT and counter-intuitive:
  since the SurrealKV 0.21 upgrade the engine compacts itself in the background and
  **`store.compact` is a no-op** — there is no manual reclamation lever, and deleting rows makes
  the store BIGGER before it makes it smaller.
---

# Diagnosing store growth (`store.status`)

## Read this first: the engine changed, and so did the answers

The node embeds **SurrealDB 3 on SurrealKV 0.21**, an LSM tree. LSM stands for *log-structured
merge tree*.

Writes land in an in-memory **memtable** and in the write-ahead log at the same time. When the
memtable reaches **100 MB** (the engine default) it is written out as one immutable **SSTable**
file. Background tasks inside the engine merge SSTables together, and that merge is where
superseded values and deleted rows finally leave the disc.

Three consequences follow, and they are the whole reason this skill exists:

| | |
|---|---|
| **Compaction is automatic** | It runs continuously in the engine. Nothing schedules it, nothing triggers it, and it never pauses writes. |
| **`store.compact` does nothing** | The verb is kept so existing callers compile. It records a skip and returns. |
| **Deleting rows grows the store** | A delete appends a tombstone. The row is freed only at a later background merge. |

Everything the previous version of this skill described — a stop-the-world pass, a boot
compaction, a boot memory guard, `LB_STORE_OPEN_UNGUARDED`, `last-compaction.json` as a live
signal — belonged to SurrealKV 0.9 and is gone. If you see that advice anywhere, it is from
before the upgrade.

## 1. Read the status

```jsonc
// POST /mcp/call   { "tool": "store.status", "args": {} }
// needs store:status:read (admins have it via store:*:read)
{
  "persistent": true,
  "log_bytes": 13110037,      // EVERY byte the store occupies on disc
  "segment_count": 6,         // number of sstables/*.sst files
  "threshold_bytes": 268435456,
  "advisory": null,           // set once log_bytes crosses the threshold
  "budget_bytes": null,       // LB_STORE_MAX_BYTES, echoed back
  "headroom_bytes": null,     // budget - log_bytes, saturating at 0
  "free_disk_bytes": null,    // ALWAYS null — see section 4
  "last_compaction": null
}
```

`log_bytes` is a historical name from the old engine, kept because the budget and every existing
reader already speak it. It now sums `sstables/`, `wal/`, `vlog/`, `versioned_index/` and
`manifest`, recursively. It also still sums `clog/`, which only the old engine created — a
leftover directory from a SurrealDB 2 store therefore still counts against the budget, which is
the correct behaviour.

The call reads file sizes only. It never touches a record, so it is cheap enough to poll.

## 2. "The store keeps growing" — what to actually check

Work down this list in order.

1. **Is it growing, or did you just delete rows?** Deletes add bytes. Take two readings a few
   minutes apart before concluding anything.
2. **How many SSTables?** `segment_count: 0` means nothing has been flushed out of the memtable
   yet, so no background merge has had anything to work on. On a store under about 100 MB this is
   normal and there is nothing to reclaim.
3. **Is retention actually bounding the rows?** This is the only real lever. `series.retention.list`
   shows the policies; a series covered by no prefix falls under the default cap. See the
   `ingest-series` skill.
4. **Is the disc itself full?** `df` on the filesystem holding the store. `free_disk_bytes` will
   not tell you (section 4).

## 3. What `store.compact` does now

```jsonc
// POST /mcp/call   { "tool": "store.compact", "args": {} }
// needs store:compact:run (ADMIN_ONLY)
{ "job_id": "store-compact-01K02V4EYD3M6AV0F7DGFJ0S5J" }
```

The job is enqueued, the reactor drains it, and the outcome on the job record is:

```jsonc
{
  "ok": true,
  "before_bytes": 0,
  "after_bytes": 0,
  "duration_ms": 0,
  "skipped": "engine compacts automatically (surrealkv 0.21 LSM)"
}
```

`ok: true` with `skipped` set means the pass was correctly declined, not that it failed. Do not
call this verb expecting `log_bytes` to change. It will not.

The one thing this is still good for: it proves the reactor is alive and draining jobs.

## 4. The disc budget (`LB_STORE_MAX_BYTES`)

```bash
LB_STORE_MAX_BYTES=8388608      # a PLAIN BYTE COUNT. Not "8MB", not "8Mi" — those are malformed.
```

Read once at the binary boundary. A malformed value warns on stderr and leaves the node
unbudgeted; boot never panics over a typo.

| | |
|---|---|
| soft mark | **80%** of the budget — also becomes `threshold_bytes` |
| hard mark | **95%** of the budget |
| headroom | `budget - log_bytes`, saturating at 0 |

**Unset ⇒ no marks at all**, a flat 256 MiB advisory, and nothing automatic ever.

### What the budget can and cannot do

It reports honestly: `budget_bytes`, `headroom_bytes`, and an advisory in the log once the soft
mark is crossed. Poll it — disc fills over days and the trend is the point.

It cannot act. **Its only action is to enqueue `store.compact`, which is a no-op.** The marks are
a reporting mechanism now, not a control mechanism.

> **Known defect. Prefer leaving `LB_STORE_MAX_BYTES` unset.**
> Past the hard mark the driver is exempt from its one-hour minimum interval, and the no-op pass
> reports a *skip* rather than an unproductive result, so the "budget too small for this workload"
> latch never engages. A budgeted node above 95% enqueues one no-op job **every 30 seconds,
> indefinitely**. Each job is a record written to the store, so it adds a little of the growth it
> is meant to prevent. Watch `log_bytes` yourself instead until this is fixed.

### `free_disk_bytes` is always `null`

The field, its serialization and its call site all ship, but the figure needs a `statvfs`-class
system call and no filesystem-stat crate is a direct dependency of the workspace. It is honestly
absent rather than guessed. Use `df`.

Remember the budget bounds the **store directory** only. Extension artifacts, sidecar binaries and
OS logs share the filesystem and are outside the number.

## 5. Grounding — measured on real stores

**Deleting rows does not free disc space.** Real SurrealKV directories, real records through the
real write path:

```text
300 rows deleted:     196,006 -> 209,778 bytes    (210,582 ten seconds later)
20,000 rows deleted:  13,110,037 -> 14,083,889    (14,085,369 fifteen seconds later)
```

Both grew. Nothing was reclaimed, because at these sizes the 100 MB memtable had never flushed, so
there were no SSTables for a merge to rewrite.

**Open cost no longer depends on history.** Same machine, two stores:

```text
92,657 bytes      -> opened in 114 ms
42,816,755 bytes  -> opened in 143 ms     (462x the bytes, 29 ms slower)
```

**The status measurement was blind before it was fixed.** Revert-checked on a real store:

```text
reported log_bytes = 55        actual bytes on disc = 133,159
after writing 1,000 records:   55 -> 55
```

It summed `clog/`, which SurrealKV 0.21 never creates, so it reported the size of `manifest`
alone. Because the budget driver decides purely on `log_bytes`, the budget could never fire — the
disc protection was silently inert. Pinned by `store/tests/status_disc_bytes_test.rs`, which walks
the whole directory as ground truth and never names a directory itself.

## 6. A node that refuses to start against an old store

```text
the store at /var/lib/lb/store was written by surrealkv 0.9 (SurrealDB 2) — it has a `clog/`
directory, which only that engine created. surrealkv 0.21 is a different on-disc format and
cannot read it, so this node will NOT start against it. Nothing has been modified. Either point
the node at a fresh directory, or move `clog/` aside once you have exported anything you still
need from the old build.
```

This is correct behaviour, not a bug. Serving an empty workspace where data used to be is worse
than refusing to start. Nothing on disc has been changed. See the Upgrading page for what to
export first.

## What to know before leaning on any of this

- **Memory stores report `persistent: false`** and zero bytes. There is no disc to measure.
- **The budget is node-scoped**, never per-workspace. One allowance for the whole node.
- **The budget adds no verb.** It is boot config, not a record, so nobody can raise the ceiling
  through an MCP call.
- Capabilities: reading is `store:status:read`; running is `store:compact:run`, a distinct `run`
  action deliberately not implied by `store:*:write`.
- **There is no manual reclamation path.** If a store is too big, the honest options are tighten
  retention, move data off the node, or give it a bigger disc. Anything that promises otherwise is
  describing the old engine.
