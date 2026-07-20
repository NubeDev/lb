# Extensions scope — concurrent calls to a native sidecar

Status: **SHIPPED** (2026-07-20). Session:
[`sessions/extensions/native-call-concurrency-session.md`](../../sessions/extensions/native-call-concurrency-session.md).

> **Shipped result — the exit gate.** Re-measured on the same live node/source: **13 queries went
> 12.68 s → 1.85 s (6.9×)**, flat to N=8. **N=2 landed at 0.93 s**, this scope's own stated success
> criterion.
>
> **The step-shaped table below did NOT reproduce when re-measured before building.** The baseline
> came back an almost exact linear serial staircase (0.93 / 1.88 / 3.85 / 7.47 / 12.68) — **N=2 at
> 1.88 s, not ~4.4 s**. The original numbers were taken *before the pool cache shipped*: they were
> serialization **plus** per-call connect churn, which is what produced the plateaus. So
> `worker_threads = 2` was **not** a co-cause (cause §3 below is retired as a *ceiling*, though the
> value was still set deliberately, to 4), and **Risk 8 is retired** — the measurement recovered.
> The rest of this scope's diagnosis was confirmed correct and is preserved as written.

Every tool call to a native (Tier-2) extension is **serialized behind one mutex per `(ws, ext_id)`**,
and the lock is held across the entire round-trip to the child — including a remote database query
~137 ms away. Concurrency to a native extension is effectively **1**, node-wide, per extension. A
dashboard issuing 13 federation queries does not run 13 queries; it runs one, thirteen times, and
every caller waits for the last. This scope makes the native control line **multiplexed** so N
in-flight calls to one child overlap.

Measured on a live node (`rubix-ai`, 13 sources across 6 panels, all queries warm):

| Concurrent queries | Total wall-clock |
|---|---|
| 1 | 0.91 s |
| 2 | 4.43 s |
| 4 | 4.42 s |
| 8 | 11.69 s |
| 13 (full dashboard) | 11.65 s |

The proof is not the totals but their shape: firing all 13 at once, **every single query reported
~11.6 s and they all completed at the same instant**. That is not thirteen slow queries — it is
thirteen queries in one queue, each billed for the whole queue.

**But the totals do not fit a pure serial queue, and that discrepancy is load-bearing.** A single
serial queue predicts linear growth — 0.9 / 1.8 / 3.6 / 7.2 / 11.7. Observed is 0.91 / 4.43 / **4.42**
/ 11.69 / **11.65**: N=2 is 2.4× worse than serialization alone predicts, and the pairs (2, 4) and
(8, 13) plateau at identical values. Only the N=13 figure coincidentally matches 13 × 0.9. That is a
step function, not a queue — so serialization is **a** cause, not provably the **whole** cause. See
"third ceiling" below and Risk 8. **Re-measure N=2 first**: it is the cheapest discriminator between
"transport serialization" and "something else is also quantizing this."

## The cause — two serialization points, not one

**1. The host holds the sidecar lock across the await** (`crates/host/src/native/call.rs:39-42`):

```rust
let first = {
    let mut sidecar = handle.lock().await;
    sidecar.call_with_caller(tool, input, caller.clone()).await   // ← whole round-trip
};
```

`SidecarMap` (`crates/host/src/native/registry.rs:23`) holds one `Arc<AsyncMutex<Sidecar>>` per
`(ws, ext_id)`, so **every** `federation.query` in a workspace contends on that one mutex regardless
of which datasource, table, or point it targets.

**2. The child's own loop is equally serial** — and this is the part that makes the obvious fix a
no-op. `crates/federation/src/main.rs` reads a frame, `.await`s `handle_call` to completion, writes
the reply, and only *then* reads the next frame. The `tokio::spawn` there is a **panic fence, not
concurrency**: it is immediately awaited.

So the transport is serial at both ends. **Removing the host mutex alone would change nothing** —
requests would simply queue in the pipe instead of on the lock. Any fix must address both, or it
ships a measurement that does not move. This is the single most important finding in this scope, and
it is the one the original diagnosis missed.

**3. A third ceiling sits behind both: the child's runtime is 2 threads.**
`crates/federation/src/main.rs:41` declares `#[tokio::main(flavor = "multi_thread", worker_threads = 2)]`.
The child-side fix (spawn a handler per inbound frame) inherits that ceiling — 13 spawned handlers
still land on 2 workers. For await-bound work (a remote query is mostly waiting on a socket) 2 workers
is not obviously the binding constraint, but it is the leading suspect for the step-shaped measurement
above, and it is *not* addressed by either fix. **Size it deliberately as part of this scope** rather
than discovering it after the transport lands and the table barely moves.

A fourth, smaller term rides along: `Sidecar::request` (`crates/supervisor/src/sidecar.rs:169`, the
discard at `:186`) reads replies in a loop and **discards non-matching ids**
(`if reply.id != id { continue; }`). Under a single-caller-at-a-time design that is harmless. Under
multiplexing it is **data loss** — it would throw away another caller's reply. It must become a
demultiplexer, not a filter.

### The id allocator resets on restart — a concrete misrouting mechanism

`Sidecar::restart` (`sidecar.rs:118`) and `Sidecar::rearm` (`sidecar.rs:153`) both set
`self.next_id = 0`. Today that is harmless: one caller, one outstanding request, no way to confuse
generations. **Under a pending-reply map it is a correctness bug.** A restart with calls in flight
produces fresh ids starting at 0 that **collide with live waiters from the dead generation** — caller
A waiting on id 3 is woken by the restarted child's id 3, which belongs to caller F. Valid JSON,
wrong data, no error. This is the concrete mechanism behind Risk 2, and it is invisible until it
happens under load.

The pending map must therefore be **generation-tagged** (or fully drained with every waiter failed)
on every `restart`/`rearm`, before the new channel accepts a single frame. Test 5 asserts this
directly.

## Goals

- **Overlap N in-flight calls to one native child**, so `federation.query` concurrency is bounded by
  the source and the child's own runtime, not by the host's transport.
- **Hold no lock across a round-trip.** Locks may be held long enough to write a frame or register a
  waiter; never across the child's work.
- **Correlate replies by `id`**, so a reply reaches the caller that asked — never another, and never
  the floor.
- **Bound the in-flight set** per child, so a stampede queues explicitly rather than spawning
  unboundedly inside the child.
- Keep the wire protocol, the `Sidecar` public API shape, supervision/restart semantics, caller
  identity stamping, and every verb's contract **unchanged**.

## Non-goals

- **A sidecar process pool** (N children per extension). It multiplies memory, connection pools, and
  restart bookkeeping, and it does not fix the child's serial loop — which is the actual second
  bottleneck. Revisit only if one child's runtime proves to be the ceiling after this ships.
- **Ordering guarantees between concurrent calls.** They were never ordered across callers; they are
  independent tool calls. A caller needing order awaits its own calls.
- **The wasm (Tier-1) path.** Different runtime, different dispatch, no stdio framing.
- **Dashboard-side query consolidation.** Real and worth doing (13 → ~5 by collapsing per-series
  queries into `point_uuid IN (...)` with a `GROUP BY`), but it is a **dashboard-definition change in
  the product repo**, not an lb change. It reduces the load; it does not fix the transport, and any
  multi-panel dashboard on any node pays this until the transport is fixed. Tracked separately.
- **Per-call cancellation.** A caller that goes away still waits for its reply to arrive and be
  discarded. Worth doing later; not needed to fix throughput.

## Intent / approach

**Multiplex the control line.** One child, one connection, many in-flight requests correlated by the
`id` the protocol already carries. Both ends change:

**Host side** — replace "lock the sidecar, do the round-trip" with a **writer half + a pending-reply
map**:

- One background **reader task** per sidecar owns the read half. It reads frames forever and routes
  each reply to a `oneshot` sender taken from a `HashMap<u64, Sender<Reply>>`.
- A call registers its `oneshot` in the map, takes a short lock to write its request frame, releases
  it, and awaits its receiver. The only mutually-exclusive section is the frame write — microseconds,
  not a network round-trip.
- The `id` allocator moves behind the same short lock (it already increments per call), and **stops
  resetting to 0 on restart** — or the map is generation-tagged so a reset cannot collide (see above).

**The reader task's lifetime is per-channel-generation, not per-sidecar.** `Sidecar` holds
`channel: Option<Channel>`, and `restart`/`rearm`/`shutdown` replace or drop it. Each new channel gets
a new reader task; the outgoing one must be shut down and joined, and the pending map drained, as part
of the same transition. This is where a task leak per restart would hide (test 7).

**The lifecycle verbs are part of this change, not bystanders.** `spawn` (the `init` handshake),
`health`, `shutdown`, `restart`, and `rearm` all route through the same `Sidecar::request` being
replaced. Two need explicit answers before coding:

- **`init` is a bootstrap ordering problem.** `Sidecar::spawn` sends `init` and awaits its reply
  *before* any reader task can meaningfully exist. Either the handshake runs synchronously on the raw
  channel and the reader task starts after it returns (simplest, recommended), or the reader starts
  first and `init` registers a waiter like any other call.
- **`shutdown` writes, reads a reply, then kills the channel.** With a reader task owning the read
  half, `shutdown` cannot read its own reply — it must register a waiter, then stop the reader and
  fail any remaining waiters before killing.

`health` is an ordinary correlated request and needs no special handling beyond the in-flight cap
(see Open Questions on starvation).

**Child side** — the loop must dispatch and keep reading. Read a frame, `tokio::spawn` the handler
**without awaiting it**, and have each handler send its reply over an mpsc channel to a single writer
task (frames must not interleave on stdout, so exactly one task owns the write half). The existing
panic fence is preserved — it becomes a real fence rather than a decorative one, since the spawned
task is no longer immediately joined.

This is a shared concern, not a federation one: the loop belongs in the **SDK/`lb-supervisor` side**
so every native extension gets it, rather than each child re-implementing a reactor. Extensions that
want to stay serial can bound their own in-flight count.

**Alternative considered and rejected: a process pool.** Spawning N children per extension would let
the existing serial code stand. Rejected because it multiplies resident memory and connection pools
per extension, complicates restart/supervision accounting (which child is "the" child for
`native_status`?), and — decisively — **does not remove the per-child serial loop**, so it buys
concurrency N instead of unbounded, at N× the cost. Multiplexing one line is cheaper and correct.

**Alternative considered and rejected: keep the mutex, shorten the critical section by writing the
request under lock and reading the reply outside it.** This is the smallest diff, but it is
*incorrect* without a demultiplexer: two callers reading the same stream would steal each other's
replies (see the `reply.id != id { continue }` note above). The reader task is not optional.

## How it fits the core

- **Tenancy / isolation:** unchanged and structural. The map key stays `(ws, ext_id)`; a ws-B call
  still resolves ws-B's child or `None`. Multiplexing happens *within* one `(ws, ext_id)` line — it
  never merges two workspaces' traffic. **The pending-reply map is per-sidecar, never node-global**,
  so an id collision cannot cross a workspace. This is a hard requirement, and a test.
- **Capabilities:** unchanged. `mcp:<id>.<tool>:call` is gated *before* dispatch, per call, exactly
  as today. Concurrency does not touch the gate — but the deny path must be re-tested under
  concurrent load (a shared pending-map is exactly where an authorization result could leak between
  callers if reply routing were wrong).
- **Placement:** either. Symmetric — no `if cloud`. This is transport, identical on every node role.
- **MCP surface:** **no new tools, no changed tool shapes.** This is entirely below the MCP contract.
  §6.1 CRUD/get-list/live-feed/batch are all N/A: the feature adds no API surface.
- **Data (SurrealDB):** none. The pending map and reader task are runtime-only motion, like
  `SidecarMap` itself (§3.3/§3.4). No records, no schema.
- **Bus (Zenoh):** N/A — this is a process-local stdio channel, not bus traffic.
- **Secrets:** unchanged. The DSN still arrives per call and lives only inside the child's pool.
- **Stateless extensions:** preserved. In-flight calls are motion; a restart drops them (see Risks)
  and the child rebuilds from spec.
- **SDK/WIT impact:** **flag loudly.** The child-side loop lives in the native SDK, so this changes
  the **stable native-extension contract** — every out-of-tree native extension inherits a new
  concurrency posture. It must ship with an SDK version bump and a migration note: handlers that were
  implicitly serialized are now genuinely concurrent, so any handler relying on that accidental
  mutual exclusion breaks. See `native-tier-scope.md` and `ext-sdk-scope.md`.
- **Skill doc:** N/A. This exposes no agent-/API-drivable surface — no MCP verb, no gateway route, no
  automatable task. It changes the performance of surfaces that already have their own docs.

## Example flow

A dashboard loads and issues 13 `federation.query` calls to one workspace's `federation` child.

1. 13 gateway requests arrive; each passes its own `mcp:federation.query:call` capability gate.
2. Each reaches `SidecarDispatch::call_tool` and resolves the **same** `Arc` handle for
   `(ws, "federation")`.
3. Each allocates an id, registers a `oneshot` in that sidecar's pending map, takes the write lock
   for the duration of one frame write, and releases it. Thirteen frames are on the wire in
   microseconds.
4. The child reads all 13 frames without blocking, spawning a handler per frame. All 13 hit the
   federation pool cache (`federation-pool-cache-scope.md`) and run against the source concurrently.
5. Replies come back **out of order**. The reader task routes each by `id` to its waiter.
6. Each caller wakes with its own reply.

Expected: **~11.6 s → roughly the slowest single query** (~0.9 s warm), bounded now by the source and
the in-flight cap rather than by the transport. Composes with the pool cache: that scope removed the
per-call connect, this one removes the queue in front of it.

## Testing plan

Per `scope/testing/testing-scope.md` — real children, real store, no mocks. The mandatory categories:

1. **Concurrency is real, measured structurally.** Issue N concurrent calls to one sidecar whose
   handler sleeps a known duration. Assert total wall-clock is far closer to *one* duration than to
   N — and, critically, assert the **completion timestamps differ**. The live symptom was "all
   thirteen finished at the same instant"; that is the shape to pin, not a raw total.
2. **Replies are correctly demultiplexed.** Concurrent calls with *distinct* inputs must each receive
   *their own* result. Every caller getting a valid-but-wrong answer is the failure mode this design
   introduces and it would otherwise look like success. Assert identity per caller, not just success.
3. **Capability deny under concurrency (mandatory).** Interleave authorized and unauthorized calls to
   the same child; assert every deny is denied and every allow returns its own result. A deny must
   never satisfy another caller's pending waiter.
4. **Workspace isolation under concurrency (mandatory).** Concurrent calls from ws-A and ws-B resolve
   distinct children and distinct pending maps; assert no reply crosses, including when both use the
   same `id` values (they will — ids are per-sidecar).
5. **Fault + restart mid-flight.** Kill the child with N calls in flight. Assert every waiter is
   woken with a transport error (**none hangs forever** — an orphaned waiter is the worst outcome
   here), the restart discipline still fires per `call.rs`'s existing policy, and `Child(_)` error
   replies still do *not* trigger a restart (the regression `call.rs:46-48` documents in blood).
   **Additionally assert the generation boundary**: with waiters outstanding on ids 0..N, restart the
   child (which resets `next_id` to 0) and assert no post-restart reply can satisfy a pre-restart
   waiter. Construct this deliberately — it is the id-collision bug above, and it will not appear by
   accident in a test that kills the child cleanly.
6. **In-flight cap.** Past the bound, calls queue rather than growing unboundedly; assert the cap
   holds and queued calls still complete.
7. **Hot-reload / lifecycle (mandatory).** `stop`/`restart`/`status` still behave with the reader
   task running; no task is leaked per restart (assert across repeated restarts).
8. **Lifecycle verbs under the reader task.** `init` handshake, `health`, and `shutdown` each still
   complete correctly with the reader task owning the read half — `shutdown` in particular must get
   its reply and then fail any remaining waiters rather than orphaning them.
9. **Existing suites stay green** — native-tier, caller-identity, supervision-reactor, federation.

**Exit gate is a re-measured table, not a green suite.** Re-run the same 13-query dashboard at
N = 1, 2, 4, 8, 13. **N=2 is the diagnostic**: if it lands near ~0.9 s the serialization model was
right and the fix worked; if it stays near ~4.4 s, transport was not the dominant term and the child's
`worker_threads`/pool behaviour is the real ceiling (cause §3). Report both the `curl` number and the
browser page-load number (Risk 9).

Per the repo rule: **break each fix and watch the test go red.** Specifically — re-serialize the host
(test 1 red), and restore `if reply.id != id { continue }` in place of routing (test 2 red). A
concurrency test that passes against serial code is worthless, and the *only* way to know is to run
it against serial code. Note that the federation pool-cache session found **two tests that passed
vacuously**; this scope's tests are more prone to that failure, not less.

## Risks & hard problems

1. **Fixing only the host and measuring nothing.** The child's serial loop means the obvious
   host-side fix produces a ~0% improvement, which reads as "the diagnosis was wrong" rather than
   "the fix was half-applied". **Both ends, or don't ship.** Land them together and measure only the
   pair.
2. **Reply misrouting is silent and severe.** A demultiplexing bug hands caller A caller B's rows —
   valid JSON, wrong data, no error, across a workspace boundary if the map were ever shared. This is
   the most dangerous defect in the scope. It is why the pending map must be per-sidecar and why
   test 2 asserts per-caller identity rather than mere success.
3. **Orphaned waiters.** If the reader task dies (child death, panic, decode error) without draining
   the pending map, every waiter hangs forever — and unlike today's failure, it hangs *silently*.
   The reader must, on exit, fail **all** outstanding waiters with a transport error. Test 5.
4. **The child's new concurrency is unbounded by default.** Spawning a task per inbound frame lets a
   stampede open N simultaneous database connections, which is how a fixed transport becomes a
   different outage. Needs an explicit in-flight semaphore.
   **Do not size it from the pool cache's `MAX_ENTRIES = 16`** — that cap counts *distinct sources
   held warm*, not concurrent connections. Thirteen concurrent queries against one source is **one**
   pool entry. The two numbers are unrelated; the in-flight cap must be sized from what one source's
   connection can absorb, and from the child's `worker_threads` (cause §3).
5. **The SDK contract changes.** Out-of-tree native extensions written against implicitly-serialized
   handlers may hold assumptions that break the moment handlers overlap. This is a breaking change to
   a stable boundary and needs a version bump plus a migration note, not a silent improvement.
6. **stdout interleaving.** Exactly one task may own the child's write half. Two concurrent writers
   would corrupt `Content-Length` framing and desynchronize the stream permanently — an unrecoverable
   failure that looks like random decode errors. The mpsc-to-single-writer shape is not optional.
7. **Restart racing in-flight calls — and the routed path recovers nothing.** `call.rs`'s retry-once
   path assumes it is the only caller. With N in flight, a fault wakes N waiters with `Transport` and
   triggers N recoveries; the recovery must be idempotent per child generation or restarts multiply
   under load, precisely when they are most harmful.
   Worse on the routed path: `SidecarDispatch` supplies a **no-op** `on_fault` (`call.rs:116`, by
   design — the adapter holds no `Launcher`). So on child death all N callers retry against a
   **still-dead child** and each burns a second transport error for nothing. Today that costs one
   wasted retry; after this it costs N. The retry needs to be generation-aware: a caller whose
   generation is already known-dead should fail fast rather than re-attempt.
8. **The measurement may not fully recover.** The 0.9 → 4.4 → 11.7 steps are serialization *plus* the
   pool churn the pdnsw README documents. With the pool cache shipped, this should be the dominant
   remaining term — but that is a prediction, not a result. The exit gate is a re-measured table
   against the same 13-query dashboard, not a green test suite.
9. **Browser connection limits are a separate ceiling.** The measurements were taken via `curl`
   against the gateway. The browser caps concurrent connections per host (and lb has a documented
   history here — see the SSE pool-exhaustion work), so the observed page load may improve less than
   the transport does. Measure both; do not report the curl number as the user-visible win.

## Open questions — RESOLVED (2026-07-20)

All six were decided while building; the reasoning is in the session doc.

- **In-flight cap per child** → **fixed at 8** (`lb_supervisor::DEFAULT_MAX_IN_FLIGHT`), sized from
  what one source's connection absorbs at ~0.9 s per warm query. Explicitly **not** derived from the
  pool cache's `MAX_ENTRIES = 16` — different quantity (warm *sources* vs concurrent *calls*; 13
  queries on one source is 13 here and 1 there). A per-extension manifest field is the next step if
  one ever needs a different number. `serve_with(.., max_in_flight)` is the escape hatch.
- **Where the child-side loop lives** → **`lb-supervisor`** (`serve.rs`). It already owns the wire
  types both ends share, so host and child cannot drift; every native extension inherits the reactor
  instead of copy-pasting one.
- **`Sidecar` API** → **`&self`** on the call path (`call`/`call_with_caller`/`health`), as this scope
  leaned. Lifecycle verbs (`restart`/`rearm`/`shutdown`) keep `&mut self` — they replace the
  generation, so exclusivity is the correct semantics there. The host additionally detaches an
  `Arc<Conn>` via `Sidecar::conn()` so its per-sidecar mutex is never held across a round-trip.
- **Slow tool starving `health`** → **no.** `init`/`health`/`shutdown` are answered **inline** in the
  child, outside the semaphore, so a health poll cannot queue behind 8 saturated calls and get the
  child wrongly declared dead under exactly the load this scope enables. Regression-tested.
- **Per-call host timeout** → **45 s** (`CALL_TIMEOUT`, `host/src/native/call.rs`). Deliberately set
  **above** the child's own 30 s query bound rather than under it: the child's typed "query exceeded
  the 30s bound" error is a better answer than an opaque host timeout, so the host bound is a backstop
  for a child that has stopped answering *at all*. Not manifest-driven yet.
- **Child `worker_threads`** → **4** (was 2). The suspicion that 2 was a third ceiling was
  **measured and disproven** (see the header). Raised anyway with a number behind it: each of 8
  concurrent handlers does real CPU work (Arrow decode + JSON serialization) on both ends of an
  await-bound wait. Not 8 — the runtime is not the bottleneck and idle threads cost memory.

### Still open (deferred, unchanged)

- **Per-call cancellation** — a caller that goes away still waits for its reply to arrive and be
  discarded. Remains a non-goal.
- **Browser connection limits (Risk 9)** — confirmed real and measured (6-connection cap: 2.60 s vs
  1.81 s unthrottled). A separate ceiling, not addressed by this scope.

## Related

- `native-tier-scope.md` — the native (Tier-2) tier this changes the transport of.
- `native-caller-identity-scope.md` — the per-call `caller` stamping that must survive multiplexing.
- `supervision-reactor-scope.md` — restart/health discipline that interacts with in-flight calls.
- `ext-sdk-scope.md` — the stable native contract the child-side loop change breaks.
- `datasources/federation-pool-cache-scope.md` — removed the per-call connect cost; this removes the
  queue in front of it. The two compose and should be measured together.
- `crates/host/src/native/call.rs:39-42` — the lock held across the round-trip.
- `crates/host/src/native/registry.rs:23` — one `Arc<AsyncMutex<Sidecar>>` per `(ws, ext_id)`.
- `crates/supervisor/src/sidecar.rs:169` — `request`, whose `reply.id != id { continue }` (`:186`)
  must become a demultiplexer.
- `crates/supervisor/src/sidecar.rs:118,153` — `restart`/`rearm` resetting `next_id = 0`: the
  id-collision hazard under a pending map.
- `crates/federation/src/main.rs:73` — the child loop that awaits each spawned call before reading
  the next (the panic fence that is not concurrency).
- `crates/federation/src/main.rs:41` — `worker_threads = 2`, the third ceiling.
- `crates/federation/src/pool.rs:43` — `MAX_ENTRIES = 16`: distinct warm **sources**, NOT a
  concurrency cap. Do not size the in-flight semaphore from it.
- README `§3` (rules 1, 4, 5, 6), `§6.5`.
