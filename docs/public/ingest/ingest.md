# Ingest — buffered read/write surface for high-volume external data (as built, S9)

The generic, capability-gated surface for absorbing high-volume external data (sensor streams, app
metrics, edge-node reports) into workspace-scoped **time-series state**, via a durable ingest buffer
(the read-side analog of the outbox). **A "device" is just a principal on a node** — IoT is one caller,
never a core concept. No `device`/`sensor`/`firmware`/`MQTT` appears anywhere in `lb-ingest` or the host
ingest module; protocol bridges live out-of-core.

## The Sample envelope

`Sample { series, producer, ts, seq, payload, labels, qos }`. `payload` is any SurrealDB-typed value
(scalar → number/bool, structured → nested object, binary → record-as-content, since the store spike
marked `DEFINE BUCKET` unavailable). The **dedup identity is `(series, producer, seq)`** — `producer`
is the **authenticated calling principal** (the host overwrites the wire value; un-spoofable). Keying on
`(series, seq)` is rejected (it loses data across producers). `qos` is `best-effort` (lossy by design)
or `must-deliver` ("never lost until on disk").

## The path: cheap append → batched exactly-once commit → typed read

1. `ingest.write(samples)` — durable **APPEND** into `ingest_staging` at `[series, producer, seq]`. The
   cheap path: no indexes, no edges, no rollup maintenance. Bounded; overflow honored at the staging end
   (drop-oldest for best-effort, dead-letter for must-deliver).
2. **Commit worker** (`drain_workspace`, mounted by the **ingest role** — config, no `if cloud`) drains a
   batch and commits it in **ONE transaction**: UPSERT into `series` on `[series, producer, seq]` **and**
   delete the staged row, same tx. So a die-mid-batch rolls back atomically, and a restart re-drain
   commits each sample **exactly once** (no double-commit).
3. `series.read(series, from?, to?)` / `series.latest(series)` — range / newest over the committed series
   (ordered by `seq`, never wall-clock `ts`). Open bounds are omitted, never a `u64::MAX` sentinel.
4. `series.find(facets)` — tag-driven discovery over the tag graph (built on `lb-tags`), returning the
   matching `series:` entities.

## MCP surface

`ingest.write` · `series.read` · `series.latest` · `series.find` — host-native tools, each gated by
`mcp:<verb>:call` (workspace-first, then capability); denials are opaque (no existence signal).

## Guarantees (and their scope)

- **Exactly-once across a hard kill** — proven by a subprocess test that SIGABRTs mid-flight and asserts
  exactly-once re-drain + atomic rollback. "Never lost until on disk" applies to **`must-deliver`** only;
  `best-effort` is lossy by design.
- **Two-producer collision** — producer-A and producer-B both writing `seq=5` to one series both survive.
- **Workspace wall** — a ws-B producer cannot write or read a ws-A series; staging is workspace-partitioned.

## Not yet built (named in scope)

Producer-side staging bound + rate-limiting; retention/GC (raw→rollup aging); the one-authoritative-ingest
sub-hub wiring; the checkpointed-ring throughput optimization.

See `scope/ingest/ingest-scope.md` for the ask and `sessions/ingest/ingest-session.md` for the build log.
