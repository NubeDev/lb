# Session — `federation.profile`: the durable per-source discovery profile

Scope: `docs/scope/datasources/datasource-profile-scope.md` (now **SHIPPED, untagged**).
Downstream consumer: `NubeIO/rubix-ai docs/scope/frontend/dashboard/quick-chart-discovery-scope.md`.

## The ask

Every "smart" consumer — rubix-ai's Quick Chart, agents writing SQL, the panel wizard — re-derived
the same per-source statistics live from a browser: N× `SELECT DISTINCT … LIMIT 200` + N× `GROUP BY`
range scans per open, ~20 s on a wide table, uncached server-side, re-paid by every user on every
page load. `datasource-samples-scope.md` had explicitly deferred this ("no statistics/profiling …
useful later"). This session made "later" now: compute it once, server-side, next to the data;
persist it; read it back in one store read.

## What landed

**Sidecar** — `rust/crates/federation/src/profile.rs` (+ `mod profile;`, a `federation.profile` arm
in `main.rs`, and an `extension.toml` tool row). One bounded pass per source: per table, the Arrow
schema → column kinds, `foreign_keys()`, one batched `MIN`/`MAX`/`COUNT` aggregate for every
numeric/time column, one `GROUP BY col COUNT(*)` per text column for cardinality + top-K values, and
— when the table has exactly one numeric column (the long/EAV signature) — one
`GROUP BY col MIN(v), MAX(v)` per text column for the grouped value ranges.

**Host** — `federation/profile.rs` (compute + upsert), `profile_get.rs` (pure read),
`profile_refresh.rs` (enqueue), `profile_record.rs` (the `datasource_profile:{ws}:{source}` record,
its `(data.tag, data.profiled_at)` index, and the LIMIT-bounded `stale()` scan),
`react_to_profiles.rs` (the reactor).

**Seams** — `BootConfig::profile: Option<ProfileConfig{enabled, refresh_after_secs, max_tables,
max_values}>` + `LB_PROFILE*` at the binary boundary only; the `datasource-profile` cargo feature on
both `lb-host` and `lb-node`, **off by default**.

## Decisions worth keeping

- **`profile_get` is a pure store read.** The hot path can never be silently converted into a 20 s
  pass; a caller that wants the compute passes `compute_if_missing: true` and is thereby stating it
  can afford to block. This is the single most load-bearing decision in the scope.
- **Reads ride `mcp:federation.query:call`; only `profile_refresh` gets a new cap.** A profile is
  strictly less than what the read cap can already `SELECT`, so reading one is not a new privilege.
  *Refreshing* spends real work on someone else's database on demand — that is a different thing, so
  `mcp:federation.profile_refresh:call` is separately grantable and revokable. The deny test asserts
  that holding the read cap is explicitly **not** enough to refresh.
- **`row_estimate` is catalog-based or absent.** It comes from `TableMeta.rows` (Postgres
  `reltuples`) where the kind exposes one, and is omitted otherwise. Never `COUNT(*)`: an exact count
  on an unindexed table is precisely the unbounded cost this pass exists to avoid.
- **Deterministic ordering everywhere** — tables sorted before truncation, values sorted by
  (frequency desc, value asc), group ranges sorted by group. This is what makes re-profiling an
  unchanged source produce a byte-identical record, which is what makes the reactor's repeated
  upserts free. The idempotence test asserts equality of the whole record, not a subset.
- **Capped cardinality is reported as a FLOOR, not a guess.** Past 200 scanned groups the column
  carries `distinct: 200, distinct_capped: true`, and the record's `truncated` flag goes true — the
  profile says it is partial rather than reading as complete.
- **Two independent in-flight guards on enqueue.** A deterministic job id (so a burst of refreshes
  collapses onto one job) *and* a `profiling_since` stamp on the record with an expiry (so a node
  that died mid-pass cannot wedge a source forever). Either alone leaves a race; the reactor test
  drives the tick twice and asserts the second is a no-op.
- **Rule 10 held throughout.** The record stores the sidecar's per-table objects as opaque JSON; the
  host never reinterprets per-kind detail, column kinds come from the Arrow type and never from a
  column NAME, and a kind that cannot answer an aggregate contributes nulls rather than an error. No
  file below the boot seam names a source kind or a consumer.

## Tests

`rust/crates/host/tests/federation_profile_test.rs` — real embedded SurrealDB, real caps, the real
supervisor spawning the real sidecar, a real on-disk SQLite fixture. No mocks.

Covered: cap-deny for both caps (mandatory), ws-isolation on the record and on the reactor
(mandatory), the happy-path record shape (kinds, cardinality, top values, min/max, null fraction,
FKs, group ranges), bounds (30 tables → 25 + `truncated`; 250 distinct → capped), redaction (a
`password` column keeps its shape, loses its values), idempotence, `profile_get` returning `NotFound`
until asked to compute, refresh-enqueue idempotence, and the reactor's fresh/stale/no-duplicate
behaviour. Plus the feature-off build, verified by a clean `cargo build -p lb-node` with the feature
absent.

## Remaining

**A `node-v*` tag.** rubix-ai is consuming this through a local `[patch]` today; nothing else is
outstanding.
