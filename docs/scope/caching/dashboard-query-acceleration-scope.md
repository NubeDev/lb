# Caching scope — dashboard query acceleration (viz.query cache passthrough + subject-scoped gateway cache + batch fan-in)

Status: scope (the ask). Promotes to `doc-site/content/public/caching/caching.md` once shipped.

Opening a real dashboard is slow, and the caches we already shipped don't touch the slow part. Measured
against the live node (`pin node-v0.6.0`, the `pdnsw-iaq-full` board: 24 query tiles, all
`federation.query`): a cold open runs **~27 s serial / ~6 s** through the browser's HTTP/1.1 connection
cap, and a **warm re-open is just as slow** — because the two caches in the stack both miss the panel
path. The gateway response cache (`response-cache-scope.md`) **deliberately excludes `viz.query`** (it is
subject-filtered), and the federation result cache (`../datasources/federation-result-cache-scope.md`)
**works but is opt-in per call** via `cache: {ttl_s}` inside each target's args — and **no caller ever
sends it** (the UI threads no cache directive anywhere). Proven: injecting `cache:{ttl_s:120}` into each
target's args takes the same 24-tile re-open from **26.9 s → 1.2 s (~22×)**. This scope closes the whole
campaign — it makes `viz.query` genuinely cacheable at **both** layers (the per-source result cache *and*
the gateway response cache, the latter via the `subject_scoped` class `response-cache-scope.md` deferred)
and removes the browser connection ceiling with a **batch fan-in** verb — so a dashboard page opens in the
time of a couple of round-trips instead of one database round-trip per tile. The goal is a **hard 10×** on
warm dashboard opens, with the cold-open path (batch + server-side concurrency) close behind.

> Read with: `response-cache-scope.md` (the gateway cache seam + the `subject_scoped`/`viz.query` deferral
> this cashes in), `../datasources/federation-result-cache-scope.md` (the per-source result cache this
> wires a caller to), `../datasources/native-call-concurrency-scope.md` (the host↔child transport
> concurrency already shipped — the server side is not the ceiling; the browser is), `../viz/grafana-
> parity-backend-scope.md` (the `viz.query` resolver + `queryOptions` seam a top-level `cache` rides), and
> the **downstream UI half** in rubix-ai: `docs/scope/frontend/dashboard/dashboard-query-acceleration-
> scope.md` (the caller that must actually send the directive + adopt the batch verb).

---

## Owning repos (cross-repo — WORKFLOW-LB §2)

This scope lands in **two repos** and ships as **two PRs**, one per repo:

1. **`NubeDev/lb` (this repo) — the platform half, the bulk.** The `viz.query` cache passthrough, the
   `viz.query` gateway `subject_scoped` cache class + quantiser, and the `viz.query_batch` fan-in verb.
   Released as a new `node-v0.x.y` tag.
2. **`NubeIO/rubix-ai` — the consumer half.** The UI actually *sends* a freshness directive, adopts the
   batch verb, and adds the per-dashboard freshness setting. It bumps the lb pin to the tag from (1).
   Scoped in `rubix-ai/docs/scope/frontend/dashboard/dashboard-query-acceleration-scope.md`; this doc is
   the authority for the lb contract it consumes.

The two are independently shippable and degrade gracefully: lb (1) is additive (absent directive ⇒ today's
behavior), so its PR merges and tags first; rubix-ai (2) bumps the pin and lights up. Neither is a quick
fix — each is the long-term shape (a source-blind caller directive, a real cache class with a capability
fingerprint, a first-class batch verb), not a patch.

## Goals

- **A hard 10× on warm dashboard opens.** A re-open of an unchanged board within the freshness window
  serves every tile from cache — the federation result cache on a cold node, the gateway response cache on
  a warm one — with the whole page in the time of a couple of round-trips, not 24.
- **`viz.query` carries a caller freshness directive, source-blind.** One top-level `cache: {ttl_s}` on the
  `viz.query` call, threaded by the resolver into **every** target's args before dispatch (exactly as `now`
  and the time-override already are). A `federation.query` target honors it (the shipped result cache); a
  `store.query`/`series.read` target ignores an unknown field harmlessly. No `if source == …` (rule 10).
- **`viz.query` becomes gateway-cacheable** via the `subject_scoped` cache class `response-cache-scope.md`
  named and deferred: a capability **fingerprint** (not identity/token) folded into the key, plus the
  time-bucket **quantiser** (`viz.query` is its only consumer). Warm opens skip the resolver entirely and
  **N concurrent viewers collapse to one compute** (single-flight).
- **Remove the browser connection ceiling with a batch fan-in.** `viz.query_batch {panels[]}` resolves many
  panels **concurrently server-side** (the transport concurrency `native-call-concurrency` already shipped)
  and returns per-panel results — one HTTP round-trip for a page instead of 24 behind a 6-connection cap.
- **Author-controlled freshness, off by default where it must be live.** A per-dashboard freshness setting
  (the UI half) picks the TTL; a live board sets it to 0 (bypass) and keeps streaming.
- **Honest by construction.** Every layer stays behind auth + the caps wall; a cached `viz.query` frame is
  only ever served to a caller whose capability fingerprint matches the key — a denied target can never
  leak across the wall through a warm entry.

## Non-goals

- **No change to what a query returns.** Caching is invisible: a hit returns exactly the rows the resolver
  would have computed. This scope adds no transform, no decimation, no new frame shape.
- **Not a client-side result cache.** react-query's per-visit dedupe stays (rubix-ai owns it); this is the
  server side and composes with it. The UI half only *sends the directive* and *adopts the batch verb*.
- **Not caching motion.** SSE / `series.watch` / `bus.watch` never cache (state vs motion). A live tile
  keeps streaming; the freshness directive applies to the history/backfill read only.
- **No warm/persistent gateway tier.** Still the `response-cache-scope.md` v2 follow-up (conditional on the
  Pi perf assertion + persisted generations). In-memory moka only; a restart is a cold cache by design.
- **No new engine / persistence.** No DuckDB, no SurrealDB-materialized frames — the same lines
  `federation-result-cache-scope.md` already drew. SurrealDB stays the one datastore.
- **Not `federation.mirror`.** Making a source *local* (the durable series-plane copy) is the mirror job's
  charter; this scope is the short-TTL page-speed layer in front of live sources.

## Intent / approach

**Three composable slices, each the long-term shape. Slices 1 & 3 are small and land first; slice 2 is the
architectural add.**

### Slice 1 — `viz.query` cache passthrough (the wire that was missing)

The resolver (`crates/host/src/viz/query.rs::dispatch_target`) already forwards each target's `args`
**verbatim** into `call_tool_at_depth`, so a `cache` object placed in a target's args **already reaches**
`federation.query` and hits the shipped result cache — that is how the 22× was measured. The missing piece
is a **first-class, source-blind caller knob** so the UI sends **one** directive per panel, not one per
target, and so the intent is explicit in the contract rather than smuggled into every args blob:

- `viz.query` accepts a top-level `cache: {ttl_s}` (sibling of `now`/`panel`). The resolver threads it into
  **every** target's args before dispatch — the same mechanism `apply_time_override` and the `now` inject
  already use — **without overwriting** a caller-supplied per-target `cache` (per-target wins; the top-level
  is the default). Source-blind: it is added to the args map unconditionally; a target verb that doesn't
  read it ignores it (result cache is `federation`-only today, and that's fine).
- Bypass is honest: absent, `ttl_s: 0`, or the node kill-switch (`LB_FEDERATION_RESULT_CACHE=off`) ⇒ no
  caching, today's path exactly (the result cache's own contract, unchanged).

This is ~a dozen lines in one file, but it is the *contract* — the caller now has a documented, tested knob
instead of a happens-to-work args field. **Rejected:** telling the UI to hand-inject `cache` into every
`sources[].args` — it works (we proved it) but forks the freshness intent across N args blobs, can't be
defaulted per-page cleanly, and leaves the contract implicit. A named top-level field is the honest seam.

### Slice 2 — `viz.query` gateway response cache via a `subject_scoped` class (the big win on a warm node)

`response-cache-scope.md` dropped `viz.query` from the v1 allowlist because it is **subject-filtered** (each
target is re-authorized under the caller's grants, so a denied target → empty frame and the *result varies
by caller*). Caching it under the existing subject-free key would leak a privileged caller's frames to a
co-workspace caller who lacks the target caps. This slice builds the **named re-entry**:

- A new `Class::VizSubjectScoped` (or a `subject_scoped: bool` on the class) whose key folds a **capability
  fingerprint** — a stable hash of the caller's *relevant* grants (the target caps the panel touches), **not**
  identity, **not** the token — into `{ws, verb, canonical-args, generation, cap_fingerprint}`. Two callers
  with the same grants share an entry; a caller who would get a *different* (denied) frame gets a *different*
  key, so a warm hit is provably the frame *that caller* would have computed. The wall holds by construction.
- The **time-bucket quantiser** `response-cache-scope.md` §Intent 4 scoped and deferred with `viz.query`:
  `viz.query`'s resolved `from`/`to` (epoch-ms, per-open) are **floored to the class TTL bucket** during
  canonicalization and the query runs on the bucketed range, so relative "last 1h" opens actually share a
  key. End-day-exclusivity (`dashboard-time-range-tokens`) must survive the floor.
- **Single-flight** falls out of moka's `try_get_with` (already in the middleware): 10 viewers opening the
  same board in a bucket → one resolve, nine awaits. This is the "N viewers, one compute" win the response-
  cache scope promised but couldn't deliver for tiles.
- Invalidation: `viz.query`'s underlying data is external (federation) — no MCP write dirties it — so it is
  **TTL/bucket-bounded**, like `ext.list` liveness and the external-writer case the response-cache scope
  already documents. Correct and stated, not a gap.

This is the layer that makes a warm-node re-open **microseconds, not one DB round-trip per tile** — it sits
*in front of* the resolver, so it also skips the per-target dispatch + transform, not just the DB.

### Slice 3 — `viz.query_batch` (kill the browser connection ceiling)

Even cache-cold, the browser fires 24 `viz.query` POSTs behind a ~6-connection HTTP/1.1 cap (~4 waves). The
server side is *not* the ceiling — `native-call-concurrency-scope.md` already made host↔child concurrent.
The fix is to stop sending 24 requests:

- `viz.query_batch {panels: [...], now?, cache?}` → `{results: [{refId?, frames, rows, status} | {error}]}`,
  one HTTP call. The handler resolves the panels **concurrently** (bounded `join_all`/semaphore, reusing the
  single-target `viz_query`), each under the **same** verb gate (`mcp:viz.query:call` — no new capability;
  batch is a fan-in of the same authorized verb, not a new privilege) and the same per-target authority.
- **Partial-failure = per-item results** (not all-or-nothing): one bad panel returns its `{error}`/denied
  frame; the rest resolve. A dashboard must never blank because one tile's SQL is wrong.
- **Bounded, synchronous — NOT a job.** The per-frame row cap (10k) and the panel count (a board's tiles,
  tens not thousands) bound the work; it is always-fast fan-in, so it stays a normal call (SCOPE-WRITTING
  §6.1's "small, bounded, always-fast batch may stay synchronous" — stated explicitly, with the bound: a
  hard cap of e.g. 64 panels per batch, over-cap → `BadInput`, the UI chunks).
- Composes with slices 1 & 2: the batch carries the top-level `cache`, and each panel's resolve still checks
  the gateway `subject_scoped` cache first — so a batch of mostly-warm tiles is a handful of computes.

**Why all three, not just one:** slice 1 alone gives ~22× on a **cold** node (result cache) but still pays
the browser connection tax; slice 3 removes that tax but still pays one DB round-trip per cold tile; slice 2
makes a **warm** node nearly free and collapses concurrent viewers. Together they hit the 10× floor across
cold *and* warm, single *and* many viewers. Shipping one is a quick fix; the ask is the durable trio.

## How it fits the core

- **Tenancy / isolation:** every cache key stays workspace-scoped (`response-cache-scope.md`'s invariant);
  slice 2 *tightens* it by adding a capability fingerprint, so a `viz.query` hit is provably per-grant, not
  just per-ws. The batch verb sets ws from the token per panel, never the payload. **Mandatory isolation
  test:** same-named board in ws A/B, prime A, read B → B's frames; and a **cross-grant test** — a caller
  lacking a target cap never receives another caller's warm frame for that target (the fingerprint differs).
- **Capabilities & deny path:** no new capability. `viz.query_batch` rides the existing `mcp:viz.query:call`
  (fan-in of the same verb); a denied target stays an opaque empty frame inside the resolver (unchanged).
  The gateway cache still runs **after** the caps check — a denied call never populates or reads a warm key.
  Deny path test: batch with one denied panel → that panel's frame denied-opaque, siblings resolve.
- **Placement:** either — any node benefits; the win is largest on edge hardware (the whole caching topic's
  motivation). On/off/TTL/budget are `BootConfig.cache` + the result-cache kill-switch, never a role branch.
- **MCP surface (§6.1 — judged):**
  - **New verb:** `viz.query_batch` (batch fan-in, synchronous, bounded, per-item partial failure). No CRUD/
    get-list/live-feed — it is a read fan-in. Long/unbounded reads remain a `federation.mirror` job (N/A here).
  - **Changed verb:** `viz.query` gains an additive top-level `cache: {ttl_s}` (slice 1) — backward-compatible
    (absent ⇒ today). No new cap.
  - **Cache admin:** `cache.stats`/`cache.purge` already ship (`response-cache-scope.md`); slice 2's new class
    surfaces in `cache.stats`' per-class breakdown for free (no new verb).
  - **Live feed:** N/A — motion never caches; a live tile keeps its SSE.
- **Data (SurrealDB):** none new in lb. Slice 2 is in-memory moka; the freshness *setting* is a rubix-ai
  dashboard-record field (its scope), riding `dashboard.save` — no new verb, no new table.
- **Bus (Zenoh):** N/A — no new subjects; watch/SSE bypass.
- **Sync / authority:** cache is droppable (purge/restart ⇒ empty, correctness unaffected); SurrealDB stays
  authority; external federated data is TTL-bounded, never authority. Offline unchanged.
- **Secrets:** none cached, none in keys (the DSN stays mediated in `lb-secrets`, never in a frame or a key).
- **One responsibility per file (FILE-LAYOUT):** the top-level `cache` thread-through in `viz/query.rs`; the
  batch handler in a new `viz/batch.rs` (+ its tool registration in `viz/tool.rs`); the `subject_scoped`
  class + fingerprint in the `cache/` folder (`policy.rs` class + a new `cache/fingerprint.rs` +
  `cache/quantise.rs` for the bucketer), each ≤400 lines. No `utils.rs`.
- **SDK/WIT impact:** **none.** `viz.query`/`viz.query_batch` are host-native verbs over the existing MCP
  contract; the cache is host-internal. No plugin-boundary change. (Flagged loudly per the checklist: this
  does **not** touch the stable ABI.)
- **Rule 10 (no special-casing):** the `cache` thread-through is source-blind (added to every target's args,
  read by whichever verb cares); the fingerprint hashes *whatever* target caps the panel names, no extension
  or source is named in a code branch. A denied target's opacity is enforced upstream, unchanged.

## Example flow

A KFC ops user opens the 24-tile IAQ board, then a colleague opens the same link a minute later.

1. **First open (cold node), batched.** The UI sends **one** `viz.query_batch {panels:[…24…], now,
   cache:{ttl_s:60}}`. The gateway authorizes `mcp:viz.query:call` once, checks the `subject_scoped` cache
   (all miss, cold), and resolves the 24 panels **concurrently**. Each `federation.query` sees `cache:
   {ttl_s:60}` (threaded from the batch) → a cold result-cache miss → one warm-pool DB query (~0.9 s), then
   populates both the federation result cache and the gateway `subject_scoped` entry (bucketed range,
   caller's cap fingerprint). Page paints in ~one concurrent wave, not 24 serial round-trips.
2. **Colleague opens the same link (warm), same grants.** Their `viz.query_batch` hits the gateway
   `subject_scoped` cache for all 24 (same ws, same bucketed range, **same cap fingerprint**) → **zero**
   resolves, zero DB touches — the page is served from memory in microseconds. Ten colleagues at once →
   single-flight collapses any in-flight computes to one.
3. **A viewer with fewer grants opens it.** Their cap fingerprint differs on the two tiles they can't see →
   those two keys miss and resolve (to denied-opaque empty frames, as always); the other 22 hit. No leak.
4. **Author sets the board "live" (TTL 0).** The freshness setting sends `cache:{ttl_s:0}` (bypass); every
   open resolves fresh and the live tiles keep streaming over SSE. Correctness over speed, by choice.
5. **A tile's SQL is wrong.** In the batch its result is `{status:"error", message:"No field named …"}`
   (query-diagnostics); the other 23 render. The board never blanks on one bad tile.
6. **Node restart.** Gateway cache cold by design; the next open re-warms via step 1. No stale window
   (in-memory tiers + counters die together).

## Testing plan

Real embedded node (`mem://` store, real gateway, a **real spawned Postgres/Timescale** seeded with rows —
the one sanctioned fake-boundary, `testing-scope.md` §0). No mocks, no fake cache. Mandatory categories:

- **Capability-deny (§2.1):** `viz.query_batch` denied without `mcp:viz.query:call` (opaque); a batch with
  one denied panel → that panel opaque-empty, siblings resolve; a warm `subject_scoped` key is unreachable
  to a caller who lacks the read cap.
- **Workspace-isolation (§2.2):** same-named board in ws A/B; prime A, read B → B's frames; `cache.purge` A
  leaves B serving. **Cross-grant (the new one):** caller-without-target-cap never receives caller-with-cap's
  warm frame — the fingerprint differs → miss → denied-opaque. This is the headline correctness test for
  slice 2; disable the fingerprint fold and watch it go red (a leak).
- **Passthrough (slice 1):** `viz.query` with top-level `cache:{ttl_s:N}` → the target `federation.query`
  receives it (prime then re-run: 2nd run is a result-cache hit; assert via `cache.stats`/the federation
  event `outcome`); a per-target `cache` **overrides** the top-level; `ttl_s:0`/absent/kill-switch ⇒ bypass
  (mutation-check each).
- **Quantiser (slice 2):** two opens with different resolved ranges inside one TTL bucket → one compute, one
  key; next bucket → fresh; end-day-exclusivity survives the floor; a non-windowed arg is never bucketed.
- **Single-flight:** N concurrent identical cold `viz.query_batch` → one resolve per panel, N identical
  responses; repeat across a generation/bucket boundary.
- **Batch semantics (slice 3):** per-item partial failure (one bad panel, rest OK); the 64-panel cap →
  `BadInput` over-cap; concurrency bound holds (no unbounded fan-out); a batch result equals N single
  `viz.query` results tile-for-tile (parity).
- **Perf assertion (the exit gate — the 10× claim, instrumented):** seed the 24-tile board; a cold batched
  open runs ≤ one concurrent wave of DB queries; a warm re-open within the bucket runs **zero** DB queries
  and **zero** resolver dispatches (gateway hit); assert the wall-clock ratio ≥ 10× warm-vs-cold. Run on (or
  emulate) armv7 for the edge go/no-go, per the caching topic's discipline.
- **Feature-off / bypass:** `page-cache` compiled off **and** result-cache kill-switch on → `viz.query`/
  `viz.query_batch` behave exactly as today (no-op seam proven green).
- **Frontend (rubix-ai, real gateway):** covered in the rubix-ai scope — the board that opens in ~27 s on the
  current pin opens ≥10× faster after the pin bump, two-theme live walk, screenshots by eye.

## Risks & hard problems

- **The capability fingerprint is the whole ballgame for slice 2.** Fold too little (miss a target cap) and
  a warm frame leaks across a grant boundary; fold too much (identity/token) and every caller misses (no
  cache). It must hash **exactly** the target caps the panel dispatches, stably (sorted, canonical), and
  nothing caller-identifying. This is the one place a bug is a security bug — hence the dedicated cross-grant
  deny test with a mutation check, and the fingerprint living in its own reviewed file.
- **Quantiser changes the executed range.** Bucketing rewrites the range the engine runs; served data can
  lag "now" by up to one bucket — the same bound the TTL already accepts, but end-exclusivity + relative-
  token resolution must survive the floor (`dashboard-time-range-tokens`). Only the `viz.query` windowed
  class is rewritten; everything else is untouched.
- **Batch amplifies a pathological panel.** One panel with a huge/slow query in a batch could stall the
  whole fan-in if unbounded. The concurrency bound + per-frame cap + per-query timeout (federation-pool-
  cache already added the timeout) contain it; a slow panel returns its own error, never wedges siblings.
- **Two caches, one truth.** The federation result cache (per-source, DB-level) and the gateway
  `subject_scoped` cache (per-caller-grant, resolver-level) both front `viz.query`. They must not disagree:
  the gateway entry is keyed on the *bucketed* args, and the result cache TTL should be ≥ the bucket so a
  gateway miss that falls through still finds a warm result. Document the TTL relationship; test a gateway-
  miss/result-hit path.
- **Freshness default is a product call, not a default-on.** A too-long default TTL makes live boards look
  stale; too-short buys nothing. The UI half defaults conservatively (short, e.g. 30–60 s, off for boards
  with live tiles) and makes it author-visible — stated in the rubix-ai scope.

## Open questions — RESOLVED (shipped 2026-07-23, lb half)

1. **Fingerprint granularity.** RESOLVED: the **sorted set of target caps the caller HOLDS** among the
   panel's dispatched tools (`cache/fingerprint.rs`), computed with the SAME `gate_tool_for` +
   `authorize_tool` decision the resolver makes per target. Minimal and provably the leak boundary
   (frame content varies by caller only via allow/deny per target; within allow all callers get
   identical rows). Mutation-checked by the cross-grant deny test.
2. **Batch cap N.** RESOLVED: **64** (`viz/batch.rs::MAX_PANELS`); over-cap ⇒ `BadInput`, the UI chunks.
   Concurrency bounded at 16 (`MAX_CONCURRENCY`).
3. **Top-level `cache` vs `queryOptions.cache`.** RESOLVED: **top-level `cache`** sibling of `now`.
4. **Slice 2 covers single `viz.query` too.** RESOLVED: yes — both the single verb and each batch panel
   resolve through the same `crate::cache::dispatch("viz.query")` cached path (the cache wraps the
   resolver, not the batch verb).
5. **Quantiser is two-sided (new).** The host floors STRUCTURED numeric ranges (`now`, `series.read`
   from/to); a `federation.query` target's time lives in its SQL string, which the host never rewrites,
   so the rubix-ai UI buckets the `$__from/$__to` it bakes into SQL to the same `ttl_s`. Both layers
   land on one grid.
6. **Freshness bound = the bucket in the key (new).** An entry becomes unreachable when its bucket rolls
   (a new key); the global moka TTL is the memory backstop. Avoids a per-class per-entry `Expiry`.

## Related

- `response-cache-scope.md` — the gateway cache seam, the `subject_scoped`/`viz.query` deferral + the
  quantiser this scope builds; `cache.stats`/`cache.purge` (slice 2 surfaces in the breakdown).
- `../datasources/federation-result-cache-scope.md` — the per-source result cache slice 1 wires a caller to
  (`cache: {ttl_s}`, the kill-switch).
- `../datasources/federation-pool-cache-scope.md` — the warm-pool connect amortization + the per-query
  timeout that bounds a batched panel; `../datasources/native-call-concurrency-scope.md` — the host↔child
  concurrency proving the server side is not the ceiling.
- `../viz/grafana-parity-backend-scope.md` — the `viz.query` resolver, `queryOptions`, and the per-target
  dispatch this threads `cache` through.
- **rubix-ai** `docs/scope/frontend/dashboard/dashboard-query-acceleration-scope.md` — the consumer half (send
  the directive, adopt the batch verb, the per-dashboard freshness setting) that bumps the lb pin.
- `../dashboard/` (rubix-ai `dashboard-first-paint-scope.md`) — the render-honesty sibling; a fast open and an
  honest empty tile are the two halves of "a dashboard that never looks broken."
- README `§3` (workspace wall, capability-first, one datastore, state vs motion, symmetric nodes), `§6.13`
  (SSE bypass), `§6.10` (jobs — explicitly not used here; batch stays synchronous & bounded).
- Skill: `skills/page-cache/SKILL.md` (owned by `response-cache-scope.md`) gains a `viz.query`/batch section
  on ship — the drivable surface (`cache.stats` showing the new class, a batched warm open) is real.
