# Store scope — persistent on-disk backend + embedded-engine capability spike

Status: scope (the ask). Promotes to `public/store/` once shipped. Stage: **S8 enabling slice
(slice 0)** — the day-one prerequisite both `scope/ingest/` and `scope/tags/` depend on. Ship this
*first*; the ingest durability story and the full tags index set are blocked until it lands and the
spike results are in.

Today the store is **in-memory only** — `Store::memory()` on the `Mem` engine
(`crates/store/src/open.rs`; workspace `surrealdb` is built with only `kv-mem`). Two consequences block
S8: (1) **nothing is written to disk**, so "buffer until committed to disk" (ingest) is literally
impossible; (2) we have **not verified** which SurrealDB index/storage features work in our embedded
build — and `kv-mem` already surprised us once (file **buckets** were unavailable in S4, forcing the
record-as-content workaround). This slice adds a **persistent embedded backend selected by config** and
**spikes the feature surface on day one** so the ingest/tags slices build on known ground, not hope.

## Goals

- **`Store::open(path)`** — a persistent embedded SurrealDB engine (on-disk), alongside the retained
  `Store::memory()` (kept for tests). The engine is chosen by **config, not a code branch** (symmetric
  nodes — `crates/store/src/open.rs` already says "file/rocksdb engine later by config").
- **Durability across restart** — write, drop the handle, reopen at the same path, read it back. The one
  thing `kv-mem` cannot do; the foundation of every must-deliver/ingest guarantee.
- **A day-one capability spike with a written GO/NO-GO threshold** — a permanent, hermetic CI test
  (own namespace, idempotent cleanup) that *defines and exercises* each SurrealDB feature the S8 scopes
  assume on the chosen embedded engine, and classifies each as **LOAD-BEARING** (a ✗ blocks all of S8 —
  stop and switch engines) or **DEGRADABLE** (a ✗ defers one capability to a follow-up; the core ships).
  This matrix **is the deliverable** — ingest/tags branch on it as a hard gate before they start, so a
  missing feature can never silently invalidate a slice mid-build:

  | Feature | Class | If ✗ (the rule) |
  |---|---|---|
  | Durability across restart (write→kill→reopen) | **LOAD-BEARING** | NO-GO. All of S8 stops; the engine is wrong. |
  | Composite/array record IDs (`[series,producer,seq]`, `[key,value]`) | **LOAD-BEARING** | NO-GO. The ingest dedup id and tag node id depend on it. |
  | `RELATE` edges with properties | **LOAD-BEARING** | NO-GO. The tag/provenance/lineage graph depends on it. |
  | Namespace-per-workspace isolation on disk | **LOAD-BEARING** | NO-GO. The hard wall must hold on the persistent engine. |
  | Transactions (multi-statement, all-or-nothing) | **LOAD-BEARING** | NO-GO. Batch-commit atomicity depends on it. |
  | `DEFINE BUCKET` / file storage | DEGRADABLE | Ingest binary payloads fall back to record-as-content (the S4 workaround) until available. |
  | `DEFINE INDEX … SEARCH` (full-text/BM25) | DEGRADABLE | Tags value full-text → follow-up; exact/facet ship now. |
  | `DEFINE INDEX … HNSW` (vector) | DEGRADABLE | Tags semantic/"similar-to" → follow-up; the rest of tags ships. |
  | `DEFINE TABLE … AS SELECT … GROUP` (materialized views) | DEGRADABLE | Rollups / `tag_counts` computed per-query until available. |
  | `LIVE SELECT` | DEGRADABLE | A convenience; motion already rides Zenoh. No impact. |

  A LOAD-BEARING ✗ on SurrealKV triggers the RocksDB fallback (below); if RocksDB also lacks it, S8 is
  re-scoped. A DEGRADABLE ✗ is recorded with its named fallback (the right-hand column) and consumed by
  the dependent scope. The spike output (the filled matrix) lands in the session doc **and** a debug entry.
- **Zero change above the open seam** — `read`/`write`/`list`/`write_tx` and every caller are untouched;
  only *which engine the handle wraps* changes. Namespace-per-workspace isolation holds identically.

## Non-goals

- **No distributed/clustered SurrealDB** (TiKV/foundationdb). Embedded single-node only — symmetric
  nodes sync via §6.8, not via a shared DB cluster (rule #2 + the sync model).
- **No data migration tooling.** Greenfield — there is no production data to migrate yet. A
  schema/migration story is a later scope if needed.
- **No second engine at runtime.** One engine per node, by config. (Tests use `memory()`; a real node
  uses `open()`.) Not a per-call choice.
- **No new persistence layer.** Still SurrealDB only — this is a *feature flag + constructor*, not a new
  store.
- **No SDK/WIT change.** Internal to the `store` crate.

## Intent / approach

**A feature flag + a second constructor, nothing more.** SurrealDB v2 selects its KV backend by Cargo
feature; we add a persistent one and a constructor that opens it at a path. The recommendation is
**SurrealKV** (`kv-surrealkv` — pure-Rust, embedded, no C++ toolchain, matches our "one binary, easy to
build everywhere / on a Pi" posture) with **RocksDB** (`kv-rocksdb`) as the fallback. The choice is made
on **three axes**, in priority order: (1) **crash-consistency maturity — this has veto power**; it is the
one property the whole slice exists to provide, and RocksDB's track record genuinely beats SurrealKV's,
so "builds anywhere" must never win over "doesn't corrupt on power-loss"; (2) the LOAD-BEARING/DEGRADABLE
feature coverage from the spike; (3) build footprint (the Pi/"builds anywhere" goal). The slice
**measures all three** (the crash tests below are the axis-1 evidence), then pins one.

```rust
// crates/store/src/open.rs — same handle type, engine by config (symmetric nodes).
impl Store {
    pub async fn memory() -> Result<Self, StoreError> { /* Mem — tests, unchanged */ }

    /// Open a persistent embedded store at `path` (a real node). Durable across restart.
    pub async fn open(path: &str) -> Result<Self, StoreError> {
        let db = Surreal::new::<SurrealKv>(path).await?;   // or RocksDb, by the pinned feature
        Ok(Self { db })
    }
}
```

Engine selection lives in the **node boot wiring** (the thin layer §3.1 permits to be config-aware), not
in core crates: `LB_STORE_PATH` set → `open(path)`; absent → `memory()` (dev/test). No `if cloud {…}`.

**Why spike before building.** "Everything at once" was the chosen ambition for tags, and ingest assumes
durable staging + buckets. If `SEARCH`/`HNSW`/`BUCKET` turn out unavailable in the embedded engine (as
buckets were in `kv-mem`), discovering it *mid-tags-build* is a wasted slice. A half-day probe up front
converts an unknown into a known degrade plan. This is the cheapest risk-buy in S8.

**Rejected alternatives:**
- *Stay on `kv-mem` + periodic snapshot-to-disk.* Rejected — re-implements durability the engine already
  provides, and still can't offer buckets or crash-consistency.
- *Default RocksDB.* Rejected as the *default* — heavier build (C++), worse for the Pi/"builds anywhere"
  goal; kept as the fallback if SurrealKV is short a needed feature.
- *Per-call engine choice.* Rejected — an engine is a node property (config), not a request parameter.

## How it fits the core

- **Tenancy / isolation:** unchanged — `use_ns(ws)` still scopes every op to the workspace namespace;
  the persistent engine holds all namespaces in one on-disk store exactly as `Mem` held them in memory.
  The isolation tests must pass **identically** on the persistent engine (re-run them there).
- **At-rest encryption is node-level, by decision (resolving the §6.7 tension).** One store per node holds
  all namespaces in one on-disk file, so at-rest encryption — when added — is **whole-store, node-level**
  (e.g. an encrypted volume / engine-level key), **not** per-workspace. This does **not** contradict §6.7:
  §6.7's per-workspace keys protect **secret values** (envelope-encrypted records inside the store), not
  the store file itself. Per-workspace store *files* (path-per-workspace) were considered and **rejected**
  — it fragments the single embedded instance, breaks cross-namespace queries, and multiplies open
  handles for marginal gain. At-rest encryption itself is out of scope here but the *shape* is decided:
  node-level. (No longer an open question.)
- **Capabilities:** N/A directly — the store sits below the capability gate (raw verbs run after
  `caps::check`). No new grants.
- **Placement:** `either` — every node opens its own embedded store; the cloud and a Pi differ only by
  `path` and what role mounts. The engine is config.
- **Data (SurrealDB):** the engine swap itself; plus the spike's probe tables/indexes — which live in a
  **permanent, hermetic CI test** (its own namespace, idempotent setup/cleanup), **not** a one-shot
  throwaway, so a future SurrealDB upgrade that drops a feature is caught. The real `series`/`tag`
  schemas land in their own slices.
- **Bus / Sync:** unchanged — records still sync as `(table, id)` upserts (§6.8). Durability makes
  offline-buffer-and-replay actually survive a restart (the previously-untestable half of those tests).
- **Secrets:** the on-disk path may warrant at-rest encryption later (out of scope; note it).

## Example flow

1. A node boots with `LB_STORE_PATH=/var/lib/lazybones/acme`. `Node::boot` calls `Store::open(path)`;
   the workspace namespace is created on first `use_ns`.
2. The node writes a channel message + a job step, then is **killed** (not gracefully stopped).
3. The node reboots, `Store::open(same path)` — the message and job step are **present**. (On `kv-mem`
   they would be gone; this is the new, now-testable guarantee.)
4. The boot-time **capability spike** ran once in CI: it defined a `SEARCH` index, an `HNSW` index, a
   materialized view, a `RELATE` edge, and a `BUCKET`, exercised each, and emitted a results table —
   e.g. `SEARCH ✓ · HNSW ✓ · views ✓ · graph ✓ · BUCKET ✓ · LIVE ✓`. Any ✗ is wired into the dependent
   scope's degrade plan before that slice starts.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Workspace isolation** — re-run the existing store + MCP isolation tests **against the persistent
  engine** (a second namespace cannot see the first's data on disk, not just in memory).
- **Offline / sync** — the offline-buffer-and-replay tests now run with a **real restart** in the middle:
  write offline → drop/reopen the store → reconnect → idempotent replay. Durability closes the half of
  these that `kv-mem` could only simulate.
- **Capability deny** — N/A new (no new grants); the existing deny tests must still pass on the new engine.

Plus this slice's own cases:

- **Durability — a foundation needs more than one crash case.** The minimum set, because ingest's entire
  "never lost until on disk" rests here:
  - write → drop handle → reopen → records present (the baseline);
  - **kill mid `write_tx`** → reopen → the partial transaction **rolled back**, not half-applied or
    corrupt (atomicity is what batch-commit relies on);
  - **kill during compaction/flush** → reopen → no corruption, last committed write survives;
  - **reopen after an unclean kill with a half-written WAL** → the engine recovers (replays or discards
    the torn tail), never returns corrupt data.
  SurrealKV maturity (below) is the real risk; a single kill-and-reopen does not retire it.
- **The capability spike** — one test per feature (`SEARCH`, `HNSW`, materialized view, `RELATE`+props,
  composite IDs, `BUCKET`, `LIVE`): define it, exercise it, assert the result; the suite's output *is* the
  recorded capability table. A feature that errors is captured as a documented ✗, not a hard failure that
  blocks the slice.
- **Engine parity** — a representative slice of the existing suite runs green on `open()` as well as
  `memory()`, proving the swap is transparent above the seam.

## Risks & hard problems

- **A needed feature is missing embedded.** The whole reason to spike. If `SEARCH`/`HNSW`/`BUCKET` is
  absent in SurrealKV, either switch to RocksDB (if *it* has it) or degrade the dependent tags/ingest
  capability to a follow-up. The slice's job is to surface this in half a day, not month two.
- **SurrealKV maturity.** Newer than RocksDB; watch for crash-consistency / large-dataset edge cases.
  Mitigation: the durability + crash test above, and RocksDB as the pinned fallback.
- **On-disk format / version pinning.** A SurrealDB minor bump could change the on-disk format. Pin the
  version; note the upgrade path (export/import) as a future migration concern.
- **Write throughput under ingest volume.** The persistent engine must keep up with batched ingest
  commits. Not load-tested here, but the engine choice should be revisited if ingest measurements show a
  ceiling. Flag, don't solve, in this slice.
- **At-rest encryption.** A real deployment may require it; out of scope but recorded so it isn't
  forgotten when a node holds real data on disk.

## Open questions

- **SurrealKV vs RocksDB** — which to pin after the spike? Decided *by* the three-axis rule above
  (crash-consistency vetoes); this records the measurement, not the criteria.
- **Config surface** — `LB_STORE_PATH` env only, or a config file field? (Lean: env now, fold into the
  node config story when it lands.)
- **Graceful shutdown / flush** — does `open()` need an explicit close/flush on node stop for
  crash-consistency, or is the engine's WAL sufficient? Verify in the crash test (the unclean-kill case
  is the proof).

Resolved in this doc (no longer open): the GO/NO-GO matrix (the feature classes + fallbacks), one store
per node with **node-level** at-rest encryption (§6.7 protects secret values, not the store file), and
the spike living as a permanent hermetic CI test.

## Related

- `scope/store/store-scope.md` — the parent store model this extends.
- `scope/ingest/ingest-scope.md` — blocked on this (durable staging, "buffer until on disk", buckets for
  binary payloads).
- `scope/tags/tags-scope.md` — blocked on this (the `SEARCH`/`HNSW`/materialized-view feature surface the
  full design assumes; its #1 risk is exactly this verification).
- README **§6.1** (SurrealDB — the multi-model engine + the time-series/buckets claims this verifies),
  **§3.1/§3.2** (symmetric nodes; engine is config; one datastore), **§6.8** (sync — now restart-durable).
- `scope/files/` — the S4 record-as-content workaround that a working `BUCKET` would replace.
