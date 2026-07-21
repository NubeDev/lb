# Federation cache — live verification against a real remote

**Topic:** `datasources` · **Name:** `federation-cache-live-verification` · **Status:** verified (2026-07-21)

Both cache scopes shipped and then ended on the same open line:

> *"Still unverified: the live latency win against a real remote source."*
> — `federation-pool-cache-scope.md`, `federation-result-cache-scope.md`

This doc closes that line. It records a hand-driven verification run of the pool cache and the
result cache against **`pdnsw`** — a live, remote Postgres 14 / TimescaleDB 2.11.1 at
`timescale-pdnsw.nube-iiot.com:5434`, ~137 ms RTT, ~33 k BMS points — through the rubix-ai gateway
(`127.0.0.1:8099`), the same authenticating path the UI uses. No mocks; a real external engine
through a real pool.

**Read this alongside the two scopes, not instead of them.** It does not re-specify the mechanisms;
it reports whether they hold on the wire and flags two things neither scope anticipated.

---

## Method (and its limits — read before trusting a number)

- Driven with the `call`/token helpers from `rubix-ai: docs/testing/datasources/README.md`.
- `pdnsw` was **not registered on the node** — a fresh store carries only `demo-buildings`. It was
  added with `datasource.add` (keyword DSN + `endpoint`, per the federation-DSN rule) before testing.
- **Every latency here is `curl` wall-time from outside the node** — it includes gateway + HTTP +
  JSON on top of the child's own `elapsed_ms`. The child's stderr event lines (`cache: hit|miss`,
  `result_cache:`, `elapsed_ms`) go to the `make dev` terminal and were **not** captured this run;
  cache behaviour was inferred from **content and timing**, which is conclusive for correctness (see
  the `now()` proof) but leaves the latency figures gateway-inclusive.
- The node is a **debug build** (`target/debug`). Absolute numbers below are not release numbers.

**These two limits matter for §"Not working well".** The correctness claims stand regardless; the
absolute latencies do not, and the doc says where.

---

## What was verified — the mechanisms hold

### Pool cache — cold connect amortised (pool scope §Goals)

`SELECT 1` against `pdnsw`, six calls in a row:

| run | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| ms | **4368** | 480 | 216 | 711 | 315 | 579 |

Cold connect dominates the first call and is paid once; every later call reuses the warm pool. The
*shape* of the scope's claim (connect once, not per query) holds live. The *magnitude* does not match
the scope's figures — see §"Not working well".

### Result cache — hit proven by **content**, not latency

The scope's risk #5 ("vacuous tests — assert on row content, never only latency/flags"). Probe:
`SELECT now() AS t`, whose value changes on every real execution, so a **repeated timestamp is
positive proof the database was not consulted**.

- `ttl_s: 30`, three calls → byte-identical `2026-07-20T23:40:36.739596132` all three times. **Hit is
  real; the DB was not touched.**
- `ttl_s: 4`, wait 6 s → timestamp advanced. **TTL expiry re-queries.**
- No `cache` field → different every call. **Bypass (absent field) is real.**
- `ttl_s: 0` → no speedup, changes every call. **Bypass (zero TTL) is real.**

That is bypass at two of the three specced levels. The env kill-switch
(`LB_FEDERATION_RESULT_CACHE=off`) was **not** exercised — it needs a node restart, avoided here.

### Single-flight / dogpile — the slot design (result scope §Intent, risk #4)

8 concurrent **identical cold** queries (`cache: {ttl_s: 120}`):

- All returned in **1517–1518 ms** — a **1 ms spread** across 8 racers.
- All 8 bodies byte-identical.

They collapsed onto one upstream query; 7 joined the in-flight refresh rather than stampeding the
remote. The `current` + shared-`inflight` slot works as written.

### Key separation & cross-source isolation (result scope test 5)

- `LIMIT 3` vs `LIMIT 4` on the same table → 3 rows and 4 rows; entries do not cross.
- Same SQL text (`SELECT 1`) on `demo-buildings` vs `pdnsw` → separate entries. The DSN hash keys
  them apart.

### The wedge regression — the test that must go red if the fix is removed (pool scope test 3)

Fired the known-timeout query (`SELECT count(*) FROM histories`, a full hypertable scan):

- Bound fired at **30.17 s** with the typed error: *"query exceeded the 30s bound and was cancelled;
  the connection was dropped."*
- **No starvation.** Throughout the 30 s wedge, `demo-buildings` answered in **0.34–0.52 s** on every
  probe. The original bug — one slow remote wedging every source — does **not** reproduce.
- **Eviction on timeout.** Next `pdnsw` call paid a **3.46 s** reconnect (`miss`), then **0.46 s**
  warm. A timed-out pool is dropped, as specced.

### Secret mediation — still holds

`datasource.list` returns `{name, kind, endpoint, secret_ref: "federation/pdnsw"}` — no DSN, no
password (§155). The added source did not leak.

---

## What is **not** working well — flagged, not fixed

The cache machinery is sound. Two findings sit *underneath* it and one is about the docs.

### 1. Warm latency is 5–10× above the documented floor, and jittery — and the cache hides it

`rubix-ai: docs/testing/datasources/pdnsw/README.md` claims **18–52 ms warm** for `pdnsw` and
**20–50 ms warm** for local `demo-buildings`. This run saw **neither, ever**:

- `pdnsw` warm `SELECT 1`: **216–711 ms** — a **3× spread** on the most trivial query possible, over
  a 137 ms RTT where a warm pool should be ~140 ms and *tight*.
- `demo-buildings` (local SQLite): **0.34–0.52 s**, where the doc says 20–50 ms.

**Both sources are ~10× over the documented warm floor and equally jittery.** A remote-specific
problem would not touch local SQLite; a source-specific problem would not touch both. This points at
the **host / gateway / debug-build path**, not at Timescale. The pool cache turned a ~4.4 s problem
into a ~0.2–0.7 s one — a real win — but in doing so it removed the pressure to explain why the floor
sits 5–10× above RTT. That is the classic thing a cache does to a latency bug: masks it.

**The 9.2 s outlier** the pdnsw README records as *"unexplained, did not reproduce"* is plausibly the
same phenomenon at higher amplitude — constant jitter that occasionally spikes, not a far-end blip.

**This is not proven host-side** — see §Method. The deciding test is cheap and not yet run:

> Capture the child's stderr event lines and compare `elapsed_ms` against `curl` wall-time on the
> same queries. `elapsed_ms` ≪ wall-time → the jitter is host/gateway/debug-build and `pdnsw` is
> fine. `elapsed_ms` ≈ wall-time → it is the connection to `pdnsw`. Re-run on a **release** build
> before drawing a conclusion.

Until that runs, treat the warm floor as **unexplained**, and treat the pdnsw README's warm figures
as **not reproduced**.

### 2. `LB_FEDERATION_ENDPOINTS` is vestigial in `make dev` and reads like an allowlist that isn't one

The federation child inherits `LB_FEDERATION_ENDPOINTS="127.0.0.1:0"` from the rubix-ai `make dev`
script — yet the intercontinental query to `pdnsw` was allowed. Endpoint approval moved to
**per-datasource at `add` time** (`crates/host/tests/datasource_crud_ownership_test.rs:264`
documents exactly this: "added from the UI connects with no boot `LB_FEDERATION_ENDPOINTS` / node
restart"). So the env var no longer gates anything.

Not a security hole — approval still happens, just at a different seam. But a stale boot-time
allowlist env var that no longer allowlists will mislead the next reader. **The fix is in the
rubix-ai `make dev` script, not in lb** — remove or comment the var. Recorded here because the
confusion surfaced during an lb-federation test.

### 3. The `pdnsw` dataset's own limits (not a cache concern, but bounds any demo on it)

- **Data ends Feb 2024** — ~2.5 years stale as of 2026-07-21. A "live BMS" demo on this shows a
  frozen historical window; a "last 24 h" query returns nothing.
- **`histories` is queryable only one `point_uuid` at a time** with a literal equality filter. No
  aggregate, time-range scan, `JOIN`, or `count(*)` survives pushdown — all hit the 30 s bound. For
  33 k points that severely limits what the source can answer.
- Of six sampled points, three had no history; `device_tags` is empty. Pick a point with data.

---

## Verdict

- **Pool cache, result cache, single-flight, timeout+eviction, no-starvation, secret mediation:**
  verified live against a real remote. The "still unverified" line in both scopes is **closed** —
  the mechanisms do what they say.
- **Absolute latency:** the warm floor is 5–10× above the documented figure on *both* sources and
  jittery; unexplained, gateway-inclusive, debug-build. **Do not promote the pdnsw README's warm
  numbers to a public doc until the `elapsed_ms`-vs-wall-time test is run on a release build.**
- **`make dev` env hygiene** and **the pdnsw dataset's staleness/query limits** are real but out of
  scope for the cache work; filed for whoever owns those.

## Follow-ups

> **Run 2 (2026-07-21) closed #1 and #2 — see the dated run section below.** #1's decider ran (on a
> debug build) and points **host-side**: the child's `elapsed_ms` is 2–3 ms warm on both `pdnsw` and
> local sqlite, so `pdnsw`'s warm latency is not the connection. #2's kill-switch and write-through
> eviction both verified by content. The one remainder is re-confirming the pdnsw README's warm
> figures *as wall-time* on a quiet **release** build; the mechanism question is settled. #3 (rubix-ai
> env hygiene) is unchanged.

1. ~~Capture child stderr `elapsed_ms` and re-run §"Not working well" #1~~ — **done (Run 2)**; decider
   points host-side. Still worth a quiet **release**-build wall-time pass before promoting pdnsw's
   warm figures, but the source-vs-host question is closed.
2. ~~Exercise the two paths this run skipped~~ — **done (Run 2)**: kill-switch (`bypass` on every
   event under `LB_FEDERATION_RESULT_CACHE=off`) and write-through eviction (288→289 by content on
   `demo-buildings`). Byte/entry caps still unexercised (would need a >4 MB result).
3. rubix-ai: remove/annotate `LB_FEDERATION_ENDPOINTS` in `make dev`; add a "register `pdnsw` first"
   note to `docs/testing/datasources/pdnsw/README.md` (a fresh store does not carry it). **Open.**

---

# Run 2 — the `elapsed_ms`-vs-wall-time decider + the two skipped paths (2026-07-21)

This run closes every follow-up Run 1 left open. It captured the federation child's own
`elapsed_ms` event lines and compared them to `curl` wall-time on the same queries — the one test
Run 1 could not do — and exercised the two paths Run 1 skipped (the kill-switch and write-through
eviction). Same node, same gateway (`127.0.0.1:8099`), same **debug** build. `pdnsw` and
`demo-buildings` were **already registered** (the store persisted from a prior run — a genuinely
fresh store does not carry `pdnsw`, per follow-up #3, but this one was warm).

## Method — how the event lines were captured (Run 1's blocker)

Run 1's limit was that the child's `elapsed_ms` lines print to the `make dev` terminal
(`/dev/pts/*`), unreadable from outside, and this sandbox **blocks `ptrace`** (strace on the child's
fd 2 → `PTRACE_SEIZE … Operation not permitted`, even for a self-spawned descendant). The fix was to
relaunch the node with its combined stdout+stderr redirected to a **logfile** (`setsid make dev >
node.log 2>&1`), which the child inherits via the supervisor's `Stdio::inherit()`. Every
`{"evt":"federation.query", …}` line then lands in a file this session can `tail`/`grep`. This is the
missing plumbing, not a code change — the events were always emitted; they just had nowhere readable
to go under `make dev`. **Wall-time was still gateway-inclusive `curl` time**; `elapsed_ms` is the
child's own measured query time. The gap between them is exactly what the decider needed.

Host caveat, load-bearing for the *absolute* numbers below: during this run the box was under heavy
CPU contention — a sibling `ems` cargo build **and** a stray `federation` cargo-test were each
pinning multiple cores (load avg ~8.5 on 32 cores). That inflates wall-time jitter on top of the
debug build. It does **not** touch `elapsed_ms` (measured inside the child), which is why the decider
holds regardless.

## THE decider — warm latency is host-side, `pdnsw` is fine

`SELECT 1`, warm, `elapsed_ms` (child) vs wall_ms (`curl`):

| source | run | wall_ms | **elapsed_ms** | ratio |
|---|---|---|---|---|
| `pdnsw` (cold) | 1 | 4540 | 4196 | connect dominates both |
| `pdnsw` | 2–8 | 173–610 | **2–3** | 58–304× |
| `demo-buildings` (local sqlite) | 1–8 | 26–629 | **3** | up to ~200× |
| interleaved A/B | — | pdnsw 149–350 / demo 551–576 | **3 / 3** | — |

The child does the actual query in a **rock-steady 2–3 ms** on *both* sources; all 150–630 ms of
wall-time jitter is above the child. `elapsed_ms ≪ wall-time` by 50–300×. **This is the decider's
"host/gateway/debug-build" branch, unambiguously:**

- A remote-connection problem **cannot** make local SQLite slow — yet `demo-buildings` shows the
  identical 3 ms-child / hundreds-of-ms-wall pattern. The two sources are indistinguishable in
  child-time and indistinguishable in wall-jitter.
- The warm floor Run 1 measured (216–711 ms) was **never `pdnsw`'s connection**. It is the gateway +
  HTTP + JSON + debug-build path (amplified this run by CPU contention), sitting on top of a ~3 ms
  query. Run 1's "5–10× over floor, on both sources, jittery" is fully explained.
- The pdnsw README's "9.2 s unexplained outlier" is consistent with the same host jitter spiking
  under load, not a far-end blip.

**Verdict on the one open question: `pdnsw`'s warm latency is HOST-SIDE, not source-side.** The
remote answers in single-digit milliseconds once warm; the pool cache is doing its job. The pdnsw
README's **18–52 ms warm** figures are still not reproduced *as wall-time* on this debug+contended
box — but they are entirely plausible as the *child's* `elapsed_ms` on a quiet release build, since
that number is already 2–3 ms here. **Debug build + host contention** is the caveat on every absolute
figure; the child-time is not subject to either and is the real cost of the query.

## Cache correctness — by content, `SELECT now()` probe (anti-vacuous)

| test | result |
|---|---|
| `ttl_s:30` ×4 | **byte-identical** timestamp all 4 → hit is real, DB not touched |
| no `cache` field ×4 | 4 distinct advancing timestamps → absent-field bypass real |
| `ttl_s:0` ×4 | 4 distinct → zero-TTL bypass real |
| `ttl_s:4`, repeat <4 s, wait 6 s, repeat | within-TTL identical (`hit`, `age_ms:570`, `elapsed_ms:0`); after 6 s advanced (`miss`) → TTL expiry re-queries |

## Single-flight / dogpile — 8 concurrent identical cold queries, `ttl_s:120`

- All **8 returned byte-identical `now()`** (1 distinct value) → collapsed onto **one** upstream
  execution. Wall spread **0 ms** (all 539 ms).
- Log proof (clean run): exactly **one** pool-level `run_query` (`cache:"hit"`, 1 line with
  `cache!=null`), exactly **one** `result_cache:"miss"` (the leader), **seven** `result_cache:"hit"`
  with `elapsed_ms:0` (joiners took the leader's result). The `current` + shared-`inflight` slot works
  exactly as specced: one query per key, N−1 joined.

## Wedge regression + eviction

- `SELECT count(*) FROM histories` → 30 s bound (`wall 30538 ms`, `elapsed_ms 30009`,
  `outcome:"timeout"`) with the typed error *"query exceeded the 30s bound and was cancelled; the
  connection was dropped."*
- **No starvation:** throughout the wedge, `demo-buildings` answered **115–506 ms** on every probe
  (its normal jittery baseline — unaffected).
- **Eviction on timeout:** next `pdnsw` call was `cache:"miss"`, `elapsed_ms 3316` (wall 3658 ms) — a
  fresh reconnect — then `cache:"hit"`, `elapsed_ms 2–3`. The timed-out pool was dropped.

## The two paths Run 1 skipped — now both closed

### Write-through eviction (follow-up #2b) — content-asserted on `demo-buildings`

`pdnsw` is read-only in practice, but `demo-buildings` (local sqlite) is writable via
`federation.write` (which **is** reachable — the verb validated through `table`→`columns`→`rows` and
the write hit the real sqlite engine, initially failing a genuine FK constraint until a valid
`point_id` was used). Test:

1. `SELECT count(*) FROM point_reading`, `ttl_s:60` → **288** (`result_cache:"miss"`).
2. Repeat within TTL → **288** (`result_cache:"hit"`, `elapsed_ms:0`).
3. `federation.write` one row (valid FK) → `{"affected":1}`.
4. Repeat same query, *still within* `ttl_s:60` → **289** (`result_cache:"miss"`).

The stale 288 was invalidated by the write — `write.rs`'s `evict_source` fires. **Write-through
eviction verified by content**, not latency. (One test-artifact row remains in the dev sqlite store;
federation exposes SELECT + write-insert only, no delete verb — `make purge-store` clears it, at the
cost of the persisted `pdnsw` registration.)

### `LB_FEDERATION_RESULT_CACHE=off` kill-switch (follow-up #2a) — needs a restart, done

Relaunched the node with `LB_FEDERATION_RESULT_CACHE=off` exported (verified present in the node's
`/proc/<pid>/environ`; the child inherits it via `os.rs` `.envs()` with no `env_clear()`).

- `ttl_s:30` ×4 → **4 distinct advancing timestamps** despite the caller asking to cache.
- Every event line said **`result_cache:"bypass"`**, never `hit`. The node-level kill-switch
  overrides caller input, exactly as `federation-result-cache-scope.md` §Goals specifies.
- **The pool cache still hits with the result cache off** (`cache:"hit"`, `elapsed_ms 2–3` after the
  cold reconnect) — the two caches are independent, as designed.

Node was restored to the default (result cache **on**) afterward.

## Run-2 verdict

- **Every Run-1 follow-up is closed.** The decider ran and points **host-side**; the kill-switch and
  write-through eviction both verified by content.
- **`pdnsw`'s warm latency is host-side, not source-side** — the child's query time is 2–3 ms warm on
  both `pdnsw` and local sqlite; the hundreds-of-ms wall-time is gateway + HTTP + **debug build** +
  (this run) CPU contention. **This is the answer to the one question Run 1 could not settle.**
- **Absolute wall-time figures remain debug-build + contention-inflated** — the pdnsw README's warm
  numbers should still be re-confirmed *as wall-time* on a quiet **release** build before promotion,
  but the *mechanism* question ("is it pdnsw?") is now definitively **no**. The `elapsed_ms` evidence
  is the record.
- Secret mediation re-checked on every node instance: `datasource.list` shows only
  `secret_ref: "federation/pdnsw"`, no DSN, no password.

Remaining (unchanged from Run 1, genuinely out of cache scope): the `make dev`
`LB_FEDERATION_ENDPOINTS` hygiene note (finding #2 above) and the `pdnsw` dataset's Feb-2024
staleness / one-`point_uuid`-at-a-time query limits.

## Related

- `federation-pool-cache-scope.md` — the connect cache; this closes its "still unverified" line.
- `federation-result-cache-scope.md` — the result cache + slot design; likewise.
- `federation-pushdown-scope.md` — why `histories` scans time out (§3 is the practical face).
- `rubix-ai: docs/testing/datasources/pdnsw/README.md` — the pdnsw notes; its warm figures are the
  ones this run did **not** reproduce.
- `crates/host/tests/datasource_crud_ownership_test.rs:264` — endpoint approval moved off the boot
  env var (finding #2).
