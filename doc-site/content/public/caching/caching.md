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

Everything else on this list dispatches every call. Extension (`<ext>.<tool>`) verbs are uncacheable
by construction (no cache class).

## `viz.query` — the subject-scoped class (dashboard query acceleration)

`viz.query` re-authorizes each panel target under the *caller's* grants (a denied target → an empty
frame), so its result varies by caller — a subject-free key would leak one caller's data to another.
It is cached under a dedicated **`subject_scoped`** class whose key folds a **capability fingerprint**:
a stable hash of the sorted set of the panel's target caps the caller HOLDS (never identity, never the
token). Two callers with the same reach share a warm entry; a caller who would get a different (denied)
frame produces a different key and resolves their own. The wall holds by construction — a warm frame is
only ever served to a caller whose grants would have computed it.

Three pieces make a warm dashboard open ~microseconds instead of one DB round-trip per tile:

- **A caller freshness directive.** `viz.query` accepts a top-level `cache: {ttl_s}` (sibling of `now`),
  threaded source-blind into every target's args before dispatch — a `federation.query` target honours
  it (the per-source result cache), other verbs ignore it. `ttl_s:0`/absent ⇒ live (bypass), the
  default. A per-target `cache` overrides the top-level.
- **A time-bucket quantiser.** The resolved range (`now` + numeric `from`/`to`) is floored to the TTL
  bucket, so relative "last 1h" opens inside one bucket share a key — and the query runs on the bucketed
  range (the cache never serves a range it did not compute). End-day-exclusivity survives the floor.
- **`viz.query_batch {panels[], now?, cache?}`.** A synchronous, bounded (cap 64) fan-in that resolves a
  board's panels concurrently in ONE call — killing the browser's HTTP/1.1 connection ceiling. Per-item
  partial failure (one bad tile errors, the rest resolve); rides the existing `mcp:viz.query:call` cap
  (no new privilege); each panel resolves through the same `subject_scoped` cached path.

Single-flight (moka `try_get_with`) collapses N concurrent viewers of the same board to one compute.
The class shows in the `cache.stats` per-class breakdown as `viz`.

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

- **A persistent warm tier** — a restart is a cold cache by design; a SurrealDB tier is a conditional
  v2, blocked on restart-persisted generation counters (a cold cache with persisted counters would
  serve stale data).
- **Per-entity generations** and **per-`ToolDescriptor` cache-class declaration** — invalidation
  refinements driven by the perf assertion.

Full design + risks: `docs/scope/caching/response-cache-scope.md`.
