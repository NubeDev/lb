# Observability — structured logs, traces, metrics, and the telemetry console

The **emit** half (the full `tracing` vocabulary, `trace_id` propagation across the routed Zenoh
hop, OTLP export) is still scoped in
[`observability-scope.md`](../../scope/observability/observability-scope.md). What has **shipped**
(S10) is the **consumer** half plus the minimal emit slice it depends on — the in-product,
self-contained telemetry console:
[`telemetry-console-scope.md`](../../scope/observability/telemetry-console-scope.md).

## What shipped — the telemetry console

A bounded, FIFO-capped SurrealDB telemetry sink + a gated, workspace-walled read surface + an
in-browser console. No external monitoring stack required for everyday "what did that tool just do?".

### The capped-retention primitive — `lb_store::capped_insert`

`capped_insert(store, ws, table, id, cap_key, cap, value)` inserts one row and trims its FIFO key
back to the newest `cap` rows, in **one SurrealDB transaction**, ordered by a ULID insert-seq (no
wall-clock, no counter row). The key selector is the caller's: per-source (newest N per
extension/tool — a chatty source can't evict a quiet one) **or** global (newest N per workspace).
The same helper does both. Generic — reusable by any bounded ring (`series`, `run-events`), not
telemetry-specific.

**Correctness under concurrency is the load-bearing property** and took three layers to get right:
the single transaction, a per-`(ns,table,cap_key)` in-process lock (so concurrent same-key trims
don't each compute a stale `$keep` and over-evict), and a bounded retry on SurrealDB's retryable
write-conflict (so a conflict with a concurrent reader/cross-key writer doesn't silently drop the
row). The concurrency test fires 100 inserts at 5× cap and asserts the final count is **exactly**
`cap` — deterministically. (See
[debugging](../../debugging/observability/capped-insert-overgrows-cap-under-concurrency.md).)

### The sink — `SurrealCappedLayer`

A `tracing-subscriber` **Layer**, peer to the stderr/OTLP layers, config-selected in
`node/src/main.rs` via `LB_TELEMETRY_SINK` (`stderr` | `surreal` | `both` | `off`) — no `if cloud`.
It writes only events with `target == lb.telemetry` (so SurrealDB's own internal logs never pollute
the ring), serializes the redacted event schema (`level`/`ws`/`actor`/`tool`/`source`/`trace_id`/
`outcome`/`ts`/`msg`/`params_digest`/`fields`), and fire-and-forgets a `capped_insert` + a publish
onto the ws-walled tail subject. Params arrive only as a `params_digest` (SHA-256 + shape) — the raw
value, and any `Secret<T>`, can never reach a row.

### The read surface — gated, workspace-walled MCP tools

Behind a new `telemetry:read` capability, all **hard-filtered to the caller's `ws` server-side**:

- `telemetry.query` — snapshot, filter by source/actor/level(≥)/outcome/trace_id/text/time, seq-paged.
- `telemetry.tail` — the live feed; rides the SSE route `routes/telemetry_stream.rs` (modelled on
  `run_stream.rs`), not an in-band tool result. 403 before any body if the grant is missing.
- `telemetry.trace` — one correlated trace by `trace_id` (the timeline pivot).
- `telemetry.purge` — node-admin, the single destructive verb.

There is **no `telemetry.write`** — writes come from the Layer only. Deny is opaque and
capability-first (the gate fires before the bridge). Cross-tenant operator reads are a separate,
higher capability — never the default workspace grant.

### The console — `ui/src/features/telemetry/`

A filter bar (source / actor / level / outcome / trace_id / free-text / live-tail toggle), a
newest-first event list (click a row's `trace_id` → the correlated timeline), and a second **Audit
lane**. Filters are URL-encoded (shareable). The audit lane reads the immutable, hash-chained
mutation ledger as a **separate store** — never merged into the evictable telemetry ring. Audit has
not shipped, so the lane is a clearly-labelled "unavailable / needs-grant" state — **never fake
rows** (the two-store provenance guarantee).

## Resolved scope open questions

- **Default caps + key granularity:** 1000/source (workstation) as the default per-source key, a
  global per-ws backstop available via `KeySelector::Global`; caps are config (`with_cap`), smaller
  on an appliance, larger on a hub.
- **Insert-sequence source:** the record ULID `id` (monotonic-ish, lexicographically sortable, no
  clock, no counter row).
- **`capped` home:** `lb_store::capped` (one verb file beside `write_tx`/`scan`/`tables`), not its
  own crate — it's a reusable store primitive.
- **`telemetry.tail` route:** its own `routes/telemetry_stream.rs`, sharing the token-verify +
  ws-wall helpers; not `run_stream` reuse.
- **Cross-node reads:** v1 reads the **local** node's ring; remote-node reads defer.
- **Trim cadence:** strict (trim every insert) for v1 — the table never exceeds the cap. Amortized
  trim (every *m* inserts, a documented/tested slack bound) is a deferred optimization.

## Still planned (emit half)

The full trace propagation across the routed Zenoh hop, the OTLP exporter as a peer sink for long
retention, and metrics (tool latency, capability-deny count, sync lag, outbox retries) remain
scoped in `observability-scope.md`. The console is one of three projections of the host chokepoint —
see the audit and undo docs.
