# Caching scope — optional gateway response cache (moka, single-flight, generation-invalidated)

Status: scope (the ask, review-hardened). Promotes to `doc-site/content/public/caching/` once shipped.

Opening a page — a dashboard above all — fires a burst of read verbs (the source lists,
`viz.query` per viewer panel) that the node recomputes on every open, even when nothing
changed. On edge hardware (a Raspberry-Pi-class node) that cold work dominates the page
paint. We want an **optional server-side response cache** on the gateway's MCP read path:
a warm open is answered from memory in microseconds instead of re-running the query
engines, and **N concurrent identical reads compute once** (single-flight). v1 is the
**`moka`** crate in-process (TTL + byte-weighted budget + `try_get_with` coalescing) —
**no Redis / no sidecar** (one-datastore rule; a second stateful service is exactly the
ops burden an edge box can't carry) and **no warm tier in v1**: a SurrealDB-persisted tier
is the named v2 follow-up, gated on measured post-restart pain and on persisted generation
counters (see Risks — without them a restart-surviving tier serves stale data). The whole
seam is **compile-time optional** (a `page-cache` cargo feature, the `external-agent`
precedent) and **runtime-configured** (an additive `BootConfig.cache`), so a RAM-tight
build ships without it and an embedder turns it on with a budget.

## Goals

- **Warm page opens skip the query engines.** Re-open of an unchanged dashboard — same
  workspace, any client, within the TTL/time-bucket window — serves every cacheable read
  from moka.
- **Single-flight (stampede protection).** Ten viewers opening the same dashboard
  concurrently, or every panel recomputing after a save's generation bump, trigger **one**
  compute per key, not ten — `moka::try_get_with`, nearly free with the chosen crate, and
  half the server-side win.
- **Additive, generic seam.** One new `BootConfig.cache: Option<CacheConfig>` field
  (`enabled`, `memory_budget_bytes`, `default_ttl`, per-verb-class TTLs); transparent
  middleware on read dispatch. Any embedder configures it; no embedder or extension is
  named (rule 10).
- **Truly optional.** Feature off ⇒ no `moka` in the binary, a zero-cost no-op seam,
  behaviour identical to today. Feature on + `cache: None`/`enabled: false` ⇒ same. CI
  proves the feature-off build.
- **Honest by construction.** Behind auth + the caps wall; writes invalidate via
  per-`{ws, verb-class}` generation counters the moment they land; TTL is the backstop;
  time-windowed verbs are **bucket-quantised** so relative ranges can actually hit
  (Intent §4) with staleness bounded by the bucket, never unbounded.
- **Static shell/ext-UI assets served cache-friendly** — ETag + `Cache-Control: immutable`
  for content-hashed bundles, revalidate for `index.html`. Independent first slice.

## Non-goals

- **No warm/persistent tier in v1.** In-memory only; a restart is a cold cache. The
  SurrealDB `cache_entry` tier is the v2 follow-up, built **only if** post-restart cold
  opens still measurably hurt on the Pi after v1 — and only together with **persisted
  generation counters** (Risks). v1 ships no new table, no sweep, no persisted state.
- **No Redis/memcached/sidecar; no distributed cache.** Each node caches its own reads;
  federation peers are not a cache tier.
- **Not caching motion.** SSE routes, `*.watch`, bus subjects: never cached (state vs
  motion). Live series ticks always bypass.
- **Not caching writes, auth, or anything subject-dependent.** `/auth/*`, `/login`, token
  material never enter the cache; caller-filtered reads are non-cacheable unless
  individually opted in with a safe key.
- **Extension tools are uncacheable in v1 — decided, not deferred.** The host cannot know
  an ext tool's read/write semantics or what dirties it; guessing risks stale or leaked
  responses. The named follow-up is a **manifest-declared cache class** (SDK manifest
  metadata, not a WIT change) so an ext opts its tools in explicitly.
- **Not SSR/HTML caching.** The UI is an SPA — "page cache" = the read-verb responses that
  make a page load, plus asset headers.
- **Not the client-side cache.** Downstream UIs keep their own react-query layer; this is
  the server side of the same problem and composes with it.

## Intent / approach

**A read-through, single-flight cache wrapped around MCP read-verb dispatch, after auth and
the caps check, keyed `{workspace, verb, canonical-args, generation}`, moka-only in v1,
feature-gated end to end. Built in three slices; slice 3 is conditional.**

1. **Placement: after the wall, before dispatch.** A hit and a miss are gated identically —
   the cache can never widen access. The key carries **no token or subject**: a cacheable
   read's response must be a pure function of `{ws, verb, args}`. Verbs whose result varies
   by caller are non-cacheable (allowlist, not denylist).
2. **Verb-class policy, declared at registration.** Each read verb registers its cache class
   (`uncacheable` default | a named class with TTL and an optional **time-window marker**)
   and each write verb registers which classes it dirties — beside the verb, not in a
   hand-maintained map inside the cache. The staleness test sweeps all registered writes
   mechanically. Extension tools have no declaration channel yet ⇒ always `uncacheable`
   (Non-goals).
3. **Invalidation: generation counters, coarse by design.** A write bumps its dirtied
   `{ws, class}` generations; the generation is part of the key, so stale entries become
   unreachable instantly and age out via TTL/eviction — no scan-and-delete. Granularity is
   **per class, not per entity**: `dashboard.save` evicts every dashboard's cached reads in
   that workspace, not just the edited one. That is the right v1 simplicity trade-off —
   stated here so nobody "fixes" it ad hoc; **per-entity generations** (key the record id
   into the generation map) are the named refinement if eviction churn shows up in the perf
   assertion. v1 counters are in-memory only (an `AtomicU64` map): correct because the
   cache is also in-memory — both die together on restart. **Any persistent tier requires
   persistent counters** (Risks).
4. **Canonicalisation, including time-bucket quantisation.** Key-by-JSON with unstable
   field order or `null`-vs-absent is a silent 100% miss; canonical serialisation is
   table stakes. The bigger version of the same bug: **time-windowed verbs receive
   resolved epoch-ms ranges** (a "last 1 hour" panel resolves `$__from`/`$__to`
   client-side before the call), so every open produces different args and `viz.query` —
   the headline verb — would never hit. Fix: a verb class marked *time-windowed* has its
   range args **quantised to the class TTL granularity during canonicalisation and the
   query executed with the bucketed range** (e.g. TTL 30 s ⇒ floor `from`/`to` to :00/:30).
   Cached and computed values always agree with the key; staleness is bounded by the
   bucket, which equals the TTL the class already accepts. Non-time-windowed verbs skip
   this entirely.
5. **Single-flight.** All lookups go through `moka::try_get_with` (or the entry API): the
   first miss computes, concurrent identical misses await the same future. One compute per
   `{key}` under any fan-in — the post-save recompute burst and the many-viewers case
   collapse to one dispatch each.
6. **Slices.** (1) **Asset headers** — ship first, independent. (2) **moka + generations +
   single-flight + quantisation** — this is where ~all the value is; the v1 exit is the
   perf assertion green on Pi-class hardware. (3) **Warm tier** — only if slice-2
   measurements show post-restart cold opens still hurt; requires persisted generations,
   the `cache_entry` table, the sweep, and the SD-write-amplification guard, all scoped
   then, not now.

**Crate choice:** `moka` — maintained, async-aware; per-entry TTL, byte-weigher, and
`try_get_with` are precisely the small-RAM + single-flight controls needed.
**Alternatives rejected:** `quick_cache` (lighter, but no per-entry TTL/weigher/coalescing
out of the box — revisit only if moka's armv7 footprint measurably hurts);
SurrealDB-only materialisation (a store round-trip per hit is most of the cost being
avoided on slow hardware, and it drags the persistence problem into v1).

## How it fits the core

- **Tenancy / isolation:** every key and generation is workspace-scoped. A hit can never
  cross the wall — tested.
- **Capabilities:** middleware runs after the caps check; denied calls never reach or
  populate the cache. `cache.stats` (read) and `cache.purge` (admin) ship in v1 under a new
  `cache:read` / `cache:admin` capability pair with deny paths — a cache you cannot observe
  cannot be tuned or trusted, and purge is the operator's escape hatch.
- **Optimistic concurrency stays safe.** Store reads return `{data, rev}` snapshots; a
  cached read serves the snapshot that was current at its generation, and any save bumps
  the generation — so a client can never obtain, through this cache, a `rev` older than the
  latest save. Rev-conflict semantics on write are untouched.
- **Symmetric nodes:** on/off/budget/TTLs are `BootConfig`, never a role/platform branch.
- **Placement:** either — any node may enable it; the win is largest on edge hardware.
- **MCP surface:** `cache.stats` (get — hit/miss/eviction counts, entry count, weighted
  size, per-class breakdown) and `cache.purge` (per-ws; small, bounded, synchronous — a
  moka invalidate-by-ws + generation bump; not a job). No CRUD/list/feed/batch beyond that.
- **Data (SurrealDB):** **none in v1** — no new table, no persisted state. (v2 warm tier
  would add `cache_entry` + persisted generations; scoped then.)
- **Bus (Zenoh):** N/A — no new subjects; watch/SSE explicitly bypass.
- **Sync / authority:** the node stays authoritative; the cache is droppable (purge/restart
  = empty, correctness unaffected). Offline unchanged.
- **Secrets:** none cached, none in keys.
- **Stateless extensions / hot-reload:** ext tools are uncacheable in v1, so publish/
  upgrade needs no cache hook yet; the manifest-declared-class follow-up must add
  generation-bump-on-publish when it lands.
- **SDK/WIT impact:** **none in v1.** The follow-up's manifest cache-class is SDK manifest
  surface (additive), not WIT.
- **File layout:** middleware, key canonicalisation (+ quantisation), generations,
  verb-class policy, the `cache.*` verbs, and the no-op seam are separate ≤400-line files
  under one gateway/cache folder.
- **Durability:** N/A — cache entries are the opposite of must-deliver; nothing rides the
  outbox.

## Example flow

1. An embedder boots with the feature on and `cache: Some({memory_budget_bytes: 32 MiB,
   ..})`. An operator opens a "last 1 hour" dashboard: each viewer `viz.query`'s resolved
   range is quantised to its 30 s bucket, misses, computes (180 ms), caches under
   `{ws, viz.query, bucketed-args, gen:7}`. The source-list reads cache likewise.
2. Ten more viewers open the same dashboard within the bucket: their quantised keys match;
   in-flight opens await the same computes (single-flight), later opens hit moka. Total
   extra query-engine work: **zero**.
3. `dashboard.save` bumps `{ws, dashboard}` 7→8; dashboard-class reads miss and recompute
   **once** (single-flight absorbs the panel-recompute burst). Untouched `series.*`
   entries keep serving.
4. The node restarts. The cache is cold by design (in-memory tiers and counters die
   together — no stale window); the first open recomputes and re-warms. If Pi measurements
   show this hurts, that is the trigger for the v2 warm tier — not a v1 concern.
5. Same binary, `enabled: false` (or a feature-off build): every step is a plain dispatch —
   identical responses.

## Testing plan

Real embedded node (`mem://` store, real gateway) via the lib API — no mock backend, no
fake cache (`scope/testing/testing-scope.md` §0). Mandatory categories:

- **Capability deny-test:** a caller without the read cap gets the same deny on a warm key
  as a cold one — another caller's cached value is unreachable to them. Deny paths for
  `cache.stats`/`cache.purge`.
- **Workspace-isolation:** same-named dashboards in ws A/B; prime A, read from B → B's
  data. `cache.purge` of A leaves B's entries serving.
- **Staleness-after-write:** for **every registered write verb** (swept mechanically off
  the dirty-map), write → immediate re-read returns the new value — including that a
  cached `{data, rev}` read after a save always carries the new rev.
- **Single-flight:** N concurrent identical cold reads → **one** dispatch, N identical
  responses. Repeat across a generation bump (the recompute burst coalesces).
- **Time-bucket quantisation:** two calls with different resolved ranges inside one bucket
  → one compute, one key; a call in the next bucket → a fresh compute; a non-windowed verb
  is never quantised. Canonicalisation unit tests (field order, `null`-vs-absent).
- **Perf/de-dup assertion (the v1 exit):** instrument dispatch — re-open of a seeded
  dashboard within the bucket runs zero engine calls; after TTL/bucket expiry it
  recomputes. Run on (or emulate) the armv7 target for the go/no-go on the warm tier.
- **Feature-off build:** CI compiles + boots with `page-cache` off and runs the read-path
  suite green (proves the no-op seam).
- **Budget/eviction:** fill past the budget; RSS stays bounded; evicted keys recompute
  correctly.
- **Ext tools uncacheable:** an extension-published read tool dispatches on every call,
  cache on or off.

## Risks & hard problems

- **Invalidation completeness is the whole ballgame.** One write verb missing from the
  dirty-map = silently stale pages. Hence policy-at-registration + the mechanical sweep.
- **Generation persistence is the warm-tier blocker (pre-answered for v2).** In-memory
  counters die on restart; a persistent tier keyed on them either never hits again
  (counters reset) or — worse — serves writes-lost-in-the-gap stale data (counters
  re-seeded from stored rows). A v2 warm tier therefore **requires counters persisted
  write-ahead of (or transactionally with) the store mutation**, an extra store write on
  every dirtying verb. That cost is why the warm tier is out of v1, and any future warm
  tier scope must carry this requirement — it is not optional.
- **Caller-dependent reads are a leak** if cached under a subject-free key. Default
  uncacheable; the build's first task is auditing grant-filtering on every allowlist
  candidate (Decisions) — any subject-filtered verb drops out.
- **Quantisation changes the executed query.** Bucketing rewrites the range the engine
  runs, so the served data can lag "now" by up to one bucket. That is the same bound the
  TTL already imposes — but it must hold: only classes explicitly marked time-windowed are
  rewritten, the bucket must equal the class TTL, and end-exclusivity conventions
  (dashboard ranges are end-day-exclusive) must survive the floor.
- **Coarse generations churn.** Class-level bumps evict sibling entities' entries; heavy
  write traffic could keep hit rates low. Deliberate v1 trade-off; per-entity generations
  are the named refinement, driven by the perf assertion, not taste.
- **Budget honesty.** A weigher that under-counts (misses the serialised value) blows the
  small-RAM budget. Weigh stored bytes; assert RSS in the budget test.
- **External writers** never bump a generation — TTL/bucket is the only defence for
  datasource-backed reads. Keep those TTLs short and documented.

## Decisions (review-resolved — no open questions)

- **Warm tier:** **cut from v1.** Slice 3, conditional on the Pi perf assertion, and
  blocked on persisted generations (Risks). Nothing persistent ships now.
- **Admin surface:** **ship `cache.stats` + `cache.purge` in v1** with deny tests and
  `skills/page-cache/SKILL.md`. **Grammar (build-resolved):** the `cache:read`/`cache:admin`
  pair is realised as two MCP verb caps — **`mcp:cache.stats:call` (read)** and
  **`mcp:cache.purge:call` (admin)** — NOT a new `Surface::Cache`. The generic host-verb
  dispatch authorizes every verb via `Surface::Mcp`; a parallel surface would force the cache
  verbs off that generic path (a rule-10 wrinkle) for no gain. Both ride `ADMIN_ONLY_CAPS`.
- **v1 cacheable allowlist (build-resolved — `viz.query` DEFERRED):** the shipped allowlist is
  the source-picker bundle reads — **`datasource.list`, `series.list`, `flows.list`,
  `flows.get`, `ext.list` (list class, 60 s TTL)** — every one proven caller-independent by
  the build's grant-filtering audit. **`viz.query` DROPPED from v1:** the audit found it
  **subject-filtered** — it re-authorizes each panel target under the caller's own grants
  (`crates/host/src/viz/query.rs:240-265`), so a denied target degrades to an empty frame and
  the result varies by caller; caching it under this scope's subject-free key would leak a
  privileged caller's frames to a co-workspace caller who lacks the target caps. Per this
  section's own "drops out **until keyed safely**" rule, `viz.query` re-enters the allowlist
  only via a **`subject_scoped` cache class** that folds a capability fingerprint (not
  identity/token) into the key. That is the named follow-up; **time-bucket quantisation defers
  with it** (`viz.query` is its only consumer, so v1 ships no quantiser). `dashboard.get/list`
  remain excluded (a single store get is cheap). `ext.list` is admitted but its liveness
  (`running`/`restart_count`) is TTL-bounded, not generation-invalidated (no MCP write dirties
  it) — a generation-bump-on-install/sidecar-transition is a hardening follow-up.
- **Feature default:** **not in lb's default features** (embedders opt in — the
  `external-agent` precedent keeps the stock binary lean); when compiled in,
  `cache: None` ⇒ off. Downstream hosts choose their own default.
- **Extension tools:** **uncacheable in v1**; manifest-declared cache class is the named
  SDK follow-up (with generation-bump-on-publish).
- **TTLs:** lists 60 s (the v1 shipped class). `viz.query` 30 s (= bucket) applies only when
  the time-windowed class ships with the `viz.query` follow-up. Starting points, tuned from
  the perf assertion.
- **Policy home (build-resolved):** the verb-class allowlist + write→dirty map is a declarative
  table (`crates/host/src/cache/policy.rs`), not per-`ToolDescriptor` fields yet — the v1
  allowlist is small enough to state and test by hand. Per-descriptor `cache_class`/`dirties`
  (so the staleness sweep is mechanical over *every* registered verb) is the named
  invalidation-hardening follow-up.
- **Asset headers:** verified/added as slice 1 — the implementing session checks the
  current shell/ext-UI serve path first and only changes what's missing.

## Related

- README `§3` (workspace wall, capability-first, one datastore, state vs motion, symmetric
  nodes), `§6.10` (outbox — explicitly not used here), `§6.13` (SSE routes — bypass).
- `scope/node-roles/embed-node-scope.md` — `BootConfig` is the seam the new field extends.
- `scope/external-agent/` — the precedent for a compile-time-optional cargo feature.
- `scope/testing/testing-scope.md`, `docs/FILE-LAYOUT.md`, `docs/key-stack.md` (moka row).
- Downstream ask that motivated this: a product embedder's page-load latency on
  Raspberry-Pi-class hardware (generic — no embedder is special-cased, rule 10).
- Skill doc: **`skills/page-cache/SKILL.md`** — owned by the implementing session (the
  `cache.stats`/`cache.purge` verbs are a drivable surface), grounded in a live run.
