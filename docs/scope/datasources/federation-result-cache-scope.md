# Federation query-result cache — dashboard page caching

**Topic:** `datasources` · **Name:** `federation-result-cache` · **Status:** scope (the ask).
Promotes to `doc-site/content/public/datasources/` once shipped.

Dashboards re-run **the same SQL on every load, every refresh tick, and for every viewer**. The two
scopes that just shipped removed everything *around* the query — `federation-pool-cache-scope.md`
killed the per-call connect (2,530 ms → ~150 ms warm) and `native-call-concurrency-scope.md` killed
the transport queue (13 queries: 12.68 s → 1.85 s). What remains is the query itself: ~0.9 s per warm
remote query, paid again for an answer the node computed seconds ago. This scope adds a **TTL-bounded
result cache inside the federation child**, so a repeat of an identical query within a
caller-declared freshness window is served from memory — and a dashboard page opens in the time of
one frame round-trip instead of one database round-trip per tile. **Caching is opt-in per call and
can be disabled at three levels** (absent field, per-page setting, node kill-switch).

## Why not DataFusion's cache, DuckDB, or SurrealDB

- **DataFusion is already the engine** (`crates/federation/Cargo.toml`, pinned at 53) — but its
  built-in caches are catalog/listing/file-metadata caches for its own scan paths. It has **no
  cross-query result cache for federated pushdown**, where the whole SELECT ships to the remote
  engine and DataFusion never sees the base tables. There is nothing to turn on; the cache has to
  wrap our own `run_query`.
- **DuckDB — rejected.** It would be a second engine *and* a second persistence layer (README §3
  rule 2), and it solves the wrong term: the cost is the **remote round-trip**, not local compute.
  Anything heavy enough to want a local columnar copy already has the platform's durable answer:
  `federation.mirror` into the series plane. Mirror is the "make it local" tool; this scope is only
  the short-TTL page-speed layer in front of live sources.
- **SurrealDB-persisted results — rejected.** Results are transient warm state, not authority:
  persisting megabytes of frames per query adds write amplification, makes staleness *durable*
  across restarts, and buys nothing a TTL in process memory doesn't. Same resolution as the pool
  cache: SurrealDB holds state, a cache is reconstructible motion (`main.rs` §3.4 header, already
  amended for exactly this distinction).
- **A generic host-level MCP result cache — rejected.** The host cannot know which opaque tool
  calls are pure reads, and the per-tab layer already exists: rubix-ai's dashboard runs one
  react-query client per visit with canonical ws-prefixed keys (`cache/queryKeys.ts`), fetch/shape
  split, and a freeze toggle. What's missing is the layer that survives **across visits, viewers,
  and refresh ticks** — and that belongs next to the cost, in the child.

## Goals

- **Serve a repeat of an identical query from memory** when the caller declares a freshness window
  (`cache: {ttl_s}` on `federation.query`), keyed on the full call identity.
- **Default off.** No `cache` field (or `ttl_s: 0`) → today's behavior, bit for bit. A node-level
  kill-switch (`LB_FEDERATION_RESULT_CACHE=off` in the child's env) forces bypass regardless of
  caller input.
- **Bound it**: entry cap, total-byte cap, per-entry size cap. A cache must never be the reason the
  child grows without limit.
- **Evict on write**: `federation.write` / `federation.migrate` / a failed `probe` against a source
  drop that source's cached results, so the child never serves rows it knows it invalidated.
- **Make it observable**: the existing `event.rs` line gains `result_cache: hit|miss|bypass` and
  `age_ms` on hits — same stderr channel, same secret discipline.
- Keep every verb shape, the pool cache, the pushdown wiring, and the wire protocol **unchanged**
  (the new input field is additive and optional).

## Non-goals

- **Caching `store.query` / `series.read`.** Local SurrealDB answers in milliseconds; caching it
  buys staleness for nothing. If that ever changes it is its own scope at the viz layer.
- **A durable or cross-restart cache.** Respawn loses it; correct, same as the pool.
- **A cross-node / shared cache.** Process-local per `(ws, ext_id)` child, like the pool.
- **Materialized views / scheduled pre-warm.** That is `federation.mirror`'s job.
- **Stale-while-revalidate / serve-stale-on-error.** Attractive for flaky remotes; deferred until
  the plain TTL ships and the live numbers say more is needed (open question).
- **The product-side UI** (per-page cache toggle, "data as of Xs ago" badge). That is a rubix-ai
  scope; this doc defines the contract it drives.

## Intent / approach

A second process-local map in the child, sibling to `pool.rs` and following its shipped pattern
(one file, `results.rs`, per FILE-LAYOUT):

```
key   = (kind, sha256(dsn), sha256(canonical(child input args minus `cache` and `dsn`)))
value = Arc<ResultEnvelope> + stored_at + size_bytes  (+ optional in-flight refresh handle)
```

**Key the whole child-received input, not just the SQL.** Hashing the canonicalized args (minus the
`cache` field and the `dsn`, which keys separately via its own hash) means every field the child
actually receives participates in identity automatically. **But this is a mechanism, not a
structural guarantee**: the child's input is *host-constructed* — `federation/query.rs:56-62`
builds `{kind, dsn, source, sql}` by enumeration — so a future result-shaping field participates
only if the host verb threads it through. The host verb is the seam where a new field can silently
reach neither the query nor the key; every scope that adds one must touch it (see the threading
list under §How it fits → MCP surface). The `source` alias is part of the hashed input and thus of
the key — intended: two aliases over one DSN double-cache (wasteful, harmless), and a rename
invalidates naturally. The DSN is hashed with the same helper `pool.rs` uses; **never stored raw**
(§155).

**Hit path:** entry exists, `now - stored_at <= ttl_s` → return the `Arc` clone, emit
`result_cache:"hit", age_ms` (the pool `cache` field is **omitted** on a result hit — no connect
was consulted, and the event must not imply one). The *caller's* TTL is compared at read time, so
two pages with different refresh intervals share one entry and each gets its own freshness
contract — a 5 s page never accepts a 30 s-old row just because a slower page stored it.

**Miss/refresh path — this is the hard part, specified here, not left to the session.** The pool's
`OnceCell` shape does **not** transfer: `OnceCell` is set-once (`pool.rs:50`), which fits
connections that never refresh, but a result slot must be *re-filled* on expiry — and because TTL
is caller-relative, "expired" is too. The slot is therefore
`{ current: Option<(Arc<Envelope>, stored_at)>, inflight: Option<Shared<future>> }`, with these
rules:

- A caller whose TTL **accepts** `current` returns it immediately — it never waits on an in-flight
  refresh, even if one is running for a stricter caller.
- A caller whose TTL **rejects** `current` (or finds none) **joins** the in-flight refresh if one
  exists, else starts one and installs the shared handle under the map lock. Exactly one query per
  key runs at a time — this is both cold-start single-flight (13 identical tiles → 1 query) and
  the no-racing-refreshers rule; they are the same mechanism.
- A joiner may receive data **fresher** than its TTL required. Always acceptable — fresher than
  asked is never wrong; *staler* than asked is the bug class the join rule prevents.
- Refresh completion replaces `current` and clears `inflight` atomically under the map lock; a
  failed refresh clears `inflight` and leaves `current` untouched (the next rejecting caller
  retries; accepting callers were never blocked).

Never hold the map lock across an `.await` — same discipline as `pool.rs`. Run the query through
the pool cache as today; store the envelope only if it fits the per-entry cap; emit `miss`.

**Eviction:** (a) TTL is lazy — expired entries are overwritten on next miss and swept when the
byte cap is hit; (b) `write.rs` / `migrate.rs` / failed `probe` call `results::evict_source(kind,
dsn_hash)`; (c) oldest-first when over `MAX_RESULT_ENTRIES` or `MAX_RESULT_BYTES` (start: 128
entries / 64 MB / 4 MB per entry — a result bigger than 4 MB is not cached, only logged; the paging
scopes exist so tiles don't fetch 4 MB).

**Who sets `ttl_s`:** the dashboard host. rubix-ai already has the page-level `?refresh=30s`
control (`useAutoRefresh` → `refreshKey`); a page that refreshes every 30 s tolerates ~30 s
staleness, so the refresh interval sizes the TTL and rides into the panel's source args, which
`viz.query`'s `dispatch_target` passes through opaquely (`viz/query.rs:141` clones args; verified).
Pages with refresh off get a per-page/panel setting (product scope). Nothing in lb core decides a
TTL. **State the real staleness bound plainly:** with `ttl_s` = refresh interval, a tick can land
on a 29.9 s-old entry, hit, and serve it for another full cycle — worst case ≈ **TTL + refresh
interval**, not TTL. The product should set `ttl_s` slightly below the refresh interval (e.g.
0.9×) if the tighter bound matters; either way the docs promise the honest number, same as the
external-writer bound below.

**Alternative rejected — cache at `viz.query`'s dispatch in the host.** It would cover all target
tools generically, but the only expensive target is federation; it would put mutable warm state in a
core crate that is deliberately stateless; and it caches *above* the single-flight point, so
concurrent cold tiles still stampede the child. In the child, one mechanism handles hit, miss, and
dogpile.

## How it fits the core

- **Tenancy / isolation:** structural. The child is per `(ws, ext_id)` (`host/src/native/registry.rs`),
  so the cache lives inside one workspace's own process — a cross-workspace hit is impossible by
  construction, not by key discipline. Within a workspace, two members holding
  `mcp:federation.query:call` can share an entry: they receive exactly the rows either would get by
  running the query, since source auth is the DSN, not the member.
- **Capabilities:** unchanged. The gate runs in the host before dispatch, per call, cached or not.
  Deny path: no grant → no call → no cache visibility.
- **Placement:** either; symmetric. No role branch.
- **MCP surface:** no new tool. One additive optional field on `federation.query`
  (`cache: {ttl_s: number}`, sub-schema `required: ["ttl_s"]` — an empty `cache: {}` is rejected by
  schema, decided: the child never invents a default freshness contract). CRUD/list/live-feed/batch
  (§6.1): N/A — no new API shape. **But the field does not ride for free** — the host verb rebuilds
  the child input by enumeration, so shipping this touches four places in the host crate, all
  small: (1) the MCP arg parse for `federation.query` extracts `cache`; (2) `federation_query()`
  gains the parameter; (3) the child-input `json!` at `federation/query.rs:56-62` includes it;
  (4) the input schema in `tools/descriptor.rs` documents it (the schema has no
  `additionalProperties: false`, so validation never rejected it — it was silently dropped, which
  is worse). Verified helpful fact: the host does **not** forward the volatile `ts` it receives, so
  child-side keys are naturally stable across ticks.
- **Data (SurrealDB):** none touched, deliberately (see rejections above).
- **Bus (Zenoh):** N/A.
- **Stateless extensions:** same resolution the pool cache already wrote into `main.rs` §3.4 —
  warm, reconstructible, invisible-in-results state is a cache, not durable state. Extend that
  header note to name the result cache too, or the next reader files it as a violation.
- **Secrets:** DSN hashed for the key (existing helper); events carry `sql_digest` only; the
  envelope stores rows the caller is entitled to anyway. No new secret surface.
- **SDK/WIT impact:** none at the protocol layer — no wire/framing/WIT change, no SDK contract
  change. The change surface is the host federation verb + the child, per the four-touch list
  above; it is **not** child-only, and the session should not discover that mid-flight.
- **Skill doc:** N/A — no new drivable surface. The existing federation/datasources docs gain the
  `cache` field.

## Example flow

Dashboard, 13 federation tiles, page refresh `30s`, two viewers.

1. Viewer A opens the page. 13 `federation.query` calls each carry `cache: {ttl_s: 30}`. All miss;
   single-flight collapses duplicates; the page costs what it costs today: **~1.85 s**.
2. Viewer B opens the same page 10 s later. All 13 hit (`age_ms` ≈ 10,000 ≤ 30,000). Each call is a
   frame round-trip to the child: **the page paints in ~100–200 ms**, dominated by the gateway, not
   the sources.
3. The 30 s refresh tick fires for A. Entries are now stale for the new tick's calls → miss →
   re-query → re-store. Steady state: **each distinct query runs once per TTL window, node-wide,
   regardless of viewer count.**
4. A flow writes through `federation.write` to one source. Its entries evict; the next tick's
   queries against it are fresh inside the window.
5. Operator sets `LB_FEDERATION_RESULT_CACHE=off` on the **node process** and restarts it: the
   child inherits it at spawn (verified — `os.rs:32-38` uses `.envs(env)` with no `env_clear()`),
   every event now says `bypass`, behavior is bit-for-bit today's.

## Testing plan

Per `scope/testing/testing-scope.md` — real SQLite engine, no mocks. The pool-cache session caught
**two vacuously-green tests**; the designs below are chosen so each test *cannot* pass against
broken code, and each must be broken-and-watched-red per the repo rule.

1. **A hit serves the cached rows, provably.** Query with `ttl_s: 60`; `INSERT` a row directly into
   the SQLite file; re-query. Assert the second call returns the **old** row set with
   `result_cache:"hit"` — asserting "fast + flagged" alone is the vacuous version; asserting the
   *absence* of the new row proves the datasource was not touched.
2. **TTL expiry.** Same shape with a short TTL: after expiry the new row appears and the event says
   `miss`.
3. **Bypass in all three forms.** No `cache` field, `ttl_s: 0`, and env kill-switch: the inserted
   row is always visible immediately; events say `bypass`/`miss`, never `hit`.
4. **Write-through eviction.** `federation.write` between two in-TTL queries → second is fresh.
   Break the evict call and watch it red.
5. **Key separation.** Two sources, two SQLs, and one SQL differing only in a paging cursor/limit:
   distinct entries, results never cross.
6. **Bounds.** An over-cap result is not cached (next call misses); exceeding the entry/byte cap
   evicts oldest and stays under the cap.
7. **Single-flight.** N concurrent identical cold queries → the source is queried once. **The
   observable is the stderr event stream, not a side-effecting query** — one cannot exist here:
   both the host gate and the child's `validate_select` (`query.rs:76`) refuse anything but a
   single SELECT. Assert exactly one `miss` event and N−1 `hit`/coalesced events for the key; make
   the race deterministic with a deliberately slow SELECT (SQLite recursive CTE) so all N arrive
   before the first completes. Also assert the refresh rules: with a warm 10 s-old entry, a
   `ttl_s: 5` caller triggers exactly one refresh while a concurrent `ttl_s: 30` caller returns
   the old rows immediately (content-asserted) without waiting.
8. **Restart (hot-reload category).** Kill + respawn mid-window: next query misses, returns fresh
   correct rows — the cache is lost harmlessly.
9. **Capability deny + workspace isolation (mandatory).** Deny path unchanged — re-run the existing
   native deny/ws suites; add one host-level assertion that ws-A's cached query never shows in
   ws-B's child (structural, but pin it — it is the scariest imaginable regression).
10. **Existing suites green** — pool cache, pushdown, concurrency, discovery, sample, write,
    migrate.

## Risks & hard problems

1. **Stale data that looks live.** The defining risk of any result cache. Contained by: opt-in
   only, TTL chosen by the surface that knows its own refresh contract, write-through eviction,
   and `age_ms` on every hit so the UI *can* badge "as of Xs ago". An external writer mutating the
   remote DB is bounded **only** by TTL — that is the contract, and the docs must say it plainly.
2. **The key misses a result-shaping input.** Hashing the child-received args is the right
   mechanism, but it is **not structural**: the child's input is host-enumerated
   (`federation/query.rs:56-62`), so a future field that changes results reaches neither the query
   nor the key unless the host verb threads it. The guard is a review rule at that seam, not the
   hash. Canonicalization must additionally be deterministic (sorted keys, dropped nulls — same
   discipline as rubix-ai's `canon()`).
3. **Memory.** Frames are the biggest objects this child handles. The three bounds (entries, total
   bytes, per-entry) are load-bearing, not tuning; size accounting must measure the serialized
   envelope, not guess. **The 64 MB cap is per child, and children are per `(ws, ext_id)`** — a
   node's worst-case result-cache footprint is 64 MB × active workspaces. Say so in the operator
   docs; a multi-tenant cloud node may want the cap configurable downward.
4. **Dogpile and racing refreshers.** Without single-flight, caching makes the cold case *worse*
   (13 misses race to store). The pool's `OnceCell` does **not** transfer (set-once vs. refill);
   the slot design in §Intent — `current` + shared `inflight` handle, join-on-reject,
   never-wait-on-accept — is the spec, and the session implements *that*, not an approximation.
5. **Vacuous tests.** Already bitten twice in this crate. Every cache test must assert on **row
   content**, never only on latency or flags.
6. **TTL vs the freeze feature.** rubix-ai's freeze toggle pins the *client* key; a frozen panel
   must not be silently unfrozen by a server hit with different rows. Non-issue as designed (frozen
   panels don't re-fetch), but the product wiring session must check it.

## Decided during review (was open)

- **Pass-through:** `viz.query` passes source args opaquely (`viz/query.rs:141`) — but the host
  **federation verb** rebuilds the child input by enumeration (`federation/query.rs:56-62`), so
  `cache` must be threaded through the four host touches listed under §MCP surface. This was the
  scope's original open question #1; the answer is "yes at viz, no at the federation verb."
- **`cache: {}` with no `ttl_s`:** rejected by schema (`required: ["ttl_s"]`). Trivially
  enforceable now that the host parses the field explicitly.
- **Kill-switch delivery:** env inheritance works — `os.rs:32-38` `.envs(env)` with no
  `env_clear()`; the child inherits the node process's environment. No `init`-handshake threading.

## Open questions

None — all decided. The two that remained are now scoped decisions:

- **`federation.sample` / `federation.schema` sharing the cache: NOT in this build.** This build
  ships `query` only; `schema`/`sample` caching is a follow-up slice once the mechanism is proven
  live (each verb's host facade has the same enumerated-input seam — the threading list applies
  per verb). Do not widen this session to chase it.
- **Serve-stale-on-error: NOT in this build.** Plain TTL semantics only. The slot's failed-refresh
  rule (leave `current`, clear `inflight`) is deliberately forward-compatible with adding it later
  with live flaky-source data in hand.

## Related

- `federation-pool-cache-scope.md` — the connect cache this layers on; `pool.rs` is the pattern
  (keying, bounds, OnceCell single-flight, evict lever) and `event.rs` the channel.
- `extensions/native-call-concurrency-scope.md` — removed the queue; with this scope the three
  compose: no connect cost, no transport queue, no repeat query.
- `datasources-scope.md` — DSN/secret rules (§155, §171) the key discipline obeys.
- `federation-pushdown-scope.md` — why DataFusion never sees rows it could cache itself.
- `page-chaining-scope.md` / `federation-paging-scope.md` — cursors ride the args hash; paged reads
  cache per page.
- rubix-ai `ui/src/features/dashboard/cache/` + `useAutoRefresh.ts` — the per-tab layer and the
  refresh control that supplies `ttl_s`; the product-side wiring is a rubix-ai scope.
- `docs/FILE-LAYOUT.md` — `results.rs` is its own file beside `pool.rs`, not a `utils.rs`.
- README §3 rules 2, 3, 4, 6; §6.5.
