# Observability scope — the telemetry console (in-store capped sink + in-browser viewer)

Status: scope (the ask). Promotes to `public/observability/` once shipped. Stage: **S10 —
cross-cutting retrofit** (`../../STAGES.md`), the **consumer half** of observability. The
**emission** half — the `tracing` vocabulary, the `trace_id` that survives the routed hop, the
`Secret<T>` redaction — is scoped in [`observability-scope.md`](observability-scope.md) and is a
**hard dependency**: this scope adds a *sink* and a *view*, it does **not** re-invent emission.

> Read with: [`observability-scope.md`](observability-scope.md) (emission — the signal this stores
> and shows), [`../audit/audit-scope.md`](../audit/audit-scope.md) (the immutable mutation ledger the
> console reads **alongside** telemetry — the "log the user that deletes/updates" half), README §6.5
> (the dispatch chokepoint that emits), §6.6 (the cap decision the viewer filters on), §6.7 (secrets
> — nothing secret reaches the store), §6.13 (the gateway SSE route the live tail rides), §3.2 (one
> datastore — the capped sink is *bounded* SurrealDB, not a second store), §3.3 (state vs motion).

The emission scope deliberately stopped at "**emit** clean OTLP and let an external Tempo/Loki/Grafana
stack collect and visualize it." That is correct for an operator with a monitoring stack — and wrong
for the common case this platform must also serve: **an operator or a workspace admin who opens the
shell and wants to see, right now, what an extension or a tool just did — with no external stack stood
up at all.** This scope makes the platform **self-contained for everyday observability**: a bounded,
FIFO-capped SurrealDB sink that any node writes to, a gated workspace-walled read surface over it, and
an in-browser **telemetry console** with first-class filters (by extension, tool, actor, level,
`trace_id`, time). OTLP export stays a **peer sink** for heavy/long-retention analysis — both are just
`tracing-subscriber` layers chosen by config; neither is privileged in the emission path.

## Goals

- **A bounded, FIFO-capped SurrealDB telemetry sink.** A `tracing-subscriber` **Layer** that writes
  each structured event (the emission scope's one event schema: `level`, `ws`, `actor`, `tool`,
  `trace_id`, `span`, `msg`, `fields`) as a record in a **capped** table. "Capped" = the reusable
  primitive below: keep the newest *N* per key, evict the oldest on overflow (first-in-first-out).
  The store can never grow unbounded — this is what makes "store telemetry in the one datastore"
  compatible with rule #2 (it is *bounded* operational data, not durable state).
- **One reusable capped-retention primitive** (`lb-store::capped`). A single helper —
  `capped_insert(store, table, key_selector, cap, record)` — that inserts a row and, when the count
  for that key exceeds `cap`, deletes the oldest rows for that key down to `cap`. **The key selector
  is configurable (the user's choice):** a caller passes a closure/spec that yields the FIFO key, so
  the *same* helper does **per-source** capping (per extension, per tool, per workspace — newest 1000
  *each*, so a chatty extension can't evict a quiet one) **or global** capping (newest 1000 per
  workspace across all sources). Defaults live in config, not in the helper. This primitive is generic
  — `series`, `run-events`, and any future bounded-ring table can reuse it (one verb file,
  FILE-LAYOUT-clean), it is not telemetry-specific.
- **A gated, workspace-walled read surface.** `telemetry.query` (snapshot, filter + paging) and
  `telemetry.tail` (live feed, SSE over the bus, §6.13) as MCP tools behind a new
  `telemetry:read` capability. **Reads are hard-filtered to the caller's `ws`** — the operator sink
  legitimately spans tenants, but the *tenant-facing view* never does (the boundary the emission
  scope flagged as "genuinely subtle" — this scope is where it gets enforced and tested).
- **An in-browser telemetry console with amazing filters.** A shell page (`ui/src/features/telemetry/`)
  that lists/streams events with composable filters: **source** (extension / tool / role crate),
  **actor**, **level**, **outcome** (`allow`/`deny`/`error`), **`trace_id`** (click a row → see the
  whole correlated edge→hub→job→relay chain on one timeline), **free-text** over `msg`, and a **time
  range** with live-tail toggle. Filters are URL-encoded so a view is shareable/deep-linkable.
- **User-action history, read in the same console — but from the right store.** Deletes, updates, and
  every other mediated mutation are the **audit ledger's** job (immutable, hash-chained,
  complete — [`../audit/`](../audit/audit-scope.md)), **not** this sampled/evictable sink. The console
  presents a **unified, filterable view across both stores** (operational telemetry *and* the audit
  ledger) so a user sees "who deleted what" next to "what the tool did" — but each pillar keeps its own
  store and its own guarantees. The console is a **reader of two seams, never a third store** for
  mutations. (If audit has not shipped yet, the console renders telemetry only and the audit lane is a
  clearly-labelled empty state — no fake rows.)

## Non-goals

- **Not a re-implementation of emission.** The `tracing` macros, the `trace_id` propagation over
  Zenoh, and the `Secret<T>` redaction are [`observability-scope.md`](observability-scope.md). This
  scope fails if it duplicates any of that; it consumes it.
- **Not a replacement for an external monitoring stack.** OTLP → Tempo/Loki/Prometheus stays the
  right answer for long retention, alerting, and cross-fleet correlation. The capped sink is **recent
  history**, bounded by design; it is not where you keep 90 days of traces. The console says so.
- **Not the durable mutation record.** User deletes/updates are audited in the immutable ledger;
  this sink is sampled and **will evict**. We never present the capped telemetry as the system of
  record for "who did what" — that misrepresents an evictable ring as a compliance log.
- **No new business/product analytics, no high-cardinality per-user metrics.** Same allow-listed,
  bounded label set as emission (`ws`/`role`/`tool`/`outcome`); the console filters on those, it does
  not mine behavior.
- **No secret-bearing fields, ever.** The sink writes only the already-redacted event schema; it adds
  **zero** new opportunity to capture a secret (the params are already a digest by the time they reach
  any layer). The planted-value test from emission is extended to cover the SurrealDB sink path.

## Intent / approach

**The sink is a `tracing` Layer, peer to the OTLP layer — not a fork in the emit path.** The emission
scope already establishes that the `node` binary's entry layer wires subscriber layers by config. This
scope adds **one more layer**: a `SurrealCappedLayer` that, on each event inside an instrumented span,
serializes the event schema and calls `capped_insert`. A node can run **stderr + SurrealDB**, or
**SurrealDB + OTLP**, or all three — config, not a code branch (symmetric nodes hold). The hot ingest
path is exempted / coarsened exactly as emission specifies; the capped sink honors the same head-ratio
sampling so a flood can't thrash the ring.

**The capped primitive is the load-bearing reusable piece, and it must be correct under concurrency.**
Naïve "count then delete oldest" races: two concurrent inserts both see `count == cap` and each delete
one, over-evicting. The long-term-correct implementation does the insert + trim as **one SurrealDB
transaction** (the store already has `write_tx.rs`), ordering by a monotonic insert sequence (a
per-key counter or the record's `id` ULID, **not** wall-clock — clocks skew and `Date::now` is banned
in some paths). The trim deletes `where key = $k order by seq asc limit (count - cap)`. We **reject** a
background "reaper job" as the *primary* mechanism (it lets the table overshoot the cap between
sweeps — unbounded in a burst, which defeats the entire point); a periodic compaction job is an
*optional* secondary safety net, not the guarantee. **Considered and rejected:** a SurrealDB native
TTL/`DEFINE TABLE … DROP` — SurrealDB has no built-in fixed-row-count ring, and TTL is *age*-based, not
*count*-based, so it can't express "newest 1000" (a quiet key would be deleted by age even with only 3
rows; a chatty key would blow past 1000 within the TTL window). We need count-bounded FIFO, so we own
the primitive.

**The console reads two seams and renders one timeline.** Operational telemetry and the audit ledger
are different stores with different guarantees (see the emission scope's "shared seam" table). The
console does **not** merge them into a third store; it queries both (`telemetry.query` +
`audit.query`), tags each row with its lane, and lets the unified filter span them. Clicking a
`trace_id` pivots to the correlated trace view; clicking an audited mutation pivots to that record's
before/after (the audit scope owns the diff). One view, honest provenance.

**Why in-store + in-product at all, given emission already exports OTLP.** Because "stand up Grafana
first" is a wrong default for an appliance/workstation, for a demo, for a first-run operator, and for a
**workspace admin** who must never be handed an operator's cross-tenant Grafana. The platform owning a
*bounded* recent-history view is the long-term-correct answer: it is self-contained, it is
workspace-walled at the read surface (which an external Grafana is not), and it costs one capped table.
OTLP remains for everything the capped ring deliberately isn't.

## How it fits the core

- **Tenancy / isolation:** the **read surface is the wall**. Records carry `ws` as a field; the
  operator's raw sink/console (a node-admin capability) may span workspaces by design, but
  `telemetry.query`/`tail` for a workspace principal **hard-filter to their `ws`** server-side (never
  client-side) — a ws-B caller gets zero ws-A rows. This is the precise boundary the emission scope
  named as subtle; it is enforced and tested *here*.
- **Capabilities:** a new **`telemetry:read`** grant gates the query + tail tools; deny is opaque
  (no surface, no count leak). The cross-tenant operator console is a **separate, higher** node-admin
  capability (`telemetry:read:all` or the existing operator role), never the default workspace grant.
  Emission itself still needs **no** grant (host observing itself). The audit lane reuses audit's own
  read capability — the console requires **both** grants to show both lanes.
- **Placement:** *either*. Every node can run the capped sink (an offline appliance keeps its own
  recent ring locally — this is *better* offline behavior than OTLP-only, which ships nothing until it
  reaches a collector). The console is served by the **hub** gateway like every other shell page; it
  queries whichever node's store via the routed MCP call.
- **MCP surface** (SCOPE-WRITTING §6.1 — only the verbs with a real caller):
  - **Reads:** `telemetry.query` (snapshot: filter by source/actor/level/outcome/`trace_id`/text/time,
    paged) and `telemetry.tail` (**live feed** — the right shape for "watch logs scroll", a `watch`
    tool + the gateway SSE route, **not** polling `query` on a timer). Plus `telemetry.trace` (fetch
    one correlated trace by `trace_id` for the timeline pivot).
  - **Writes:** **N/A by tool** — telemetry is written by the `tracing` **Layer**, never a
    `telemetry.write` MCP tool (there is nothing to forge; a guest cannot inject log rows). The only
    "write" is config: retention caps via prefs.
  - **Admin:** `telemetry.purge` (a gated node-admin verb to clear the ring) is the single
    destructive op; CRUD beyond that has no caller and is **not** built.
  - **Batch:** N/A — query paging covers bulk reads; there is no long-running batch, so **no job**.
- **Data (SurrealDB):** **yes, but bounded** — a `telemetry` capped table (and the console reads the
  separate `audit` table). The cap primitive guarantees an upper bound per key, so this honors rule #2
  *in spirit*: it is not a second datastore and not unbounded growth, it is a fixed-size ring inside
  the one store. Retention (`cap`, key granularity) is **config** in prefs, per role/workspace.
- **Bus (Zenoh):** **fire-and-forget only**, and only for `telemetry.tail` (a reserved
  `_lb/telemetry/**` subject the SSE route folds — mirrors `run_stream.rs`). Telemetry **never** goes
  through the outbox (it is allowed to drop). The *store write* is the durable-ish recent history; the
  *tail* is live motion. State vs motion stays clean.
- **Sync / authority:** N/A — each node's ring is independent operator/recent data, not synced
  workspace state. Correlation across nodes is by `trace_id` (from emission), not by replicating rings.
- **Secrets:** the sink writes only the pre-redacted event schema; `Secret<T>` `Debug` is `***` and
  params are already a digest before any layer sees them. The planted-value test is **extended to the
  SurrealDB sink** so a leak into the new store path fails CI.

## Example flow (open the console, watch an extension, follow a delete)

1. A workspace admin opens **Telemetry** in the shell. The page calls `telemetry.tail` over SSE
   (`?token=…`, like `run_stream`); the gateway verifies the token, checks `mcp:telemetry.tail:call`
   (**403 before any body** if missing), and the bus subject is ws-walled — a ws-B session can never
   observe ws-A.
2. They filter **source = `mqtt` extension**, **level ≥ `warn`**. Rows scroll live; each is one event
   the `SurrealCappedLayer` wrote (and the tail mirrored). The capped table holds the **newest 1000
   per the configured key** — older `mqtt` warnings have already FIFO-evicted, the count never grew
   past the cap.
3. A row shows `decision=deny` on `doc.delete`. They click its **`trace_id`** → `telemetry.trace`
   returns the correlated chain (edge click → hub dispatch → cap deny) on one timeline — the deny is
   *seen* here, while its *enforcement* lives in caps and its *immutable record* in audit.
4. They flip to the **Audit lane** (same filter bar). Because they also hold the audit read grant, the
   console queries `audit.query` and shows the authoritative, hash-chained "`user:bob` updated
   `doc:42`" entries — next to, but not mixed into, the operational telemetry. Clicking one opens the
   before/after (audit owns the diff). The telemetry ring may have already evicted the operational
   span for that update; **the audit entry has not** — that is exactly why they are two stores.
5. The same node, run offline as an appliance, still shows steps 1–3 from its **local** ring with no
   collector reachable — self-contained observability, the thing OTLP-only could not give.

## Testing plan

Mandatory categories from [`../testing/testing-scope.md`](../testing/testing-scope.md), against the
**real** `mem://` store + real bus + real gateway (no fakes, CLAUDE §9):

- **Capability-deny (§2.1):** a principal **without** `telemetry:read` gets a `403`/deny from
  `telemetry.query` and `telemetry.tail` (no rows, no count, opaque). A principal with telemetry but
  **without** the audit grant sees the telemetry lane and an empty, labelled audit lane (not an error,
  not fake rows).
- **Workspace-isolation (§2.2):** seed real telemetry rows for ws-A and ws-B; assert a ws-B
  `telemetry.query`/`tail` returns **only** ws-B rows and the ws-walled subject never delivers ws-A
  events — the read-surface wall the operator sink legitimately doesn't have.
- **The FIFO-cap test (the headline new primitive):** insert `cap + k` rows for one key, assert the
  table holds **exactly `cap`** and the **survivors are the newest** (oldest evicted, FIFO).
  - **Per-source vs global:** with a per-source key, a chatty source at its cap does **not** evict a
    quiet source's rows; with a global key, the ring is bounded across sources. Both from the **same**
    `capped_insert` helper with different selectors — proves the "configurable both" requirement.
  - **Concurrency (the correctness trap):** fire **concurrent** `capped_insert`s past the cap and
    assert the final count is **exactly `cap`**, never over-evicted nor overgrown — the single-tx
    ordering holds (this is the test that proves we didn't ship the racy count-then-delete).
- **Offline/sync (§2.3):** an offline appliance writes to and reads from its **local** ring with no
  collector; on reconnect, OTLP export resumes for *new* events (the local ring is not back-filled to
  the collector — assert that documented behavior, not exactly-once).
- **The redaction test (extended from emission — #1 risk):** plant a **known** secret value through
  the secrets surface and a tool param, drive the full dispatch→`SurrealCappedLayer`→stored-row→
  `telemetry.query` path, and assert the secret string appears in **zero** stored rows and **zero**
  query output. A different value would pass under a leak — the planted identity is required.
- **Unified-view provenance:** assert telemetry rows and audit rows render in distinct, correctly
  labelled lanes and are **not** merged into one store; an evicted telemetry span for a mutation does
  **not** remove the corresponding audit entry (proves the two-store guarantee).
- Unit: the `capped_insert` trim SQL + ordering; the filter→query codec; the SSE frame shape (reuse
  `run_stream` test scaffolding); the event-schema (de)serialization round-trip.
- UI: a `vitest` against a **real** spawned gateway (`pnpm test:gateway`, rule 9) — open the console,
  seed real telemetry rows into the real store, assert the filters narrow correctly and a live row
  arrives over SSE. No `*.fake.ts`.

## Risks & hard problems

- **The capped trim under concurrency is the part most likely to be shipped wrong.** Count-then-delete
  races into over-eviction; a reaper-only design overshoots the cap in a burst (unbounded — the exact
  failure the feature exists to prevent). The single-transaction, sequence-ordered trim is the
  non-negotiable correct path, and the concurrency test is what proves it. Treat "it usually stays
  near 1000" as a bug, not a pass.
- **Write amplification / hot-path overhead.** Every event becoming a SurrealDB write on a busy node
  is real cost. Mitigation: the capped sink honors emission's head-ratio sampling, the ingest hot path
  stays coarse (counts, not a row per sample), and the trim is amortized (trim every *m* inserts /
  batch the delete) rather than on literally every insert — **without** ever letting the table exceed
  cap by more than the batch window (a documented, bounded slack, asserted in the test).
- **The operator-sink vs. tenant-wall boundary — the leak that matters.** A "harmless" console that
  forgets to filter leaks ws-A's tool names and usage to ws-B. The filter is **server-side and
  mandatory** in `telemetry.query`/`tail`; the cross-tenant operator console is a *different, higher*
  capability. This is the same boundary emission flagged — here it is load-bearing, so it is tested
  directly, not assumed.
- **Two stores, one view — provenance confusion.** Presenting an evictable ring next to an immutable
  ledger risks a user trusting telemetry as the mutation record. The UI must **label lanes** and the
  data layer must **never** copy mutations into the ring. The unified-view test guards this.
- **Cardinality / unbounded filters.** Free-text and `trace_id` filters over a large ring need an
  index; without one a busy node's `query` scans the whole table. Define the SurrealDB indexes
  (`ws`, `trace_id`, `level`, `source`, insert-seq) up front; the cap keeps the table small enough
  that this stays cheap.
- **Audit dependency ordering.** Audit may not have shipped when this does. The console must degrade to
  telemetry-only with an honest empty state, and the unified-view code must not hard-depend on audit
  tables existing — flagged so the implementing session sequences it.

## Open questions — RESOLVED (S10, see [session](../../sessions/observability/telemetry-console-session.md))

- **Default caps + default key granularity per role.** ✅ **Resolved:** per-source default key
  (`KeySelector::PerSource`), `DEFAULT_CAP = 1000`, with a global per-ws backstop available via
  `KeySelector::Global`. The cap is config (`SurrealCappedLayer::with_cap`) — smaller on an appliance,
  larger on a hub. (Prefs-backed per-role override is a trivial follow-up; the mechanism is there.)
- **Trim cadence:** ✅ **Resolved for v1: strict (trim every insert)** — the table never exceeds the
  cap, the hard invariant the feature exists for. Amortized (every *m* inserts, a documented/tested
  slack bound) is a deferred optimization, not needed until a benchmark shows the strict trim costs on
  a busy ingest path.
- **Does `telemetry.tail` reuse the agent-run SSE plumbing or get its own route?** ✅ **Resolved: its
  own `routes/telemetry_stream.rs`** (modelled on `run_stream.rs`, sharing token-verify + ws-wall
  helpers) — one responsibility per file.
- **Cross-node console reads.** ✅ **Resolved: v1 reads the local node's ring.** A routed remote-node
  `telemetry.query` (operator debugging an edge from the hub) is **deferred**.
- **Insert sequence source.** ✅ **Resolved: the record ULID `id`** (no clock, no counter row). The
  concurrency test confirms exact-cap ordering under concurrent inserts — *with* the per-key lock +
  retry (SurrealDB's `kv-mem` raises a retryable conflict the design must handle, not just order on).
- **Should `capped` live in `lb-store` or its own tiny crate?** ✅ **Resolved: `lb_store::capped`**
  (one verb file beside `scan.rs`/`write_tx.rs`) — a reusable store primitive, not telemetry-specific.

## Related

- [`observability-scope.md`](observability-scope.md) — **the emission half, a hard dependency**: the
  `tracing` vocabulary, `trace_id` propagation, `Secret<T>` redaction, the OTLP peer sink. This scope
  is its consumer (sink + view), not a re-scope.
- [`../audit/audit-scope.md`](../audit/audit-scope.md) — the immutable, hash-chained mutation ledger
  the console reads in its second lane ("who deleted/updated what"); the durable counterpart to this
  evictable ring (the "shared seam").
- [`../undo/undo-scope.md`](../undo/undo-scope.md) — the third projection of the same chokepoint.
- README **§6.5** (dispatch chokepoint — emits), **§6.6** (cap decision — filtered/gated),
  **§6.7** (secrets/redaction), **§6.13** (gateway SSE — the tail), **§3.2** (one datastore — bounded),
  **§3.3** (state vs motion).
- `../../FILE-LAYOUT.md` — `capped` as one verb file; `routes/telemetry_stream.rs`,
  `ui/src/features/telemetry/` as folders-of-verbs.
- `key-stack.md` — the "Observability/audit" row (this + emission resolve its "needs a scope" note).
- Code anchors: `rust/crates/store/src/{write_tx,scan,tables}.rs` (the cap primitive's home),
  `rust/role/gateway/src/routes/run_stream.rs` (the SSE pattern to mirror),
  `rust/node/src/main.rs` (the entry layer that wires subscriber layers by config).
