# Upgrading

Behaviour changes that act on data you already have. Read the note for the release you are
crossing **before** you upgrade the binary — some of these delete rows on the first reactor tick.

---

## SurrealDB 3 / SurrealKV 0.21 — your existing store cannot be opened

> **Stop. This one destroys nothing, but it strands everything.**
> A node running this release **will not start** against a store directory written by any earlier
> release. There is no migration, no conversion tool, and no in-place upgrade. Export what you
> need **before** you replace the binary; afterwards the old node is the only thing that can read
> it.

### What changed

The embedded engine moved from **SurrealDB 2 on SurrealKV 0.9** to **SurrealDB 3 on SurrealKV
0.21**. SurrealKV 0.9 was a bitcask commit log: one append-only file per store, in a `clog/`
directory. SurrealKV 0.21 is an LSM tree and writes `sstables/`, `wal/`, `vlog/`,
`versioned_index/` and a `manifest` instead. The two formats have nothing in common, and 0.21
cannot read 0.9.

The node detects the old layout by the presence of `clog/` — only the old engine ever created one
— and refuses to start with a message that names the cause:

```text
the store at /var/lib/lb/store was written by surrealkv 0.9 (SurrealDB 2) — it has a `clog/`
directory, which only that engine created. surrealkv 0.21 is a different on-disc format and
cannot read it, so this node will NOT start against it. Nothing has been modified. Either point
the node at a fresh directory, or move `clog/` aside once you have exported anything you still
need from the old build.
```

**Nothing on disc is modified or deleted.** The refusal is deliberate: a node that silently served
an empty workspace where your data used to be would be far worse than a node that will not start
and says why.

### Pre-upgrade checklist

Do all of this while the **old** binary is still installed and running.

1. **Export every series you need to keep.** Use the ingest read verbs against the running old
   node — `series.list` to enumerate, then read the samples out per series. There is no bulk dump
   verb; write what you export somewhere off the node.
2. **Export the dead-letter table** if it is evidence in an open investigation.
3. **Export your configuration records** — retention policies (`series.retention.list`), workspace
   membership, roles and installed extension manifests. These are records in the store like any
   other, so they are stranded with it.
4. **Stop the node. Move the store directory aside** rather than deleting it, so you can go back
   to the old binary if an export turns out to be incomplete:
   ```bash
   systemctl stop rubix-ai
   mv /var/lib/lb/store /var/lib/lb/store.surrealdb2
   ```
5. **Start the new binary** against a fresh, empty directory. It will create the new layout.
6. **Re-apply the configuration you exported in step 3**, then re-import data if you are importing
   any.

Keep `store.surrealdb2` until you are satisfied. It is the only copy of that data in a form
anything can read.

### Compaction is gone — and so is every guard around it

The old engine never reclaimed anything on its own, so the node carried a whole apparatus to do it
by hand. SurrealKV 0.21 compacts continuously in the background, and all of that apparatus was
removed rather than left as dead weight:

| Removed | Was |
|---|---|
| The `store.compact` **pass** | A stop-the-world rewrite of the whole log, pausing every writer |
| The **boot** compaction pass | Ran on every `Store::open` |
| The boot **memory guard** | Refused to open when the log exceeded available RAM |
| `LB_STORE_OPEN_UNGUARDED` | The override for that refusal. **This variable no longer does anything** — remove it from your unit files |

The `store.compact` **verb** still exists, and still needs `store:compact:run`, so nothing that
calls it breaks. It now records a skip and returns without doing any work.

Boot no longer replays history, so a large store is no longer a slow or dangerous start. Measured
on the same machine: a 92,657-byte store opened in 114 ms, and a 42,816,755-byte store — 462 times
the bytes — in 143 ms.

### If you set `LB_STORE_MAX_BYTES`, unset it for now

The disc budget still reports `budget_bytes` and `headroom_bytes` honestly, and still logs an
advisory past the soft mark. But its only action was to enqueue a `store.compact` pass, and that
pass is now a no-op.

There is a known defect on top of that: past the 95% hard mark the driver is exempt from its
one-hour interval, and the no-op pass reports a *skip* rather than an unproductive result, so the
"budget too small" latch never engages. A budgeted node above 95% enqueues a no-op job **every 30
seconds, indefinitely**, and each job is a record written to the store. Until it is fixed, leave
the variable unset and watch `store.status` yourself.

### There is no longer a manual way to reclaim disc space

This is the real operational change, and it deserves to be said in one sentence: **if a store is
too large, the only levers are tighter retention, moving data off the node, or a bigger disc.**
Background merges reclaim space on the engine's schedule, not on yours.

Note also that **deleting rows makes the store bigger before it makes it smaller**. On an LSM tree
a delete appends a tombstone, and both the row and the tombstone leave the disc only at a later
merge. Measured: deleting 20,000 rows moved a store from 13,110,037 to 14,083,889 bytes, and it
was still 14,085,369 bytes fifteen seconds later.

### Related

- [Store](../store/store.md) — how the LSM engine lays data out, and what `store.status` measures.
- Skill: `docs/skills/store-compact/SKILL.md`

---

## Bounded defaults — unpoliced series are now evicted (issue [#122](https://github.com/NubeDev/lb/issues/122))

> **Do this first, on every node, before you upgrade.**
> Any series with **no `series_retention` policy record** and more than **100,000** raw samples
> **permanently loses** everything past the newest 100,000 — on the first retention tick after
> the node starts, at most 300 seconds in. There are no rollup tiers to fall back on, because a
> series with no policy has no tiers. The data is gone.
>
> The opt-out is to **create** a policy record with `max_samples: 0`. Leaving no record is no
> longer the opt-out — it is now opt-*in* to the default cap.

### What changed

`DEFAULT_MAX_SAMPLES` (100,000) was advisory for one release: the GC logged a warning saying a
future release would enforce it. **This is that release.** A series that no `series_retention`
record covers is now FIFO-evicted down to 100,000 raw samples by the same 300 s retention reactor
that already runs your policies — oldest `ts` first.

100,000 samples is not a lot. A single point polled at **1 Hz crosses it in about 1.2 days**. At
2 s cadence, ~2.3 days. Any node that has been ingesting an unpoliced series for longer than that
is over the line today and will be trimmed on the first tick.

There is nothing to fold the evicted window into. The policy-driven cap rolls over-cap rows into
the policy's tiers before deleting them, so coarse history survives; the default cap has no policy
and therefore no tiers, so the eviction is outright. That is why this note is loud.

### The opt-out: policy-record **existence** decides

This is the whole rule, and it is the reverse of what it used to be:

| `series_retention` record for the series | Before | Now |
| --- | --- | --- |
| **No record at all** | unbounded, warning logged | **capped at 100,000, rows evicted** |
| Record with `max_samples: 0` | unbounded | **unbounded**, honoured exactly as written |
| Record with `max_samples: N` | capped at N | capped at N (unchanged) |

`max_samples: 0` is the explicit, permanent "keep everything" — the escape hatch the previous
release's warning text already promised operators. It is honoured as written and nothing in this
change touches it.

Longest-matching-prefix still decides which policy governs a series, and "covered" means a policy
prefix the series name starts with. A policy at `modbus.` covers `modbus.plant-a.coil-3`, so that
series is *not* unpoliced even though no record names it directly.

### Pre-upgrade checklist

Run this against each workspace on each node, **before** replacing the binary.

1. **List what you have.** `series.list {prefix: ""}` (cap `mcp:series.list:call`) gives every
   registered series name in the workspace.
2. **List what is policed.** `series.retention.list` gives every policy prefix. Anything in step 1
   that does not start with one of these prefixes is **unpoliced** and in scope for the new cap.
3. **Find the ones that will actually lose data.** For each unpoliced series, `series.stats
   {series}` (cap `mcp:series.stats:call`) reports its sample count. A count over **100,000** means
   rows will be deleted on the first tick after upgrade.
   `series.retention.status {series}` will tell you what governs a given series if you would rather
   ask per-series than reason about prefixes.
4. **Decide, per unpoliced series — and write the decision down as a record.** There are three
   honest answers, and all three are a `series.retention.set` call
   (cap `mcp:series.retention.set:call`):

   ```jsonc
   // (a) Keep it truly unbounded — the explicit opt-out. Must exist BEFORE you upgrade.
   series.retention.set { prefix: "legacy.audit.", max_samples: 0 }

   // (b) Keep more than the default, with a number you chose.
   series.retention.set { prefix: "plant-a.", max_samples: 5000000 }

   // (c) Accept a bound, but keep coarse history — the shape we actually recommend.
   series.retention.set {
     prefix: "plant-a.",
     max_samples: 100000,
     tiers: [{width_ms: 900000, keep_for_ms: 0, method: "avg"}]
   }
   ```

   Option (c) is the one worth the extra minute: with a tier, the over-cap window is **rolled up
   before it is evicted**, so you keep a 15-minute average forever and only lose raw resolution.
   Without a tier — including under the default cap — eviction is unrecoverable.
5. **Apply the records, then verify.** Re-run step 2 and confirm every series you care about is now
   covered. Only then upgrade.
6. **Expect the store to grow first** (see "Eviction does not free space" below). Make sure the
   disc has headroom for that before you upgrade; there is no pass you can run to get it back
   sooner.

If you upgrade first and set policies after, the first tick has already run. `max_samples: 0`
applied afterwards keeps what is left; it does not bring anything back.

### Every eviction is reported — check for these

The default cap is never silent. Each affected series produces a notice on the pass:

- **In `GcPass::warnings`**, returned by the `series.retention.gc` verb to its caller.
- **In the node log**, at `WARN`, emitted by the retention reactor.
- **On the stored pass record**, so a pass that ran while nobody was watching is still auditable.

The notice names the series, how many samples it held, how many were evicted, and how to opt out.
`GcPass::capped_raw` counts the rows. After the first post-upgrade tick, read the warnings — that
list is the exact set of series that just lost history.

### Dead letters now expire after 30 days

`ingest_dead_letter` — must-deliver samples diverted by the staging bound — was the one ingest
table nothing ever pruned. It now has a **30-day horizon** (`DEAD_LETTER_KEEP_MS`), enforced by the
same per-workspace GC pass, reported as `GcPass::evicted_dead_letters`.

The horizon is deliberately separate from `raw_for_ms` and is not a policy field: dead letters are
diagnostic, they are small, and tightening series retention to chase a disk problem must never
destroy the evidence of *why* records were dead-lettered.

**One sharp edge.** A row's age is its `dead_at` stamp — when the node diverted it. Rows written
**before this upgrade have no `dead_at`** and are aged on the sample's own `ts` instead, which is
the *producer's* clock. A producer with a badly skewed clock can therefore have pre-upgrade dead
letters pruned on the very first pass. If your dead-letter table is evidence in an open
investigation, **export it before upgrading.** The fallback is deliberate: the alternative was an
upgrade that pinned the whole legacy table to "never expires".

### API break for embedders

`lb_ingest::over_cap_warning` is **removed**. `lb_ingest::default_cap_notice(series, evicted,
count) -> String` replaces it. The old function produced an advisory ("a future release will
apply…"); the new one reports what was *deleted* and how to opt out, so the signature carries the
evicted count.

`GcPass` gains `evicted_dead_letters: usize`. It is `#[serde(default)]`, so stored pass records
written before the upgrade still deserialize; a Rust caller constructing `GcPass` literally will
need the new field.

### Eviction does not free space — it costs space

On SurrealKV a delete is a **tombstone**: a small record saying "this key is gone", appended like
any other write. The row it hides is still in an SSTable file on disc. The first post-upgrade tick
deletes rows and therefore makes `log_bytes` go **up**, not down.

Measured on a real store: deleting 20,000 rows moved it from 13,110,037 to 14,083,889 bytes, and
it was still 14,085,369 bytes fifteen seconds later.

The space comes back when a **background merge** rewrites the SSTables holding those rows. That
runs on the engine's schedule. **There is nothing you can trigger to make it happen sooner** —
`store.compact` is a no-op on this engine. Do not read the first tick's disc number as a failure
of this change; it is the engine working as designed. Make sure the disc has room for the growth
before you upgrade.

### Related

- Scope: `docs/scope/store/disk-budget-scope.md` (slice 3; decisions 7, 9, 10) ·
  Session: `docs/sessions/store/disk-budget-session.md`
- `docs/scope/ingest/series-sample-cap-scope.md` — the count bound and why it exists alongside a
  time horizon. `docs/scope/ingest/series-retention-scope.md` — rollup-then-evict.
- [Ingest](../ingest/ingest.md) — retention policies, tiers and filters.
  [Store](../store/store.md) — how the engine lays data out, and what `store.status` measures.
