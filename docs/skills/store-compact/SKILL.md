---
name: store-compact
description: >-
  Bound a node's SurrealKV commit log at runtime: read `store.status` (log bytes, segment
  count, last-compaction outcome, threshold advisory), trigger a `store.compact` job, watch it
  complete, and confirm the shrink. Read this when a task involves "the store/disk keeps
  growing", "slow boot / long replay", "commit log size", "compaction", or the
  `store:status:read` / `store:compact:run` capabilities. Also covers the node **disk budget**
  (`LB_STORE_MAX_BYTES`): the soft/hard marks that make the node compact itself, the headroom
  read, and the "budget too small for this workload" verdict. **Also covers the boot MEMORY guard
  (issue #128)**: why boot may SKIP its compaction pass (`last_compaction.skipped`), how to read the
  persisted `last-compaction.json`, and how to recover a node that REFUSES to open
  (`StoreError::WontFit`, `LB_STORE_OPEN_UNGUARDED=1`) — read this for "node won't start / OOM at
  boot / the box died on restart". IMPORTANT: `store.compact` is a JOB
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
  "last_compaction": { "ok": true, "before_bytes": 58230, "after_bytes": 11024, ... },

  // The disk budget (see below). All null on an unbudgeted node — the default.
  "budget_bytes": null,       // LB_STORE_MAX_BYTES, echoed back
  "headroom_bytes": null,     // budget - log_bytes, saturating at 0
  "free_disk_bytes": null     // filesystem free space — ALWAYS null today, see below
}
```

A healthy quiet store has `advisory: null`. The node also logs the advisory from its reactor
(30 s tick) once the log crosses `threshold_bytes` — visibility first; **unbudgeted, nothing
auto-compacts**.

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
    "requested_by": "user:test",
    "outcome": { "ok": true, "before_bytes": 58230, "after_bytes": 11024, "duration_ms": 22 }
  }
}
```

`status: failed` carries `payload.error` instead — a failed pass leaves the log exactly as it
was (best-effort contract; the store keeps serving on a fresh handle either way).

## 4. Confirm the shrink

Call `store.status` again: `log_bytes` now tracks `after_bytes` (plus a little engine
bookkeeping), and `last_compaction` records the pass.

## 5. The disk budget: let the node compact itself

Everything above is operator-driven. A node given a **disk budget** does step 2 for itself once
the log crosses a mark. Nothing changes unless you set the allowance.

### 5.1 Set the allowance

```bash
LB_STORE_MAX_BYTES=8388608      # a PLAIN BYTE COUNT. Not "8MB", not "8Mi" — those are malformed.
```

Read once, at the binary boundary (`node/src/config.rs::store_budget_bytes_from_env` →
`BootConfig::store_budget_bytes`). Malformed or overflowing ⇒ a warn on stderr and **no budget** —
boot never panics over a typo. An embedder fills the `BootConfig` field directly; only the binary
reads env.

From the allowance the node derives two marks and the advisory threshold:

| | |
|---|---|
| soft mark | **80%** of budget — also becomes `threshold_bytes`, so the advisory and the action fire at the same point |
| hard mark | **95%** of budget |
| headroom | `budget - log_bytes`, saturating at 0 |

**Unset ⇒ `None` ⇒ no marks at all.** `threshold_bytes` stays the flat 256 MiB advisory,
`budget_bytes`/`headroom_bytes` are `null`, and **nothing auto-triggers, ever** — at any log size.
That is the upgrade-changes-nothing property; do not assume a node compacts itself unless someone
set the number.

### 5.2 Read the headroom

Same `store.status` verb, same `store:status:read` cap — budgeted, the four budget fields carry
values:

```jsonc
{
  "log_bytes": 6733387,
  "budget_bytes": 8388608,
  "headroom_bytes": 1655221,     // 1.6 MiB left
  "threshold_bytes": 6710886,    // == the soft mark, because a budget is set
  "advisory": "store commit log is 6733387 bytes (threshold 6710886): boot replays every byte…",
  "free_disk_bytes": null
}
```

Poll this — disk fills over days, and the trend is the point. `headroom_bytes: 0` means the log is
at or past the whole allowance (it saturates; it never goes negative).

**`free_disk_bytes` is always `null` today.** The field, its serialization and its call site ship,
but the figure needs a `statvfs`-class syscall and no filesystem-stat crate is a direct dependency
of the workspace — so it is honestly absent rather than guessed. Known follow-up. Until it lands,
the budget bounds the **store directory** only: extension artifacts, sidecar binaries and OS logs
share the filesystem and are outside the number. Check real free space with `df` yourself, and
leave room — a compaction rewrites the log and needs somewhere to put it.

### 5.3 The soft mark (80%): the automatic pass

Past the soft mark the reactor enqueues **one** `store.compact` job — the same job, the same
drain, the same outcome record an operator's trigger produces. One crossing, one job, in the
node's configured workspace; never a fan-out (the pass is node-global and each one quiesces every
write on the node).

You see an `INFO` line, then the pass:

```text
INFO store crossed the disk soft mark — enqueued an automatic store.compact
     log_bytes=6733387 job_id=store-compact-01KYS1E616W7Q5H099QRP3819K soft_mark_bytes=Some(6710886)
INFO store compaction pass complete ws=ops job=store-compact-01KYS1E616W7Q5H099QRP3819K
     before_bytes=6734316 after_bytes=436237 duration_ms=76
```

Rate-limited to **one automatic pass per hour**. An operator can still trigger a pass at any time;
the interval only governs the driver's own.

### 5.4 Who asked? `requested_by`

A budget-driven job names the driver, not a person:

```jsonc
{
  "status": "done",
  "payload": {
    "requested_by": "system:store-budget",   // NOT a principal — the budget driver caused this pause
    "outcome": { "ok": true, "before_bytes": 6734316, "after_bytes": 436237, "duration_ms": 76 }
  }
}
```

`"system:store-budget"` is a literal that no real principal can hold. Reading a job record, that
one field tells you whether a human or the disk budget quiesced your writes.

### 5.5 The hard mark (95%): exempt from the interval

If the log keeps growing to 95% of budget, the node compacts **again immediately, even inside the
hour**, and says so at `WARN`:

```text
WARN store crossed the HARD disk mark — compacting now, exempt from the minimum interval
     (on an append-only engine only a compaction frees bytes). Writes pause for the pass.
     log_bytes=8039608 job_id=store-compact-01KYS1EHR6Z4PJKXEQ1Q86JN8K hard_mark_bytes=Some(7969177)
```

The exemption is not a tuning knob — it is what keeps the budget safe on an append-only engine,
where a delete *adds* bytes (a tombstone) and only a compaction ever gives them back. Without it
the sequence 80% → compact → still growing → 95% → retention evicts → compaction blocked by the
interval → budget blown is real.

What the hard mark does **not** do:
- **It never refuses a write.** No ceiling in this design says no; that needs a failure contract
  for every producer and is a different scope.
- **It never evicts.** Eviction is the routine 300 s retention tick's job; doing it here would add
  bytes at the worst possible moment.

### 5.6 "budget too small for this workload"

When a pass returns `after_bytes > 0.9 × before_bytes`, compaction reclaimed essentially nothing —
the **live set itself** is the budget, and re-compacting hourly would be a recurring write outage
for zero bytes. The driver stops auto-enqueueing and says so at the *soft* mark, every tick:

```text
WARN store is over the soft mark but compaction reclaims almost nothing — budget too small for
     this workload; not auto-compacting (raise LB_STORE_MAX_BYTES or tighten retention).
     Auto-passes resume once a pass reclaims again.  log_bytes=6743426 budget_bytes=Some(8388608)
```

This is information, not a failure. Your options, in order:
1. **Raise `LB_STORE_MAX_BYTES`** if the disk has the room (`df`).
2. **Tighten retention** — shrink the horizon / `max_samples` so the live set actually shrinks.
   Remember the ordering: eviction grows the log; the next compaction is what frees it.
3. **Move data off the node.** No knob makes a live set smaller than the data in it.

The suspension **lifts by itself** the next time *any* pass pays — the driver folds in every
compaction outcome, so an operator's productive manual pass resumes automatic ones just as the
driver's own would. A suspended driver stays suspended at the hard mark too: a pass that reclaims
nothing at 80% reclaims nothing at 95%, and the exemption exists to beat the clock, not this.

## Grounding — the budget flow (live run, 2026-07-30)

A real node (real SurrealKV dir, the real `spawn_store_compact_reactors`, real records through the
real write path, tick shortened to 1 s), `LB_STORE_MAX_BYTES=8388608` ⇒ soft 6710886, hard 7969177.

**Productive workload** (keys rewritten, so the log bloats over a small live set):

```text
LIVE status@boot:               log_bytes=523      headroom_bytes=Some(8388085) advisory=None free_disk_bytes=None
LIVE status@soft-mark-crossed:  log_bytes=6733387  headroom_bytes=Some(1655221) advisory=Some("store commit log is 6733387 bytes (thres…")

WARN store commit log is 6733387 bytes (threshold 6710886): boot replays every byte of it — run store.compact …
INFO store crossed the disk soft mark — enqueued an automatic store.compact log_bytes=6733387
     job_id=store-compact-01KYS1E616W7Q5H099QRP3819K soft_mark_bytes=Some(6710886)
INFO store compaction pass complete ws=ops job=store-compact-01KYS1E616W7Q5H099QRP3819K
     before_bytes=6734316 after_bytes=436237 duration_ms=76

LIVE status@after-auto-pass:    log_bytes=437768   headroom_bytes=Some(7950840) advisory=None
LIVE job: id=store-compact-01KYS1E616W7Q5H099QRP3819K status=Done
     payload={"requested_by":"system:store-budget","outcome":{"ok":true,"before_bytes":6734316,
              "after_bytes":436237,"duration_ms":76,"error":null}}

# kept writing, past 95% — the hour-old interval does NOT block it:
LIVE status@hard-mark-crossed:  log_bytes=8039608  headroom_bytes=Some(349000)
WARN store crossed the HARD disk mark — compacting now, exempt from the minimum interval …
     log_bytes=8039608 job_id=store-compact-01KYS1EHR6Z4PJKXEQ1Q86JN8K hard_mark_bytes=Some(7969177)
INFO store compaction pass complete before_bytes=8040259 after_bytes=437040 duration_ms=83
LIVE total store-compact jobs: 2   (both requested_by system:store-budget)
```

**Unproductive workload**, same budget, 3100 unique keys — the live set *is* the budget:

```text
LIVE too-small: log_bytes=6740809 (all 3100 keys live)
INFO store crossed the disk soft mark — enqueued an automatic store.compact job_id=…RAFY0NRJNGRK5E5C9NC6
INFO store compaction pass complete before_bytes=6741738 after_bytes=6741889 duration_ms=148
WARN store is over the soft mark but compaction reclaims almost nothing — budget too small for this
     workload; not auto-compacting … log_bytes=6743426 budget_bytes=Some(8388608)
     (repeated every tick for the next 30 s)
LIVE too-small: total store-compact jobs = 1     ← one pass, then never again
```

**Pause at real scale** (`crates/store/tests/compaction_pause_measure_test.rs`, release, NVMe):
`before_bytes=2161424374` (2.06 GiB) → `after_bytes=16903331` (16 MiB), `duration_ms=771` — a 128×
reclaim in under a second. The pause scales with the **live set** being rewritten, not the log being
discarded. On SD-card media expect ~1–2 s; watch `duration_ms` on the job record.

## Grounding (live run, 2026-07-15)

From the real end-to-end run in `crates/host/tests/store_admin_test.rs::compact_job_enqueues_drains_and_records_outcome`
(a real SurrealKV dir, real churn, the reactor's own drain fn):

```text
GROUNDING: status before: log_bytes=57308 advisory=none
GROUNDING: store.compact -> job store-compact-01KXJCC803WX7WGVPV3DJXFTJ5
GROUNDING: job record payload: {"requested_by":"user:test","outcome":{"ok":true,
  "before_bytes":58230,"after_bytes":11024,"duration_ms":22,"error":null}}
GROUNDING: status after: log_bytes=12464, last_compaction.ok=true
```

## 6. The boot memory guard: when boot declines, or refuses (issue #128)

Everything above bounds bytes on **disk**. Boot also has a **memory** limit, and on a small box it
is the one that bricks the machine: the boot pass resolves the live set into RAM and the open then
replays the log to build the index. Since #128 the node decides both, from this machine's
`MemAvailable`:

| decision | rule | what you see |
|---|---|---|
| skip the boot pass | `log_bytes > 0.5 × available RAM` | WARN + `last_compaction.skipped` |
| skip the boot pass | last **persisted** pass reclaimed ~nothing (`after > 0.9 × before`) **and** log ≤ `1.25 × after` | WARN + `last_compaction.skipped` |
| refuse to open | `log_bytes > 1.0 × available RAM` | nonzero exit + the `WontFit` diagnostic on stderr |

Unmeasurable memory (no readable `/proc/meminfo`) ⇒ **no guard at all**, today's behaviour. Nothing
overrides the skips (skipping a pass is always safe); the *open* guard is overridable — see 6.3.

### 6.1 Read the persisted record

The outcome of every pass that actually **ran** is written beside the store as
`<store dir>/../last-compaction.json` (atomic tmp+rename). It is a *sibling* of the engine directory
on purpose: `log_bytes` and the disk-budget marks never count it. Read it directly when the node is
down — that is exactly when you need it:

```bash
cat /var/lib/lb/last-compaction.json
# {"at_epoch_ms":1785565525582,"ok":true,"before_bytes":10800,"after_bytes":10984,
#  "duration_ms":55,"error":null,"skipped":null}
```

A **skip is never persisted** — the file holds the last pass that ran, because that is what the next
boot's "is a pass worth it" precondition reads. The skip itself is in `store.status`.

### 6.2 Interpret a skip

```jsonc
// store.status  →  last_compaction
{
  "ok": false,               // nothing was compacted…
  "error": null,             // …and nothing failed: a skip is a DECISION
  "skipped": "log 617000000 bytes exceeds 50% of available RAM (802000000 bytes) — …",
  "before_bytes": 617000000, // == after_bytes: the log is untouched
  "after_bytes": 617000000
}
```

`ok: false` + `error: null` + `skipped: <reason>` is the signature. What it means and what to do:

- **Headroom skip** — the box is too small for a pass over this log. The node is up and serving on
  an uncompacted log; boot is slower, nothing is at risk. Fix the *cause*: tighten retention so the
  live set shrinks, or run `store.compact` **online** (section 2) — the guard does not gate the
  online pass, and a failed pass there costs a job, not the box.
- **Unproductive skip** — a previous pass reclaimed essentially nothing, i.e. the log **is** the
  live set. Compacting again would be an expensive no-op. Same remedy as "budget too small for this
  workload" (5.6): raise the allowance, tighten retention, or move data off the node. The skip lifts
  by itself once the log grows past `1.25 ×` the last pass's `after_bytes`.

The `#122` budget driver **re-seeds** its own suspension from the persisted record at boot, so a
node that has proven compaction does not pay here no longer forgets it on every restart.

### 6.3 Recover a refused open

A node that refuses to open exits nonzero in milliseconds and says exactly why:

```text
store at /var/lib/lb/store will not fit in memory: the commit log is 11426 bytes and only 1024
bytes of RAM are available, so replaying it would likely OOM this machine. Refusing to open (this
is a heuristic guard). Remedies: add RAM or swap; compact the store on a larger machine; lower
retention so the next compaction shrinks the live set; or, if you know it fits, set
LB_STORE_OPEN_UNGUARDED=1 to force the open.
```

It will **not** fall back to an empty `mem://` store — a node silently serving a workspace that
"lost" its data is worse than a down node with a legible reason. Under a restarting unit this loops
on a `stat` instead of on 90 s of allocation, so ssh stays usable. In order of preference:

1. **Give it memory** — add swap or raise the box/cgroup, then start normally.
2. **Compact elsewhere** — copy the store directory to a bigger machine, open it there once (the
   boot pass runs and shrinks it), copy it back.
3. **Tighten retention first**, then do (2) — otherwise the pass reclaims nothing.
4. **Force it**, only if you know it fits (you added swap, or you measured):

```bash
LB_STORE_OPEN_UNGUARDED=1 systemctl start rubix-ai     # exactly "1"; anything else warns and the guard STAYS ON
```

The override disables the **open** guard only, and warns loudly on stderr while it does. Embedders
can instead supply a truer ceiling than the host's figure — a cgroup limit — through
`BootConfig::store_available_ram_bytes`.

### 6.4 Grounding — the boot-guard flow (live run, 2026-08-01)

`cargo test -p lb-host --test store_boot_guard_test -- --ignored --nocapture` (a real SurrealKV dir,
real records through the real write path, the real `store.status` verb; the RAM figure is injected
so the gigabyte decisions are reproducible on a workstation):

```text
LIVE boot(normal):   log_bytes=11212 last_compaction=Some(CompactionRecord { at_epoch_ms: 1785565525582,
                     ok: true, before_bytes: 10800, after_bytes: 10984, duration_ms: 55, error: None,
                     skipped: None })
LIVE persisted record: Some(CompactionRecord { .. same record .. })
LIVE sidecar path:   /tmp/lb-host-bootguard-live-01KYXZZDAZC27QAQ9NYV7587NX/last-compaction.json

LIVE boot(skipped):  log_bytes=11426 skipped=Some("log 11212 bytes exceeds 50% of available RAM
                     (16818 bytes) — the boot compaction pass can peak at more than the log size
                     (measured 0.26x on a fat-record store, ~1.4x on a key-dense one) and could OOM
                     this machine; skipping it and opening on the uncompacted log")

LIVE boot(refused):  store at …/store will not fit in memory: the commit log is 11426 bytes and only
                     1024 bytes of RAM are available … set LB_STORE_OPEN_UNGUARDED=1 to force the open.

LIVE boot(override): opened, log_bytes=11640
```

Note the skipped boot still **opened and served** — that is the expected outcome on a box like the
incident's (617 MB log, 802 MB available): pass declined, open allowed, degraded boot speed, no
drama. Refusal is reserved for the genuinely hopeless case.

**What a pass costs, measured at GB scale** (1.34 GB log / 867 MB live set, fat records): peak RSS
**0.26 ×** the log with the pass, **0.11 ×** with it skipped, and boot wall time 9.7 s → 3.4 s. The
ratio is record-size dependent (SurrealKV's boot memory tracks the *index*, not the values) — the
incident's key-dense store peaked at ~1.4 × its log, which is why the constants are set where they
are. Details: `sessions/store/boot-memory-guard-session.md`.

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
  action, deliberately NOT implied by `store:*:write`. The budget adds **no verb**: it is boot
  config, not a record, precisely so nobody can raise the ceiling through an MCP call.
- **The budget is node-scoped**, never per-workspace. One allowance for the whole node's store.
- **The boot guard is not the disk budget.** It never evicts, pauses or bounds anything: it turns
  one guaranteed-fatal boot into a diagnostic. `LB_STORE_MAX_BYTES` (the disk half) is what keeps a
  node from ever getting near it — set both.
- **The reactor mints no principal.** A budget-driven pass is node maintenance below the namespace
  wall — the capability gate is what an *operator's* `store.compact` goes through.
