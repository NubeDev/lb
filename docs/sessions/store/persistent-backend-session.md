# Store — persistent on-disk backend + capability spike (session)

- Date: 2026-06-27
- Scope: ../../scope/store/persistent-backend-scope.md
- Stage: S8 — data plane (durable store + ingest + tagging) (slice 0, the gate). See STAGES.md.
- Status: done

## Goal
Ship `Store::open(path)` on a persistent embedded SurrealDB engine (keeping `Store::memory()` for
tests), select the engine by config (`LB_STORE_PATH`) with no role code-branch, and run the day-one
GO/NO-GO capability spike as a permanent hermetic CI test. This slice is the gate: ingest + tags are
blocked until the engine is pinned and the matrix is recorded.

## What changed
- `rust/Cargo.toml` — `surrealdb` now builds with `kv-mem` **and** `kv-surrealkv` (both engines
  compiled into every node; the constructor chosen at boot is the config decision, not a code branch).
- `crates/store/src/open.rs` — added `Store::open(path)` (SurrealKV) alongside `Store::memory()`, plus
  a raw `query_ws(ws, sql, bindings)` escape hatch (the spike + ingest/tags need RELATE / composite-ID
  / multi-statement statements the generic kv verbs don't express). Namespace selected from `ws` first
  — the hard wall holds identically.
- `crates/host/src/boot.rs` — `Node::open_store()` reads `LB_STORE_PATH` (set → `open(path)`; unset →
  `memory()`). Thin boot-wiring layer §3.1 permits; no `if cloud`.
- `crates/store/tests/capability_spike_test.rs` — the permanent hermetic spike (own temp dir,
  idempotent cleanup). LOAD-BEARING features FAIL the test on ✗ (NO-GO); DEGRADABLE features are
  recorded (printed) not gated.
- `crates/store/examples/crash_writer.rs` + `tests/crash_test.rs` — the crash set (subprocess +
  SIGABRT). `tests/persistent_parity_test.rs` — isolation/verb parity re-run on `open()`.

## Decisions & alternatives
- **Engine pinned: SurrealKV.** The three-axis rule (crash-consistency vetoes → feature coverage →
  build footprint): all five LOAD-BEARING features are AVAILABLE on SurrealKV and the crash set passes,
  so axis-1 is satisfied without falling back to RocksDB. SurrealKV is pure-Rust (no C++ toolchain) —
  the "builds anywhere / on a Pi" posture. RocksDB stays the documented fallback if a future
  LOAD-BEARING regression appears.
- **Both engines compiled in, constructor chosen by config.** Rejected a Cargo-feature-per-engine that
  would make a node's backend a build variant — that drifts from "one binary, role by config". The
  symmetric-node rule is satisfied because the *code* is identical; only `LB_STORE_PATH` differs.
- **Spike as a permanent test, not a throwaway probe.** A future SurrealDB bump that drops a feature is
  caught in CI, not mid-build of a dependent slice.

## Spike matrix result (the deliverable)
Recorded from `cargo test -p lb-store --test capability_spike_test -- --nocapture`:

| Feature | Class | Result |
|---|---|---|
| Durability across restart (write→kill→reopen) | LOAD-BEARING | ✓ AVAILABLE |
| Composite/array record IDs (`[series,producer,seq]`, `[key,value]`) | LOAD-BEARING | ✓ AVAILABLE |
| `RELATE` edges with properties | LOAD-BEARING | ✓ AVAILABLE |
| Namespace-per-workspace isolation on disk | LOAD-BEARING | ✓ AVAILABLE |
| Multi-statement transactions (all-or-nothing) | LOAD-BEARING | ✓ AVAILABLE |
| `DEFINE BUCKET` / file storage | DEGRADABLE | ✗ UNAVAILABLE → ingest binary payloads use record-as-content (S4 fallback) |
| `DEFINE INDEX … SEARCH` (BM25 full-text) | DEGRADABLE | ✓ AVAILABLE → tags full-text ships |
| `DEFINE INDEX … HNSW` (vector) | DEGRADABLE | ✓ AVAILABLE → tags vector ships |
| `DEFINE TABLE … AS SELECT … GROUP` (materialized view) | DEGRADABLE | ✓ defines, ✗ **does not populate** → tag_counts computed per-query (see debugging) |
| `LIVE SELECT` | DEGRADABLE | ✓ AVAILABLE (unused; motion rides Zenoh) |

**GO.** All LOAD-BEARING ✓ → SurrealKV pinned, all of S8 cleared to build. `bucket=false` consumed by
ingest (record-as-content); materialized-view non-population consumed by tags (per-query counts).

## Tests
- `cargo test -p lb-store` — capability spike (6), crash set (4: baseline, commit-then-kill,
  kill-mid-tx rollback, flush-burst), persistent parity/isolation (4), plus the existing kv tests. All
  green (output pasted in the final-verify section of the tags session / STATUS).
- Mandatory categories: **workspace-isolation** re-run on the persistent engine
  (`ws_b_cannot_read_ws_a_record_on_disk`); **offline/sync** via the crash + reopen tests; **capability
  deny** — n/a new (no new grants; the store sits below the cap gate).

## Debugging
None new in the store engine itself. The spike *recorded* two degradations consumed downstream:
`DEFINE BUCKET` unavailable (already logged S4: debugging/store/define-bucket-unavailable-in-kv-mem-build.md)
and materialized-view non-population (logged by tags: debugging/tags/materialized-view-does-not-populate.md).
Separately fixed a pre-existing workspace build break — see
debugging/store/half-wired-modules-block-workspace-build.md.

## Public / scope updates
- Promoted to `public/store/store.md` (persistent backend section + the pinned engine + matrix).
- Scope open questions resolved: SurrealKV pinned (the measurement); `LB_STORE_PATH` env is the config
  surface (lean taken); graceful-shutdown/flush — the unclean-kill crash test proves the WAL recovers,
  so no explicit close is required for crash-consistency.

## Dead ends / surprises
- `DEFINE BUCKET … BACKEND "memory"` failed to parse on SurrealKV — recorded as the documented ✗
  (record-as-content fallback), exactly the degrade path the scope anticipated.

## Follow-ups
- At-rest encryption is node-level by decision (out of scope; recorded in the scope doc).
- RocksDB fallback remains documented but unused.
- STATUS.md updated: slice 0 shipped, engine pinned.
