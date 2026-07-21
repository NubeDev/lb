# Session: federation query-result cache

**Scope:** `docs/scope/datasources/federation-result-cache-scope.md` ·
**Status:** implemented, tested (49 federation + host suites green; three unrelated federation e2e
suites confirmed pre-existing stack overflows, not this change); **live latency verification pending**

Ships the TTL-bounded result cache inside the federation child, plus the four host touches that make
the `cache: {ttl_s}` field actually reach it. Third and last layer of the same campaign: pool cache
(connect) → call concurrency (transport) → **result cache (the query itself)**.

## What shipped

**1. `crates/federation/src/results.rs` (new, ~340 lines).** The cache, per FILE-LAYOUT — its own
file beside `pool.rs`, not folded into `query.rs`.

- **Key** `(kind, sha256(dsn), sha256(canonical(args)))`, where `args` is the child-received input
  minus `cache` and `dsn`. Canonicalization recursively sorts object keys and drops nulls, so key
  order and explicit-null fields cannot double-store while any real value change misses. The DSN is
  hashed with `pool.rs`'s discipline and never stored raw (§155); the `source` alias IS in the key,
  deliberately (two aliases over one DSN double-cache — wasteful, harmless; a rename invalidates).
- **Slot** `{ current: Option<(Arc<Envelope>, Instant)>, inflight: Option<broadcast::Sender<…>> }`,
  implementing the scope's four rules exactly — **not** `pool.rs`'s `OnceCell`, which is set-once and
  cannot refill. Accept → return immediately (never waits on a stricter caller's refresh). Reject →
  join the in-flight refresh, or become the refresher and install the handle under the map lock.
  Completion replaces `current` + clears `inflight` atomically; a failed refresh clears `inflight`
  and leaves `current` untouched. The map `Mutex` is never held across an `.await`.
- **Bounds** 128 entries / 64 MB total / 4 MB per entry, measured on the serialized envelope. An
  over-cap result is served but not stored (and logged). Every removal routes through one `remove()`
  so the byte counter cannot drift from the map — a drifted counter silently disables the cap.
- **Default off**: no `cache` field, `ttl_s: 0`, or `LB_FEDERATION_RESULT_CACHE=off` → bypass.

**2. Child wiring.** `query.rs` gains `run_query_cached(kind, dsn, sql, source_name, input)` which
wraps the **unchanged** `run_query_with` — pool cache, validation, timeout, and eviction all still
happen exactly as before; the result cache only decides whether that inner future runs at all.
`main.rs` dispatches `federation.query` through it and passes the whole input (which is what the key
is computed from). Eviction hooks in `write.rs` and `migrate.rs` (applied path only) and on a failed
`probe`.

**3. `event.rs`.** `result_cache: hit|miss|bypass` plus `age_ms` on hits. The pool `cache` field is
now `Option` and is **omitted on a result hit** — no connect was consulted, and an event must say
what the call did, not what is true of the child afterwards. Secret discipline unchanged: never the
DSN, never raw SQL.

**4. The four host touches** (`crates/host/`), all of which are required or nothing works:
`federation/tool.rs` parses `cache`; `federation/query.rs`'s `federation_query()` takes it as a
parameter and threads it into the enumerated child-input `json!`; `tools/descriptor.rs` documents it
with `required: ["ttl_s"]` inside the sub-schema.

**5. `main.rs` §3.4 header** amended to name the result cache alongside the pool, with the reason
they resolve the same way and the one respect in which the result cache is stricter (a warm socket
cannot be stale; a cached row can, so it is opt-in and killable).

## Decisions made in-session (the scope did not cover these)

**Eviction lives in `write.rs`/`migrate.rs`, not in the `main.rs` dispatch arms.** The scope said
"`write.rs` / `migrate.rs` / failed probe call `evict_source`" without saying where. Putting it in
the engine functions means *any* path that writes invalidates, rather than one caller of it — the
guarantee belongs to the write. Long-term this is the difference between an invariant and a
convention someone forgets at the next call site.

**`federation_query()`'s new parameter is `Option<&Value>`, and the three non-tool callers pass
`None`.** The verb has four internal callers. `federation.mirror` is explicitly never cached — a
mirror's whole purpose is to copy the source's *current* rows into the series plane, so a cached
answer would make staleness durable, which is precisely what the TTL exists to prevent. `query.run`
and the channel query worker are one-shot reads with no refresh contract. Only the MCP tool surface
can carry a caller-declared window, which is correct: the surface that knows its refresh interval is
the only one entitled to declare staleness.

**A `broadcast` channel rather than `futures::Shared`.** The scope said "shared inflight handle"
without naming a type. `Shared` would need a `futures` dependency in a crate that has none, and
requires the future to be `Clone`-friendly; `broadcast::Sender<Result<Arc<Envelope>, String>>` is in
the `tokio` the crate already links, gives every joiner the same outcome, and a joiner that is
dropped mid-wait cannot block the refresher.

**Miss/bypass emit a second event line rather than back-patching the first.** `run_query_with`
already emits a `federation.query` event with pool state and real elapsed time. Rather than plumb
the result-cache verdict down into it, `run_query_cached` emits its own line carrying
`result_cache`. An operator can therefore count hit/miss/bypass from one field on one event shape.
The cost is two lines per uncached call; the alternative was threading a cache concept through the
uncached path, which would have coupled the two layers.

## Testing

`crates/federation/tests/result_cache_test.rs` (new) covers all 10 categories of the scope's
§Testing plan against **real seeded SQLite files** — no mocks. Every behavioural test asserts **row
content**, and does it by mutating the SQLite file underneath the cache: a hit must show the OLD
rows, a miss/bypass the NEW ones. Latency and event flags are corroborating evidence only, never the
assertion — this crate has shipped vacuously-green cache tests twice.

Coverage: hit-serves-cached-rows · TTL expiry · bypass ×3 (absent field, `ttl_s: 0`, kill-switch) ·
write-through eviction + eviction is per-source not global · key separation (DSN / SQL / paging
cursor / `source` alias) · bounds (over-cap not stored; entry cap bounds the map without evicting the
newest) · single-flight · the refresh rules (accepting caller never waits) · failed-refresh leaves
the entry serving · restart · **workspace isolation** · event reports state without leaking.

### Revert-check (mandatory per repo rule — every test broken and watched red)

| Break | Tests that went red | Result |
|---|---|---|
| Rule 1: accepted `current` never served (`&& false` on the TTL filter) | 8 tests incl. `a_hit_serves_the_cached_rows…`, `an_accepting_caller…` | ✅ red |
| Remove `results::evict_source` from `write::run_write` | `a_write_evicts…`, `a_write_to_one_source…` | ✅ red |
| Rule 2: joiners start their own query instead of subscribing | `concurrent_cold_queries_collapse_to_one` | ✅ red |
| Key ignores args (constant hash) | `distinct_calls_never_share_an_entry`, `args_hash_separates_paging_cursors`, `the_source_alias_is_part_of_the_key` | ✅ red |

### Two false greens caught during this session

Recorded because both would have shipped tests that could never fail — the exact failure mode the
scope's Risk 5 names.

1. **The single-flight tests' slow query was a recursive CTE, and it did not run at all.**
   `validate_select` collects every table name a SELECT references — *including the CTE's own name* —
   and the query path then tries to register `slow` as a real table provider: `no such table: slow`.
   This surfaced as a hard failure rather than a silent pass, so it cost nothing; but the fix
   (a self-cross-join over a real seeded `burn` table) is the durable one, since it references only
   tables that exist and is genuinely pushed down.
2. **The replacement burn query was too fast, which made both single-flight tests vacuous.** At 1,500
   rows the cross-join took ~40 ms and the whole test finished in 0.36 s — the mid-flight `INSERT`
   landed *after* every racer had already completed, so nothing ever raced and the test would have
   passed against a full stampede. Fixed by sizing the burn deliberately (12,000 rows ≈ 750 ms,
   measured, not guessed) **and** adding an assertion that the race outlasted the insert delay, so
   the vacuity cannot silently come back if the box gets faster.

Green output is pasted at the end of this doc.

### A third flake manifestation, found + fixed on this machine

Re-running `result_cache_test` at `--test-threads=16` on this (different) box surfaced a fresh
intermittent failure — the **timing** half of `an_accepting_caller_never_waits_on_a_stricter_callers_refresh`
(`waited < 100 ms` at line 619), distinct from the content half a prior session already fixed. The
accept path is a lockless synchronous return, so that assertion measures only scheduling latency;
under a saturated 16-worker run tokio can leave the (non-blocking) continuation unscheduled past
100 ms. Widened the bound to 350 ms — still under half the ~750 ms refresh, so it catches a genuine
rule-1 block (which would cost the full refresh) while absorbing load-scaled jitter. Verified 3/3
green at `--test-threads=16` after the change. Logged as the third cause in
`docs/debugging/federation/result-cache-tests-flake-under-parallelism.md`.

### The three federation e2e stack overflows are PRE-EXISTING (not this change)

Three lb-host suites — `federation_test`, `federation_sqlite_test`, `query_test` — abort with
`fatal runtime error: stack overflow` (SIGABRT). They are **not** caused by the result cache:

- `federation_test` was already proven pre-existing earlier in the session (stash of the six host
  files, re-run, restore).
- `federation_sqlite_test` and `query_test` re-verified cleanly here (the earlier `query_test`
  stash-and-rerun had produced no result line and was inconclusive). Method: reverted all six host
  touches to the parent commit (`git checkout 18dc1f44~1 -- <the six files>`), ran
  `cargo test -p lb-host --test query_test --test federation_sqlite_test --no-fail-fast`, and both
  aborted with the **identical** `fatal runtime error: stack overflow` / SIGABRT. Restored the files
  to HEAD afterward.

The result-cache commit landed as `18dc1f44 "added ds cache"` on `master` (the working branch named
in the handoff, `fix/native-install-preserves-runtime-net-grants`, had already been merged/renamed —
no work lost). Reverting to `18dc1f44~1` therefore isolates exactly this change's six host edits.

Conclusion: the recursion lives in the shared federation plan path — both the sqlite suites and the
postgres suite hit it, so it is not a driver bug and the sqlite suites give a container-free repro.
Out of scope for this slice; recorded in STATUS.md for a future owner. **No regression to fix.**

## Not verified yet — live

The measurement this scope exists for (**a 13-tile dashboard page repainting in ~100–200 ms instead
of ~1.85 s for the second viewer**) has **not** been confirmed against a running node with a real
remote source. The tests prove the cache is populated, keyed, bounded, invalidated, single-flighted,
and serving the right rows; they cannot prove the latency win, because local SQLite's query cost is
milliseconds. That verification is the next step, same posture as the pool-cache session.

## Scope status

All open questions were already decided in the scope and none needed reopening. The two "NOT in this
build" items were respected: `federation.sample`/`schema` do not share the cache, and there is no
serve-stale-on-error (the failed-refresh rule is deliberately forward-compatible with adding it).
The rubix-ai UI wiring is out of scope by instruction; the contract it needs shipped exactly as
specified — `cache: {ttl_s}` on `federation.query`, `required: ["ttl_s"]`, `age_ms` on hits.

## The honest staleness bound (for the product docs)

With `ttl_s` = the refresh interval, a tick can land on a 29.9 s-old entry, hit, and serve it for
another full cycle — worst case ≈ **TTL + refresh interval**, not TTL. An external writer mutating
the remote DB directly is bounded **only** by TTL. Both numbers are in the public doc as stated,
rather than the flattering version.
