# Federation read concurrency — same-source reads must not serialize

**Topic:** `datasources` · **Name:** `federation-read-concurrency` · **Status:** scoped

A dashboard open fans N panel queries at one datasource. The transport is concurrent
(`native-call-concurrency`, shipped — measured: 8 parallel `SELECT 1` = 15 ms wall), the batch verb
fans out under a 16-permit semaphore (`viz/batch.rs`) — and then every same-source read funnels into
**one connection**, so the fan-out collapses back to a serial staircase. Measured live (2026-07-31,
seeded `demo-buildings` sqlite, 5.8 M-row `point_reading`, warm pool, result cache off):

| Path | Measured | Notes |
|---|---|---|
| 1 panel query alone | ~230–300 ms | DataFusion plan + sqlite scan — the honest unit cost |
| 4 identical-cost scans, parallel | 228 / 444 / 654 / 865 ms — wall 867 ms | a pure staircase: each waits for the one before |
| 10 panels fully parallel | wall 3,529 ms | ≈ the serial sum (3,646 ms) |
| `viz.query_batch`, 10 panels | 3,491–3,508 ms, 5 runs | the 16-way semaphore buys **nothing** |
| 8 × `SELECT 1` parallel | wall 15 ms | control: transport + runtime are NOT the ceiling |

The choke point is `SqliteSource` (`crates/federation/src/source/sqlite.rs`): the upstream
`datafusion-table-providers` `SqliteConnectionPool` is a **single tokio-rusqlite connection**, and
every cached `TableProvider` is bound to it. `federation/src/main.rs` already records the earlier
half of this hunt ("linear serial staircase … transport serialization alone") — the transport half
was fixed; this is the remaining half, one layer down. This is also why the pool/result caches never
made a *cold board* fast: cold open ≈ warm open ≈ 3.85 s for a 17-target board, because the term
that dominates is `N × unit-cost, serialized`.

## Goals

- **N concurrent reads on one source cost ~max, not ~sum.** The acceptance number: the 4-scan
  staircase above becomes wall ≈ slowest-single (±50%), and a 10-panel `viz.query_batch` on one
  sqlite source lands under ~1 s where it measures ~3.5 s today.
- **Bounded.** A source never holds more than a small fixed number of read connections; many
  registered sources cannot fd-exhaust the child.
- **Generic (rule 10).** The invariant is per-`Source`, not per-kind: the trait contract becomes
  "concurrent `query` calls on one source may proceed concurrently, bounded by the source". The
  sqlite source is the one that violates it today and the one this scope fixes; the postgres source
  already rides a real multi-connection pool and only inherits the test.

## Non-goals

- No query-cost work (indexes, decimation, pushdown changes) — the ~240 ms unit cost is a separate
  concern; this scope removes the ×N serialization multiplier only.
- No change to `viz.query_batch`, the result cache, the pool cache keying, or the wire contract of
  any verb. Zero API surface change.
- No upstream fork/patch of `datafusion-table-providers` — the fix composes K instances of its pool
  rather than rewriting it (see Rejected).
- No write-path concurrency. `federation.write`/`delete`/`migrate` keep their current single-writer
  behavior — sqlite has one writer by design, and interleaving writes is a correctness question this
  scope must not touch.

## Intent / approach

**K read slots per sqlite source, round-robined per query.** `SqliteSource` holds
`slots: Vec<ReadSlot>` (K = 4) instead of one pool, where each slot is its own
`SqliteConnectionPool` (its own OS connection) plus its own per-table provider cache:

- `connect()` builds slot 0 eagerly (today's behavior, same validation/error) and the remaining
  K−1 **lazily on first use** — a source that only ever sees serial traffic never pays for 4
  connections.
- `query()` takes `slot = counter.fetch_add(1) % K` (one `AtomicUsize` on the source) and resolves
  its `TableProvider`s from that slot's cache, building them against that slot's pool on miss —
  providers stay bound to exactly one connection, which is what makes this safe with zero upstream
  change. The provider-per-table build cost (measured ~0.4–1.2 s, first touch only) is paid at most
  K× per table; after warm-up, all K slots serve scans concurrently.
- **K = 4, a constant.** Rationale: the child runs 4 worker threads (`main.rs`, documented choice)
  and the scan work is CPU+IO on both ends; 4 read connections saturate that without oversubscribing
  a node also running host/store/bus. Not env-tunable — a knob here is a support surface for a
  number nobody should be retuning per site; revisit only with a measurement that says 4 binds.
- Multiple sqlite sources compose safely: the existing pool cache (`pool.rs`) already bounds live
  sources; K multiplies its per-source cost by at most 4 file handles.
- `SqliteSource::probe`/FK reads (direct `rusqlite` opens on `path`) are untouched — they already
  open their own short-lived connections.
- The `Source` trait itself does not change shape; the postgres impl is already conformant. Add the
  contract sentence to the trait doc so the next source kind inherits the requirement.

### Rejected alternatives

- **One connection per query** (open/close around each read): re-pays sqlite open + pragma setup per
  tile and regresses the pool-cache scope's whole point; K warm slots amortize like the pool does.
- **Patch upstream `SqliteConnectionPool` to hold N connections:** right place in principle, but it
  forks a vendored dependency for a behavior we can compose from the outside in ~one file, and the
  fork must then track upstream forever.
- **WAL mode + shared cache tricks on the single connection:** concurrency in sqlite comes from
  *separate connections*; no journal mode makes one connection serve two scans at once.
- **Raise `viz.query_batch` semaphore / add host-side parallelism:** the host is already parallel;
  measured proof above. Adding parallelism above a serial funnel changes nothing.

## How it fits the core

- **Capabilities & deny path:** none touched — this is below the caps wall, inside the child's
  engine layer. Same verbs, same gates, same DSN mediation (each slot connects with the same
  host-mediated DSN, never stored).
- **Workspace isolation:** unchanged — slots are per-`(kind, dsn)` source like the pool they
  replace 1-of-K-fold; nothing crosses a workspace that didn't before.
- **§3.4 statelessness:** identical reasoning to `pool.rs` (read its header): K warm connections
  are a cache, reconstructible from the next call's own input; a kill + respawn costs K slow
  queries instead of one.
- **Rule 10:** the invariant lands on the `Source` trait, not on a kind check; no caller knows K
  exists.
- **One responsibility per file:** the slot vector + counter live in `source/sqlite.rs` where the
  single pool lives today; if the slot struct wants room, `source/sqlite_slots.rs`, not a util.

## Example flow

1. A viewer opens `demo-operations` (17 federation targets, one sqlite source), result cache off.
2. The grid sends one `viz.query_batch`; the host fans 16-wide; the child receives ~16 concurrent
   `federation.query` calls.
3. Calls land round-robin on slots 0–3; four scans run at once. Wall ≈ ceil(17/4) × ~250 ms ≈ 1.1 s
   (was 3.5 s). A re-open inside a caller-declared `cache:{ttl_s}` window is still ms — the result
   cache sits above this and is unchanged.

## Testing plan

Real engine, real file, no fakes (testing-scope §0):

- **The staircase test (the headline):** seed a real sqlite file with enough rows that a scan
  measurably costs (~100 ms+); run 4 concurrent `query()` calls; assert wall < 2× the
  slowest-single (generous CI margin — the serial behavior measures ~4×, so the assertion has a
  wide honest gap). **Mutation check:** set K = 1 and watch it go red.
- **Correctness under concurrency:** N concurrent queries with distinct predicates each return
  exactly their own rows (no cross-talk between slots' provider caches).
- **Lazy slots:** a source serving strictly serial calls builds exactly one connection (assert via
  slot-build counter).
- **Write path untouched:** a `federation.write` interleaved with concurrent reads keeps its
  current semantics (existing write tests keep passing — run them, that is the assertion).
- **Postgres conformance:** the staircase test runs against the postgres source too when Docker is
  available (it should already pass — that is the negative control proving the test measures the
  source, not the harness).

## Risks & hard problems

- **`SQLITE_BUSY` under read/write mix.** Readers on separate connections can hit BUSY while a
  write holds the file. The factory's existing 5 s busy-timeout carries over to every slot;
  read-only workloads (the dashboard case) never contend. Covered by the write-interleave test.
- **Provider cache memory ×K.** Schemas are small (KBs); bounded by K=4 × tables-touched. Not worth
  a second eviction mechanism riding a cache that already evicts by source.
- **First-touch latency ×K.** The K-th cold slot re-pays the per-table provider build. Accepted and
  bounded (lazy slots mean it only happens under real concurrency, exactly when the payoff exists).

## Open questions

None — resolved by decision in this scope: K = 4 constant (not configurable); slots lazy after the
first; sqlite-only implementation with the invariant stated on the trait; no upstream patch; write
path explicitly untouched.

## Related

- `federation-pool-cache-scope.md` — the per-source warm pool this multiplies by K; its §3.4
  reasoning is reused verbatim.
- `federation-result-cache-scope.md` — the layer above; unchanged, still the only thing that makes
  an identical re-open ~ms.
- `caching/dashboard-query-acceleration-scope.md` — the batch verb whose semaphore this finally
  lets breathe.
- `crates/federation/src/main.rs` worker-thread doc — the earlier "staircase" finding this scope
  closes out.
- Downstream consumer: rubix-ai `docs/scope/frontend/dashboard/dashboard-load-tail-scope.md`
  (needs only the tag bump — no UI change rides this scope).
