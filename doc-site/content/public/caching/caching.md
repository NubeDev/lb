# Caching

lb ships an **optional server-side response cache** on the host MCP read path: a warm page open is
answered from memory instead of re-running the query engines, and N concurrent identical reads
compute **once** (single-flight). It is off unless an embedder turns it on.

## What it is (v1)

- **In-process only** — the [`moka`](https://github.com/moka-rs/moka) crate: TTL + a byte-weighted
  budget + `try_get_with` single-flight. **No Redis, no sidecar, no new table** (one-datastore rule).
- **Behind the wall.** The cache sits *after* auth and the capability check, keyed
  `{workspace, verb, canonical-args, generation}` — never a token or subject. A denied call never
  reaches or populates it; a hit is gated exactly like a miss.
- **Optional twice.** The `page-cache` cargo feature compiles the whole thing in or out (feature-off
  is a zero-cost no-op seam — no `moka` in the binary). When compiled in, `BootConfig.cache:
  Option<CacheConfig>` is the runtime switch + budget.

## What is cached (the v1 allowlist)

The **source-picker bundle** — the read verbs a page-open fires that were audited to be a pure
function of `{workspace, args}` (a coarse verb-level cap gate, no per-caller row filtering):

| Verb | Class | TTL |
|---|---|---|
| `datasource.list` | datasource | 60 s |
| `series.list` | series | 60 s |
| `flows.list` / `flows.get` | flows | 60 s |
| `ext.list` | ext | 60 s |

Everything else dispatches every call. **`viz.query` is deliberately NOT cached in v1**: it
re-authorizes each panel target under the *caller's* grants, so its result varies by caller and a
subject-free key would leak one caller's data to another. It re-enters only once keyed safely (a
capability-fingerprinted key) — the named follow-up. Extension (`<ext>.<tool>`) verbs are uncacheable
by construction (no cache class).

## Invalidation

A write **bumps the per-`{workspace, class}` generation counter** the moment it lands; the generation
is part of the key, so stale entries become unreachable at once and age out via TTL/eviction (no
scan-and-delete). Invalidation is **coarse by class, not per entity** — e.g. `ingest.write` bumps the
`series` class; a generic `store.write` conservatively nukes every class. `ext.list` liveness
(`running`/`restart_count`) is process state no write covers, so it is bounded by the TTL, like an
external database writer.

## Operating it

- **Turn it on (embedder):** set `BootConfig.cache = Some(CacheConfig::default())` (enabled, 32 MiB,
  60 s lists). The standalone `node` binary reads `LB_CACHE=1` + `LB_CACHE_BUDGET_MB`.
- **Observe:** `cache.stats` (cap `mcp:cache.stats:call`) — hits, misses, evictions, entry count,
  weighted size, and a per-class breakdown.
- **Purge:** `cache.purge` (cap `mcp:cache.purge:call`) — drop the calling workspace's cached reads
  (a bounded generation bump; other workspaces untouched). The operator's stale-data escape hatch.

Both `cache.*` verbs are admin-only. See `skills/page-cache/SKILL.md` for the drivable surface.

## Not in v1 (named follow-ups)

- **`viz.query` (+ time-bucket quantisation)** — cached under a capability-fingerprinted key.
- **A persistent warm tier** — a restart is a cold cache by design; a SurrealDB tier is a conditional
  v2, blocked on restart-persisted generation counters (a cold cache with persisted counters would
  serve stale data).
- **Per-entity generations** and **per-`ToolDescriptor` cache-class declaration** — invalidation
  refinements driven by the perf assertion.

Full design + risks: `docs/scope/caching/response-cache-scope.md`.
