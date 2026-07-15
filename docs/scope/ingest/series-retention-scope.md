# Ingest scope — series retention + rollup tiers + GC (the table stops growing forever)

Status: **shipped** (issue #58, `series-plane-readiness`) — see
[`../../sessions/ingest/series-plane-readiness-session.md`](../../sessions/ingest/series-plane-readiness-session.md).

The ingest scope names retention/downsampling but deferred the mechanism; until this slice there was
no retention, no rollup, no eviction — the `series` table grew without bound (protocol extensions
explicitly delegate retention to the host: "host data, host retention policy"). This slice adds a
**per-series-prefix retention policy** (raw → downsampled tiers → evict), workspace-scoped,
administered through new capability-gated `series.retention.*` verbs, executed by an on-demand
**rollup-then-evict GC pass**.

## The shape

- **Policy** (`series_retention` table, one row per series-name prefix):
  `{prefix, raw_for_ms, tiers: [{width_ms, keep_for_ms}]}`. `raw_for_ms = 0` disables eviction;
  `keep_for_ms = 0` keeps a tier forever.
- **Verbs** (host-native MCP, each gated `mcp:<verb>:call`, admin-tier caps):
  - `series.retention.set` — upsert a policy (also gates `series.retention.delete`; deleting is the
    same administrative privilege, no new cap minted);
  - `series.retention.list` — the workspace's policies;
  - `series.retention.gc {now_ms?}` — run one pass now (`now_ms` caller-injectable, determinism §3;
    wall-clock only as the omitted-arg fallback at the tool layer).
- **GC pass** (`lb_ingest::run_gc`): for each policy × matching series — fold raw samples older than
  the horizon into each tier's buckets (stored `series_rollup` rows carrying
  `{min, max, sum, num_count, count, last, last_ts}` so later re-aggregation is exact, never a
  mean-of-means), then delete the raw rows, then evict each tier's own stale rows. The cutoff is
  **snapped down to the widest tier's bucket boundary** so only complete buckets ever roll up — a
  later pass never re-aggregates a half-evicted bucket.
- **Reads compose:** `series.read {mode:"buckets"}` merges the finest stored rollup tier into any
  window raw no longer covers, so a chart over evicted history still renders (decimation scope). Raw
  `mode:"rows"` reads simply no longer see evicted samples.

## Decisions (and the rejected alternatives)

- **On-demand GC verb, not a background worker.** The ingest-role node (or an embedder's scheduler /
  a rule) calls `series.retention.gc` on its own cadence — mounting is config, never a role branch
  (rule 1), and tests inject `now_ms` for determinism. A built-in timer loop was rejected for v1: it
  adds a lifecycle to supervise and hides the clock. Revisit when an embedder asks.
- **Stored rollup tiers ARE allowed here — reconciling the decimation scope.** The decimation slice
  rejected *materialized rollups as a read-time cache* (read-time `GROUP BY` is fast enough over
  indexed raw). Retention is the different case: after eviction the raw is GONE, so the tier is the
  **only** surviving copy, not a cache. Tiers live in SurrealDB (rule 2 — one datastore).
- **Sum+count travel with every rollup row** so bucketed reads re-aggregate exactly.
- **Workspace-scoped everything**: policies, rollups, GC — the hard wall (rule 6). A ws-B `gc`
  physically cannot touch ws-A rows; deny + isolation tests are mandatory and shipped.

## Testing (shipped, green)

`rust/crates/ingest/tests/series_plane_test.rs` (rollup-then-evict, exact re-aggregation, tier
eviction, idempotent second pass, rollup-backed bucket reads) and
`rust/crates/host/tests/series_plane_host_test.rs` (MCP round-trip, **capability-deny on every
verb**, **workspace isolation** for policies and GC).

## Follow-up: the count axis, the driver, and the default

*(Scoped 2026-07-15: [`series-sample-cap-scope.md`](series-sample-cap-scope.md).)* This slice bounds
a series by **time**; three gaps remain that mean a disc can still fill:

1. **No count bound.** `raw_for_ms` says "how old is too old" — but bytes are set by **rate**, which
   the producer chooses. 24h is 864k samples at 10Hz and 86.4M at 1000Hz. A `max_samples` FIFO cap
   adds the axis an operator can actually multiply (measured: ~700 bytes/sample).
2. **`run_gc` has no driver.** It is called only by tests and the on-demand `series.retention.gc`
   verb — **nothing ticks it at boot**, so the "on-demand GC, not a background worker" decision below
   means retention evicts *nothing* on a real node unless someone calls the verb by hand. Same
   missing-driver class as [`drain-backpressure-scope.md`](drain-backpressure-scope.md). The follow-up
   ships `spawn_retention_reactors`, which revisits that decision — an embedder has now asked.
3. **Keep-forever default.** No policy → nothing evicts, ever. A fresh node fills its disc with zero
   config and zero warning.

## Open questions

- ~~A scheduled GC (jobs/rules integration) vs the current caller-cadence verb.~~ **Answered:** the
  caller-cadence verb alone means retention never runs. A reactor is scoped in the follow-up above.
- Per-tier `last` fidelity across re-tiering (today: `last` of the finest tier wins).
- Should `series.retention.gc` stream progress for very large workspaces (it is batch-bounded
  internally via the keyset scan, but returns one summary)?

## Related

- [`ingest-scope.md`](ingest-scope.md) — the parent surface (§ Retention/GC open question, now closed).
- [`../datasources/series-decimation-scope.md`](../datasources/series-decimation-scope.md) — slice C;
  bucketed reads merge these tiers post-eviction.
- [`../datasources/series-paging-scope.md`](../datasources/series-paging-scope.md) — slice B; the
  keyset scan GC folds over.
