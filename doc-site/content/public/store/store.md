# Store — how the node keeps its data on disc

Every node embeds its own database. There is no separate database server to install or run. The
engine is **SurrealDB 3** running on **SurrealKV 0.21**, and both are compiled into the node
binary.

SurrealKV is an **LSM tree**. LSM stands for *log-structured merge tree*. It is worth
understanding the shape, because almost every question an operator asks about disc space has its
answer here.

## How a write reaches the disc

1. A write goes into a **memtable** — a sorted table held in memory — and, at the same moment,
   into the **write-ahead log** (`wal/`). The write-ahead log is what makes the write survive a
   power cut before the memtable has been saved.
2. When the memtable reaches its size limit (**100 MB**, the engine default), the engine writes it
   out as one **SSTable** file under `sstables/`. SSTable means *sorted string table*: a file of
   key-value pairs, written once and never modified afterwards.
3. Background tasks inside the engine **merge** SSTables together over time. This is called
   compaction. During a merge, a key that has been overwritten several times keeps only its
   newest value, and a deleted key is dropped for good.

Step 3 is the important one. **Compaction is automatic and continuous.** The node does not
schedule it, cannot trigger it, and never pauses writes for it.

## What is on disc

```
<store path>/
  sstables/          the data, in immutable .sst files
  wal/               the write-ahead log
  vlog/              the value log
  versioned_index/   the index that supports versioned reads
  manifest           which SSTable files are currently live
```

Every one of these counts towards the node's disc use, and `store.status` sums all of them.

## The verbs

### `store.status` → snapshot

Capability: `store:status:read`. Cheap by construction — it reads file sizes only. It never reads
a record, so it needs no workspace principal.

```jsonc
{
  "persistent": true,
  "log_bytes": 13110037,      // every byte the store occupies on disc
  "segment_count": 6,         // number of .sst files
  "threshold_bytes": 268435456,
  "advisory": null,           // a warning string once log_bytes crosses the threshold
  "budget_bytes": null,       // LB_STORE_MAX_BYTES, echoed back
  "headroom_bytes": null,     // budget - log_bytes, saturating at 0
  "free_disk_bytes": null,    // filesystem free space; always null today
  "last_compaction": null
}
```

`log_bytes` is a historical name. Under the old engine the store really was one commit log. The
field is kept because the disc budget and every existing reader already speak it, and renaming it
would break them for no gain.

> **This measurement was blind until recently, and the consequence was severe.** It summed a
> `clog/` directory that only the old engine ever created, so on SurrealKV 0.21 it reported the
> size of `manifest` alone. Measured on a real store: **55 bytes reported against 133,159 bytes
> actually on disc**, and writing 1,000 records moved the reported figure from 55 to 55. Because
> the disc budget decides purely on `log_bytes`, the budget could never fire. It is fixed, and
> `store/tests/status_disc_bytes_test.rs` pins it by walking the whole directory as ground truth.

### `store.compact` → `{ job_id }`

Capability: `store:compact:run`, admin only. **This verb does nothing.**

It is kept so that existing callers and the disc budget keep working, but the pass behind it was
removed with the SurrealKV 0.21 upgrade. It enqueues a job, the job runs, and the job records a
skip:

```jsonc
{
  "ok": true,
  "before_bytes": 0,
  "after_bytes": 0,
  "duration_ms": 0,
  "skipped": "engine compacts automatically (surrealkv 0.21 LSM)"
}
```

There is nothing to invoke. The old implementation reached past SurrealDB with a second direct
SurrealKV handle, quiesced every writer behind the global session lock, swapped the live handle
out, rewrote the whole log, and reopened. That whole mechanism existed because SurrealKV 0.9 never
reclaimed anything on its own. SurrealKV 0.21 does it in the background, so the mechanism was
deleted rather than kept as dead weight.

**The honest consequence: a node has no manual lever to reclaim disc space.** If a store is
growing, the only things that change the outcome are tightening retention so fewer rows are kept,
or giving the node a larger disc. Waiting for the background merge is the only reclamation path,
and it runs on the engine's schedule, not yours.

## Deleting rows makes the store bigger, not smaller

This surprises people, so it is stated plainly with numbers.

On an LSM tree a delete is a **tombstone** — a small record that says "this key is gone". It is
appended like any other write. The row it hides is still in an SSTable file. Both the row and the
tombstone disappear only when a background merge rewrites the SSTable that holds them.

Measured on a real store:

| Rows deleted | Bytes before | Bytes after | Bytes 10–15 s later |
|---|---|---|---|
| 300 | 196,006 | 209,778 | 210,582 |
| 20,000 | 13,110,037 | 14,083,889 | 14,085,369 |

Nothing was reclaimed in either case, and the store grew both times. The reason is step 2 above:
the memtable limit is 100 MB, so at these sizes nothing had been flushed to an SSTable yet, and
with no SSTables there was nothing for a merge to rewrite.

The rule to carry away: **retention eviction costs disc space in the short term.** Plan for the
store to grow first and shrink later, and do not read the first tick after a retention change as a
failure.

## Boot

`Store::open` reads the manifest and the tail of the write-ahead log. It does not replay history.

| Store size | Open time |
|---|---|
| 92,657 bytes | 114 ms |
| 42,816,755 bytes | 143 ms |

The second store holds 462 times the bytes of the first and opens 29 ms slower. Open cost is no
longer a function of how much history a node has accumulated.

Two guards used to stand in front of this call and are now **gone**:

- **The boot compaction pass.** There is no pass to run.
- **The boot memory guard**, including `LB_STORE_OPEN_UNGUARDED`. It refused to open when the
  commit log was larger than available RAM, because the old engine replayed the whole log into
  memory. That premise is false now. It was removed rather than repaired, because it read the same
  broken measurement described above — it could only ever refuse zero bytes, and had the
  measurement been corrected while the guard remained, the guard would have begun refusing to open
  perfectly healthy large stores.

## The disc budget

Set `LB_STORE_MAX_BYTES` to a plain byte count and the node derives two marks from it: a **soft
mark** at 80% and a **hard mark** at 95%. Leave it unset and the node has no marks at all, warns
at a flat 256 MiB, and takes no action ever.

What the budget still does honestly:

- `store.status` reports `budget_bytes` and `headroom_bytes`, so an operator can watch the trend.
- The node logs an advisory once the store passes the soft mark.

What it can no longer do: **its only action is to enqueue `store.compact`, and that verb is a
no-op.** The marks are now a reporting mechanism, not a control mechanism.

> **Known defect, not yet fixed.** Past the hard mark the driver is exempt from its one-hour
> minimum interval, and the no-op pass reports a skip rather than an unproductive result, so the
> "budget too small for this workload" latch never engages. A budgeted node above 95% therefore
> enqueues one no-op job **every 30 seconds, indefinitely**. Each job is a record written to the
> store, so the behaviour adds a small amount of the very growth it is trying to prevent. Until it
> is addressed, prefer leaving `LB_STORE_MAX_BYTES` unset and watching `log_bytes` yourself.

`free_disk_bytes` is always `null`. Measuring it needs a `statvfs`-class system call and no
filesystem-stat crate is a dependency of the workspace, so the field is honestly absent rather
than guessed. Check real free space with `df`. Remember that the budget bounds the store directory
only — extension artifacts, sidecar binaries and OS logs share the filesystem and sit outside it.

## Upgrading from a SurrealDB 2 node

**A store written by the old engine cannot be opened by this one.** SurrealKV 0.9 wrote a bitcask
commit log into `clog/`; 0.21 is a different on-disc format entirely. The node refuses to start
and says so:

```text
the store at /var/lib/lb/store was written by surrealkv 0.9 (SurrealDB 2) — it has a `clog/`
directory, which only that engine created. surrealkv 0.21 is a different on-disc format and
cannot read it, so this node will NOT start against it. Nothing has been modified. Either point
the node at a fresh directory, or move `clog/` aside once you have exported anything you still
need from the old build.
```

Nothing is modified and nothing is deleted. See [Upgrading](../upgrading/upgrading.md) for what to
export before you cross this line.

## Related

- Skill: `docs/skills/store-compact/SKILL.md`
- The session mutex the store leans on: `docs/scope/store/session-concurrency-scope.md`
- The disc budget: `docs/scope/store/disk-budget-scope.md`
