# Caching scope — optional gateway response cache (moka + SurrealDB warm tier)

Status: scope (the ask). Promotes to `doc-site/content/public/caching/` once shipped.

Opening a page — a dashboard above all — fires a burst of read verbs (`dashboard.get`, the
source lists, `viz.query` per viewer panel) that the node recomputes on every open, even when
nothing changed. On edge hardware (a Raspberry-Pi-class node) that cold work dominates the
page paint. We want an **optional server-side response cache** on the gateway's MCP read
path: a warm open is answered from memory in microseconds instead of re-running the query
engines. Hot tier = the **`moka`** crate in-process (TTL + byte-weighted budget); warm tier =
**SurrealDB** (the one datastore) for expensive derived results only, so a restart doesn't
re-pay the heaviest queries. **No Redis / no sidecar** — a second stateful service would
break the one-datastore rule and is exactly the ops burden an edge box can't carry. The whole
seam is **compile-time optional** (a `page-cache` cargo feature, like `external-agent`) and
**runtime-configured** (an additive `BootConfig.cache`), so a RAM-tight build ships without
it and an embedder turns it on with a budget.

## Goals

- **Warm page opens do zero query-engine work.** Second open of an unchanged dashboard —
  same workspace, any client — serves every cacheable read from the hot tier.
- **Additive, generic seam.** One new `BootConfig.cache: Option<CacheConfig>` field
  (`enabled`, `memory_budget_bytes`, `default_ttl`, `warm_tier`, per-verb-class TTLs);
  transparent middleware on read dispatch. Any embedder configures it; no embedder or
  extension is named (rule 10) — an extension's read tools fall under the same generic
  verb-class policy as core tools.
- **Truly optional.** Feature off ⇒ no `moka` in the binary, no cache table, a zero-cost
  no-op seam, behaviour identical to today. Feature on + `cache: None`/`enabled: false` ⇒
  same. CI proves the feature-off build.
- **Honest by construction.** The cache sits behind auth + the caps wall; writes invalidate
  via per-`{ws, verb-class}` generation counters the moment they land; TTL is the backstop.
- **Static shell/ext-UI assets served cache-friendly** — ETag + `Cache-Control: immutable`
  for content-hashed bundles, revalidate for `index.html`. Independent first slice.

## Non-goals

- **No Redis/memcached/sidecar; no distributed cache.** Each node caches its own reads;
  federation peers are not a cache tier.
- **Not caching motion.** SSE routes, `*.watch`, bus subjects: never cached (state vs
  motion). Live series ticks always bypass.
- **Not caching writes, auth, or anything subject-dependent by default.** `/auth/*`,
  `/login`, token material never enter the cache; caller-filtered reads are non-cacheable
  unless individually opted in with a safe key.
- **Not SSR/HTML caching.** The UI is an SPA — "page cache" = the read-verb responses that
  make a page load, plus asset headers.
- **Not the client-side cache.** Downstream UIs keep their own react-query layer; this is
  the server side of the same problem and composes with it.

## Intent / approach

**A read-through cache wrapped around MCP read-verb dispatch, after auth and the caps check,
keyed `{workspace, verb, canonical-args, generation}`, moka in front, SurrealDB behind,
feature-gated end to end.**

1. **Placement: after the wall, before dispatch.** A hit and a miss are gated identically —
   the cache can never widen access. The key carries **no token or subject**: a cacheable
   read's response must be a pure function of `{ws, verb, args}`. Verbs whose result varies
   by caller are marked non-cacheable in the policy (allowlist, not denylist).
2. **Verb-class policy, declared at registration.** Each read verb registers its cache class
   (`uncacheable` default | a named class with TTL) and each write verb registers which
   classes it dirties — beside the verb, not in a hand-maintained map inside the cache. The
   staleness test sweeps all registered writes mechanically.
3. **Invalidation: generation counters.** A write bumps its dirtied `{ws, class}`
   generations; the generation is part of the key, so stale entries become unreachable
   instantly and age out via TTL/eviction — no scan-and-delete. TTL covers what generations
   can't see (an external writer mutating a sqlite datasource): keep `viz.query`'s default
   TTL short.
4. **Two tiers.** `moka` (TinyLFU, per-entry TTL, weigher = serialised-value bytes so the
   budget is honest) answers hot repeats. The warm tier writes only results whose compute
   cost crossed a threshold to a `cache_entry` table `{key, ws, value, generation,
   expires_at}`; a sweep prunes expired rows; `warm_tier: false` skips SurrealDB entirely
   (protects SD-card write endurance on edge boxes).
5. **Crate choice.** `moka` — maintained, async-aware, and its TTL + weight-bounding are
   precisely the small-RAM controls needed. **Alternative rejected:** `quick_cache`
   (lighter, but no per-entry TTL/weigher out of the box — revisit only if moka's armv7
   footprint measurably hurts); **SurrealDB-only materialisation** (still pays a store
   round-trip per hit — most of the cost being avoided on slow hardware).

## How it fits the core

- **Tenancy / isolation:** every key and generation is workspace-scoped; warm rows carry
  `ws` and are pruned with the workspace. A hit can never cross the wall — tested.
- **Capabilities:** middleware runs after the caps check; denied calls never reach or
  populate the cache. No new grants for v1; the optional `cache.stats`/`cache.purge` admin
  pair (open question) would carry its own capability + deny path.
- **Symmetric nodes:** on/off/budget/TTLs are `BootConfig`, never a role or platform branch.
- **Placement:** either — any node may enable it; the win is largest on edge hardware.
- **MCP surface:** none for v1 (transparent middleware). API-shape walk: no CRUD/get/list/
  feed/batch; the only candidate surface is `cache.stats` (get) + `cache.purge` (small,
  bounded, synchronous — a moka clear + one delete-by-ws; not a job).
- **Data (SurrealDB):** one feature-gated table, `cache_entry` — derived, droppable state in
  the one datastore. State vs motion holds.
- **Bus (Zenoh):** N/A — no new subjects; watch/SSE explicitly bypass.
- **Sync / authority:** the node stays authoritative; the cache is droppable (purge = delete
  all, correctness unaffected). Offline unchanged.
- **Secrets:** none cached, none in keys.
- **Stateless extensions / hot-reload:** instances hold no cache state; an extension
  publish/upgrade bumps its verb classes' generations so stale ext responses die.
- **SDK/WIT impact:** **none.** No ABI change — the cache class of an extension tool is
  host-side policy (default uncacheable), not a WIT surface. Flag loudly if implementation
  finds otherwise.
- **File layout:** middleware, key canonicalisation, generations, verb-class policy, warm
  tier, and the no-op seam are separate ≤400-line files under one gateway/cache folder.
- **Durability:** N/A — cache entries are the opposite of must-deliver; nothing rides the
  outbox.

## Example flow

1. An embedder boots with the feature on and `cache: Some({memory_budget_bytes: 32 MiB,
   warm_tier: true, ..})`. An operator opens a dashboard: `dashboard.get` misses → store
   read → cached under `{ws, dashboard.get, {id}, gen:7}`. Each viewer `viz.query` computes
   (180 ms), lands in moka; the two heaviest also hit the warm tier.
2. Any client re-opens the dashboard: every cacheable read hits moka; zero query-engine work.
3. `dashboard.save` bumps `{ws, dashboard}` 7→8; the next `dashboard.get` misses, returns
   fresh, re-caches. Untouched `series.*` entries keep serving.
4. The node restarts. moka is empty; the heavy `viz.query` results are found in the warm
   tier with a current generation → served, re-promoted. Cold entries recompute.
5. Same binary, `enabled: false` (or a feature-off build): every step is a plain dispatch —
   identical responses, no table touched.

## Testing plan

Real embedded node (`mem://` store, real gateway) via the lib API — no mock backend, no fake
cache (`scope/testing/testing-scope.md` §0). Mandatory categories:

- **Capability deny-test:** a caller without the read cap gets the same deny on a warm key
  as a cold one — another caller's cached value is unreachable to them. Deny paths for
  `cache.*` verbs if they ship.
- **Workspace-isolation:** same-named dashboards in ws A/B; prime A, read from B → B's data.
  Purging A leaves B intact; warm-tier rows of A prune with A.
- **Staleness-after-write:** for **every registered write verb** (swept mechanically off the
  dirty-map, including an extension-published tool), write → immediate re-read returns the
  new value.
- **Perf/de-dup assertion:** instrument dispatch — second open of a seeded dashboard runs
  zero engine calls; after TTL expiry it recomputes.
- **Feature-off build:** CI compiles + boots with `page-cache` off and runs the read-path
  suite green (proves the no-op seam).
- **Budget/eviction:** fill past the budget; RSS stays bounded; evicted keys recompute
  correctly.
- **Warm tier:** restart on a persistent store → current-generation warm entry served;
  expired/superseded rows are not, and the sweep prunes them.
- **Hot-reload:** publish a new extension version → its cached responses invalidate.

## Risks & hard problems

- **Invalidation completeness is the whole ballgame.** One write verb missing from the
  dirty-map = silently stale pages. Hence policy-at-registration + the mechanical sweep.
- **Caller-dependent reads are a leak** if cached under a subject-free key. Default
  uncacheable; opt in per verb after review. Audit which core lists are grant-filtered
  today before writing the v1 allowlist.
- **Arg canonicalisation.** Unstable JSON field order / `null`-vs-absent = silent 100% miss
  rate. Canonical serialisation, unit-tested.
- **Budget honesty.** A weigher that under-counts (misses the serialised value) blows the
  small-RAM budget. Weigh stored bytes; assert RSS in the budget test.
- **Warm-tier write amplification** would hammer SD cards — hence expensive-only + the
  threshold + the independent toggle; guard with a metric.
- **External writers** never bump a generation — TTL is the only defence for
  datasource-backed reads. Keep those TTLs short and documented.

## Open questions

- **Admin surface in v1:** ship `cache.stats`/`cache.purge` (per-ws) now, or stay fully
  transparent until operating it demands visibility? If shipped: capability name, deny
  tests, and `skills/page-cache/SKILL.md`.
- **v1 cacheable allowlist:** candidates `dashboard.get/list`, `ext.list`,
  `datasource.list`, `series.list`, `flows.list/get`, viewer-mode `viz.query`. Which are
  subject-filtered today and stay excluded?
- **Feature default:** `page-cache` in default features (with `cache: None` runtime-off), or
  opt-in like `external-agent`? Measure moka's binary + idle-RSS cost on armv7 first.
- **Warm-tier threshold + initial TTLs:** e.g. warm at >100 ms compute; `viz.query` 15–30 s;
  lists 60 s — tune from the perf assertion, not taste.
- **Asset headers:** does the shell/ext-UI serve path already send ETags, or is the
  static-asset slice a real change here?

## Related

- README `§3` (workspace wall, capability-first, one datastore, state vs motion, symmetric
  nodes), `§6.10` (outbox — explicitly not used here), `§6.13` (SSE routes — bypass).
- `scope/node-roles/embed-node-scope.md` — `BootConfig` is the seam the new field extends.
- `scope/external-agent/` — the precedent for a compile-time-optional cargo feature.
- `scope/testing/testing-scope.md`, `docs/FILE-LAYOUT.md`, `docs/key-stack.md` (moka row).
- Downstream ask that motivated this: a product embedder's page-load latency on
  Raspberry-Pi-class hardware (generic — no embedder is special-cased, rule 10).
- Skill doc: **conditional** — N/A while transparent; `skills/page-cache/SKILL.md` owned by
  the implementing session if the `cache.*` admin verbs ship.
