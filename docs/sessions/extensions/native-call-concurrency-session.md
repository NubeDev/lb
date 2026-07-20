# Session: native call concurrency — multiplexing the Tier-2 control line

**Scope:** [`docs/scope/extensions/native-call-concurrency-scope.md`](../../scope/extensions/native-call-concurrency-scope.md)
**Status:** shipped · measured live · exit gate met
**Date:** 2026-07-20

Every tool call to a native (Tier-2) extension was serialized behind one mutex per `(ws, ext_id)`,
with the lock held across the entire round-trip. This session multiplexed **both ends** of the
control line — host and child — and re-measured against the live `pdnsw` remote Timescale source.

**Result: a 13-query dashboard went from 12.68 s to 1.85 s (6.9×).**

---

## Step 0 — the baseline, measured BEFORE any code changed

The scope's original table was step-shaped (0.91 / 4.43 / 4.42 / 11.69 / 11.65), which does not fit a
serial queue and left open the possibility that something else was quantizing the transport. The
scope named `worker_threads = 2` as the leading suspect and required N=2 be re-measured first as the
discriminator. It was:

| N | Scope's original | **Re-measured baseline** | Pure-serial prediction |
|---|---|---|---|
| 1 | 0.91 s | **0.93 s** | 0.93 |
| 2 | 4.43 s | **1.88 s** | 1.86 |
| 4 | 4.42 s | **3.85 s** | 3.72 |
| 8 | 11.69 s | **7.47 s** | 7.44 |
| 13 | 11.65 s | **12.68 s** | 12.09 |

**The step function did not reproduce. N=2 landed at 1.88 s, not ~4.4 s.** The curve is now an almost
exact linear serial staircase, which is the signature of pure transport serialization and nothing
else.

**Conclusion recorded before building:** the "something else is also quantizing this" hypothesis is
**disproven**, and `worker_threads = 2` was **not** a co-cause. The scope's original numbers were
taken *before the pool cache shipped* — they were serialization **plus** ~2.5 s per-call connect
churn, which is what produced the plateaus. With the pool warm, the residual is exactly what this
scope predicted would remain. The planned fix was therefore the right one, and the scope's Risk 8
("the measurement may not fully recover") is retired.

Two measurement traps hit during step 0, both worth recording:

1. **The first baseline was invalid and looked plausible.** I used `ts` as the timestamp column; the
   real column is `timestamp`. Every query was failing, so I was timing error round-trips — and they
   produced a *fast, clean staircase* that would have been reported as a valid baseline. Caught only
   because the harness records a per-caller **row count**, which read `ERR`. This is the same
   vacuous-measurement class the pool-cache session hit twice.
2. **One transient `rows=ERR` at N=13** (4.73 s while 12 siblings took 12.24 s) did not reproduce
   across four subsequent runs. Logged, not averaged away, but not a blocker.

The measurement harness (`measure.sh`) fires N concurrent `federation.query` calls with a **distinct
`LIMIT` per caller**, so every reply is attributable to the caller that asked — the totals alone
would not have caught misrouting.

---

## What shipped

Both ends, landed together. Fixing only the host provably moves nothing: the child awaited each
handler before reading the next frame, so requests would simply queue in the pipe instead of on the
mutex.

### Host side

**`crates/supervisor/src/conn.rs` (new, ~200 lines).** One multiplexed connection per **channel
generation**: a reader task owning the read half, a mutex over the write half, and a pending-reply
map routing replies to `oneshot` waiters by `id`. A call registers its waiter, takes the write lock
for exactly one frame, releases, and awaits unlocked.

Three invariants the file exists to hold, each a silent-corruption bug if broken:

1. **One `Conn` per generation, never per sidecar.** `restart`/`rearm` build a new `Conn` and close
   the old one. `next_id` restarting at 0 therefore cannot collide with a live waiter — the map died
   with the generation. (This is the scope's concrete misrouting mechanism: caller A on id 3 woken by
   the restarted child's id 3, which belongs to caller F. Valid JSON, wrong rows, no error.)
2. **The reader fails ALL outstanding waiters on exit.** Dropping the senders wakes every waiter with
   a transport error — never a silent forever-hang, which is strictly worse than the failure it
   replaces.
3. **Exactly one task reads.** The old `if reply.id != id { continue }` discard became *routing*, not
   filtering.

**`crates/supervisor/src/sidecar.rs`.** `call`/`call_with_caller`/`health` now take **`&self`**
(resolving the scope's open question in favour of `&self` — a `&mut self` API keeps inviting exactly
the bug being fixed). Lifecycle verbs keep `&mut self`, which is correct: they replace the
generation. `Sidecar::conn()` hands out an `Arc<Conn>` so the host can detach from the mutex.

`init` runs **synchronously on the raw channel** before the reader starts (the recommended option —
the bootstrap has no reader to race). `shutdown` registers a waiter like any other call, gets its
reply through the reader, then closes the generation and fails the rest.

**`crates/host/src/native/call.rs`.** The load-bearing change:

```rust
let conn = { let guard = handle.lock().await; guard.conn()? };  // short lock, then RELEASE
tokio::time::timeout(CALL_TIMEOUT, conn.call_with_caller(tool, input, caller)).await
```

`tokio`'s mutex is exclusive regardless of `&self`, so keeping the guard across the round-trip would
re-impose concurrency 1 however well `Conn` multiplexes. **Inlining this into
`handle.lock().await.call_with_caller(..).await` silently restores the entire bug** — noted in the
code, and covered by a revert-checked test.

**Per-call host timeout (`CALL_TIMEOUT = 45 s`)** — promoted from an open question. The child bounds
its own queries at 30 s but the host waited forever. This matters *more* after multiplexing: serially
a stuck call blocked everyone and was obvious; multiplexed it silently pins a waiter and an in-flight
slot. Set **above** the child's 30 s deliberately, so the child's typed error wins and the host bound
is a backstop for a child that has stopped answering at all.

### Child side

**`crates/supervisor/src/serve.rs` (new).** The serve loop lives in the **SDK crate**, not in
federation — every native extension inherits it rather than copy-pasting a reactor (the scope's open
question, resolved to `lb-supervisor` because it already owns the wire types both ends share, so the
ends cannot drift).

- Read a frame, `tokio::spawn` the handler **without awaiting**, keep reading. The existing
  `tokio::spawn` was a panic fence that was *immediately joined* — decorative, not concurrency. It is
  now a real fence.
- Replies go over an mpsc to **exactly one writer task**. Two writers would interleave
  `Content-Length` headers with bodies and desynchronize the stream permanently — an unrecoverable
  failure presenting as random decode errors.
- **`DEFAULT_MAX_IN_FLIGHT = 8`**, an explicit semaphore. Deliberately **not** derived from the pool
  cache's `MAX_ENTRIES = 16`, which counts *distinct warm sources*, not concurrent calls — 13 queries
  on one source is 13 here and **1** there. Sized from what one source's connection absorbs at
  ~0.9 s per warm query.
- `init`/`health`/`shutdown` are answered **inline**, so a health poll cannot queue behind 8 saturated
  tool calls and get the child wrongly declared dead under exactly the load this scope enables.

**`crates/federation/src/main.rs`.** Serial loop deleted, replaced by `lb_supervisor::serve(...)`.
`worker_threads` 2 → **4**, as a decision with a number behind it: the measurement proved 2 was never
the binding constraint, but each of 8 concurrent handlers does real CPU work (Arrow decode + JSON
serialization) on both ends of an await-bound wait. Not 8 — the runtime is not the bottleneck and
idle threads cost memory for nothing.

---

## Exit gate — the re-measured table

Same node, same `pdnsw` source, same 13 queries, pool warm. Two runs, stable.

| N | Before | After | Speedup |
|---|---|---|---|
| 1 | 0.93 s | **0.89 s** | — |
| 2 | 1.88 s | **0.93 s** | 2.0× |
| 4 | 3.85 s | **0.94 s** | 4.1× |
| 8 | 7.47 s | **0.94 s** | 7.9× |
| 13 | 12.68 s | **1.85 s** | **6.9×** |

**N=2 at 0.93 s is the scope's own success criterion** ("if it lands near ~0.9 s the serialization
model was right and the fix worked").

Flat to N=8, then a step: that is `DEFAULT_MAX_IN_FLIGHT = 8` working as designed — N=13 runs as two
waves, visible in the per-call times splitting into a ~0.9 s group and a ~1.8 s group.

**Both numbers, per Risk 9** (the browser caps connections per host, so the curl number is not the
user-visible win):

| Client shape | After |
|---|---|
| 13 unthrottled (`curl`) | **1.81 s** |
| 6-connection cap (browser-like) | **2.60 s** |

Before, both would have been ≥ ~12 s — the serial transport bounds the total from below regardless of
client concurrency. The browser ceiling is real and costs ~0.8 s on top of the transport win; it is a
separate ceiling and not addressed here.

Live verification by the user on the real dashboard: confirmed faster.

---

## Testing

Real children, real store, real capability gate, no mocks. The fake children are in-memory fake
*processes* (mock only the true external) and are deliberately **concurrent and slow** — a fake that
answers instantly or serially makes every concurrency assertion pass whether or not the transport
multiplexes.

Test files (split by scenario per FILE-LAYOUT §4 once the combined file passed 400 lines):
`concurrent_child.rs` (the shared fake child), `concurrent_call_test.rs` (throughput + demux, 3),
`concurrent_lifecycle_test.rs` (generation/lifecycle, 4), `native_concurrent_call_test.rs` (host, 3).

```
test each_caller_identity_stays_with_its_own_call ... ok
test each_caller_receives_its_own_reply ... ok
test thirteen_calls_overlap_instead_of_queueing ... ok
test result: ok. 3 passed; 0 failed

test child_death_wakes_every_waiter_with_an_error ... ok
test init_health_and_shutdown_all_complete_under_the_reader ... ok
test a_restart_never_lets_the_new_child_answer_an_old_waiter ... ok
test repeated_restarts_do_not_leak_reader_tasks ... ok
test result: ok. 4 passed; 0 failed

test capability_deny_holds_under_concurrent_calls ... ok
test the_host_call_path_does_not_serialize ... ok
test workspace_isolation_holds_under_concurrent_calls_with_colliding_ids ... ok
test result: ok. 3 passed; 0 failed
```

### Demultiplexing proven against the REAL remote database, not only the fake child

The strongest end-to-end evidence in the session. 13 concurrent `federation.query` calls to the live
node, each with a **distinct `LIMIT` (401…413)** so every reply is attributable to the caller that
asked:

```
13 concurrent: 1.98 s
valid replies: 13/13
caller 1 -> 401 rows    caller 6  -> 406 rows    caller 11 -> 411 rows
caller 2 -> 402 rows    caller 7  -> 407 rows    caller 12 -> 412 rows
caller 3 -> 403 rows    caller 8  -> 408 rows    caller 13 -> 413 rows
caller 4 -> 404 rows    caller 9  -> 409 rows
caller 5 -> 405 rows    caller 10 -> 410 rows
```

Every caller got **its own** answer against a real remote Timescale source. The misrouting failure
mode this design introduces — valid JSON, wrong rows, no error — would appear here as mismatched row
counts. It does not.

Existing suites unaffected — `sidecar_test` (8), `native_deny_test` (3), `native_isolation_test` (1),
and `native_test` (5, **real OS processes**: kill/restart/rearm/health-decay) all green. The
real-process suite is the strongest existing check on this change, since it exercises kill/restart
against an actual child rather than an in-memory duplex.

> **Prerequisite, not a regression:** `native_test` fails all 5 with *"missing echo-sidecar … run:
> cargo build -p echo-sidecar"* whenever a concurrent `cargo build`/`cargo test --workspace` has
> rebuilt `target/`. Hit twice this session and it reads exactly like a breakage. Run
> `cargo build -p echo-sidecar` first. (Same family as the `make build-wasm` prerequisite.)

### Mandatory categories, re-proven under concurrency

The capability gate and workspace wall were already tested serially. They are re-tested because a
shared pending map is exactly where an authorization result could leak between callers — a deny that
satisfied someone else's waiter would be a capability bypass that *looks* like a successful call.

- **Capability deny under concurrency** — interleaved granted/ungranted callers against one child;
  every deny denied, every allow returning **its own** result.
- **Workspace isolation under concurrency with colliding ids** — ws-A and ws-B fire identical
  sequences simultaneously, so both use the same `id` values at the same time. Each reply must come
  from the caller's own workspace's child (the fake stamps `ws` for exactly this).
- **Per-caller reply identity** — asserted as *identity*, never mere success. Every caller getting a
  valid-but-wrong answer is the failure mode this design introduces and it looks like success.

### Revert-check — every test broken and watched go red

Per the repo rule, and because the pool-cache session found two vacuous tests:

| Revert | Test | Result |
|---|---|---|
| Hold write lock across round-trip (`conn.rs`) | `thirteen_calls_overlap_instead_of_queueing` | ✅ RED — **1.319 s**, the exact serial 13 × 100 ms |
| Hold sidecar guard across round-trip (`call.rs`) | `the_host_call_path_does_not_serialize` | ✅ RED — **1.330 s** vs ~0.1 s |
| Route to an arbitrary waiter (restore filter-not-demux) | `each_caller_receives_its_own_reply` | ✅ RED — *"caller 0 received another caller's reply: echo:8"* |
| ″ | `each_caller_identity_stays_with_its_own_call` | ✅ RED |
| ″ | `child_death_wakes_every_waiter_with_an_error` | ✅ RED |
| ″ | `init_health_and_shutdown_all_complete_under_the_reader` | ✅ RED — health queued 102 ms behind calls |
| Don't close the outgoing generation on restart | `a_restart_never_lets_the_new_child_answer_an_old_waiter` | ✅ RED — *"a pre-restart waiter was ANSWERED across a generation boundary"* |
| Generation-blind retry (drop the `current_gen == first_gen` guard) | `a_noop_recovery_does_not_retry_the_same_generation` | ✅ RED — 2 call frames instead of 1 |
| ″ (the complement must stay green) | `a_real_restart_is_still_retried` | ✅ still GREEN — the guard does not over-fire |

### A bug the new timeout introduced, caught by its own test

Adding `CALL_TIMEOUT` **silently removed restart-and-retry for hung children**. The fault arm matched
only `Transport(_)`; a child that stopped answering used to surface as `Transport` (EOF) or hang
forever, but now surfaces as `Timeout(_)`, which fell through to the catch-all `Err(other) => Err(other)`
— no recovery, no retry, for exactly the case supervision exists for.

Found by `a_real_restart_is_still_retried` failing with `left: 1, right: 2` (the retry never
happened). Fix: `Timeout(_)` joins `Transport(_)` in the fault arm — same condition ("the child is not
answering"), same recovery path. `Child(_)` deliberately stays out of it (an error *reply* over a
healthy line is not a fault — the regression `call.rs` documents in blood).

Worth recording because the timeout was the *safe-looking* part of this change.

### Generation-aware retry (Risk 7) — and two vacuous tests before one that worked

`Sidecar` gained a monotonic `generation()`; the retry now proceeds **only if the installed
generation actually changed**, i.e. something really replaced the child. On the routed
`SidecarDispatch` path (no-op `on_fault` by design — it holds no `Launcher`) nothing recovers the
child, so a generation-blind retry sends a second doomed frame per caller — N of them with N in
flight.

Getting a test to *prove* this took three attempts, and the first two are the instructive part:

1. **Asserted on relaunch count** — vacuous. The spec's restart budget caps relaunches at 5 with or
   without the guard. Measured both ways: **5 and 5**.
2. **Asserted on call frames, via the typed `call_sidecar` path** — also vacuous. That path's
   `on_fault` performs a real `restart()`, which *does* bump the generation, so the guard never fires
   there. Both ways: identical.
3. **Asserted on call frames, driving `call_once_or_restart` directly with a no-op `on_fault`** — the
   actual routed shape. **1 frame with the guard, 2 without.** Paired with
   `a_real_restart_is_still_retried` (2 frames — the retry must still happen after a genuine
   restart), so the pair proves the guard fires *and* does not over-fire.

The lesson repeats the one below: a test that never exercises the path it names will pass for the
wrong reason. Two of three attempts here did exactly that.

**One vacuous test was caught and fixed**, exactly as the scope predicted this design would invite:

`repeated_restarts_do_not_leak_reader_tasks` originally asserted on `num_alive_tasks()`. It **passed
against a deliberately leaking `restart`** — because `Conn::drop` aborts the reader, so a generation
`restart` forgot to close still got cleaned up the moment the last `Arc` went away, and the count
never grew. Rewritten to assert the observable property: the superseded generation must be **dead
while still referenced** (a call on it is refused), which is only true if `restart` actually closed
it. Confirmed RED against the leaking version.

---

## Decisions recorded (scope open questions resolved)

| Question | Decision |
|---|---|
| In-flight cap per child | **Fixed at 8.** Not from `MAX_ENTRIES = 16` — different quantity entirely. Manifest field is the next step if an extension ever needs a different number. |
| Where the child loop lives | **`lb-supervisor`** — it already owns the wire types both ends share, so host and child cannot drift. |
| `Sidecar` API: `&mut self` or `&self` | **`&self`** on the call path. A `&mut self` API keeps inviting the exact bug being fixed. Lifecycle verbs stay `&mut self`. |
| Can a slow tool starve `health`? | **No** — `init`/`health`/`shutdown` are answered inline in the child, outside the semaphore. |
| Per-call host timeout | **45 s**, above the child's 30 s so the child's typed error wins. Not manifest-driven yet. |
| Child `worker_threads` | **4**, with the reasoning above. The suspicion that 2 was a co-ceiling was measured and disproven. |

## Risks retired / still open

- **Risk 8 (measurement may not recover)** — retired. It recovered 6.9×.
- **Risk 9 (browser ceiling)** — confirmed real (~0.8 s), separate, unaddressed here.
- **Still open:** per-call cancellation (a caller that goes away still waits for its reply to arrive
  and be discarded) remains a scope non-goal.

## SDK impact — breaking, and a REAL cross-repo gap (not just a version bump)

**Flagged loudly, per the scope — and it turned out to be bigger than the scope assumed.**

The scope treats "the child-side loop lives in the SDK" as one fact. It is actually **two loops in two
repos**, and only one of them was fixed here:

| Loop | Repo | Used by | State after this session |
|---|---|---|---|
| `lb_supervisor::serve` (new) | **lb** (this repo) | `federation` | ✅ concurrent |
| `lb_ext_native::serve` | **`lb-ext-sdk`** (sibling repo, v0.4.0) | every out-of-tree native ext | ❌ **still serial** |

`lb-ext-sdk/crates/lb-ext-native/src/serve.rs` still awaits `dispatch_call` before reading the next
frame — the exact shape federation's old loop had. So, precisely:

- **The host-side fix benefits every native extension already**, because the per-sidecar mutex is gone
  regardless of which child loop is in use. That is the larger of the two terms.
- **The child-side fix currently benefits only `federation`.** An out-of-tree extension on
  `lb-ext-native` will still self-serialize at the child and see much less improvement, because
  requests queue in its own loop instead of on the host's mutex.

**This is not a version bump — it is a breaking trait change.** `Tools::call` takes **`&mut self`**:

```rust
fn call(&mut self, tool: &str, input: &str) -> impl Future<Output = Result<String, String>> + Send;
```

Concurrency requires `&self` (or interior mutability), so making `lb-ext-native` concurrent changes
the trait every out-of-tree extension implements. That work belongs in `lb-ext-sdk`, which is
**standalone-authoritative** (lb consumes it; it is not a mirror), so it was deliberately **not**
done here.

**Handoff for `lb-ext-sdk`:**
1. Port `lb_supervisor::serve`'s shape — spawn per frame, single writer task, in-flight semaphore,
   `init`/`health`/`shutdown` inline.
2. Change `Tools::call`/`call_with_caller` to `&self`; extensions needing mutation move to interior
   mutability. **Breaking → minor bump to 0.5.0** with a migration note.
3. Provide the `max_in_flight = 1` escape hatch so an extension that must stay serial can opt in
   explicitly rather than relying on an accident of the transport.

Until then the contract change is real but **latent** for out-of-tree extensions: their handlers are
not yet concurrent, so nothing breaks today — but they also do not get the child-side win, and the
moment step 2 lands, any handler relying on the old implicit mutual exclusion breaks. See
`ext-sdk-scope.md` and `native-tier-scope.md`.
