# Store scope — the global session mutex is a node-wide write ceiling

Status: scope (the ask) — **a tracking scope, not a green light**. Promotes to
`doc-site/content/public/` only if a slice ships.

Every namespace-scoped store operation on a node takes ONE global mutex, held across the whole
query. So the store has **no write parallelism at all**: measured on a real embedded store, 18
concurrent writers each targeting their **own workspace** took 7.0ms wall — exactly 18 × the 0.4ms
a single writer takes. Perfect linear serialization, zero concurrency gain, node-wide, across
workspaces. This is a **known, deliberate** trade (it is what makes the workspace wall hold), and it
is currently **cheap enough to be invisible** — sub-millisecond per op. It is scoped here because it
is the platform's next structural scaling ceiling once per-op cost or concurrency rises, and because
it deserves a decision made on purpose rather than discovered under load.

**Do not "quick fix" this.** Removing the mutex without replacing what it guarantees reintroduces a
workspace-wall violation — a cross-tenant data leak. This scope exists to hold the problem, not to
rush it.

## The measurement

Probe: N tasks, each writing one record to its **own** workspace, on a multi-thread runtime against
a real `Store` (no mocks). Own-workspace means nothing but the session mutex can serialize them.

| Writers (each own ws) | Wall | Per writer | Perfect parallelism would be |
|---|---|---|---|
| 1 (sequential baseline) | 5.9ms / 18 ops | 0.3ms | — |
| 2 | 0.8ms | 0.4ms | ~0.3ms |
| 4 | 1.5ms | 0.4ms | ~0.3ms |
| 8 | 3.0ms | 0.4ms | ~0.3ms |
| 18 | 7.0ms | 0.4ms | ~0.3ms |

Wall-clock scales **1:1 with writer count**. Adding cores changes nothing. This independently
reproduces the ~18× figure reported from a live node (~96ms/writer, 1.7s wall at 18 writers — same
shape, a larger constant on a big on-disk store).

## Why it exists (read this before touching anything)

`store/src/open.rs` — every `Store` clone shares ONE embedded SurrealDB connection, and that
connection carries a **single mutable session** (its selected namespace + database). Selecting a
workspace's namespace (`use_ns(ws)`) is a **global mutation** of that shared session, and it is a
distinct `await` from the query it is meant to scope. Without the mutex, two operations for
different workspaces interleave —

    task A: use_ns(A) … [yield] … task B: use_ns(B) … task A: SELECT  ← runs against B's namespace

— and A's query silently reads/writes **B's tenant data**. It is not theoretical: it shipped, and
surfaced as a flaky login (`not a member of any workspace` — a membership written into one namespace
and read back from another). See `debugging/store/concurrent-use-ns-namespace-race.md`; the
regression is `crates/store/tests/concurrent_ns_test.rs` (64 workspaces writing concurrently).

So the mutex makes `use_ns` + query **one critical section**. It buys the hard wall (rule 6) with
throughput. That trade was correct and remains correct until something replaces the guarantee.

## Goals

- **Keep the workspace wall airtight.** Non-negotiable. Any candidate must pass
  `concurrent_ns_test` and the isolation suites unmodified — and if a candidate makes the wall
  depend on reviewer discipline rather than structure, it is the wrong candidate.
- **Remove the node-wide serialization point**, so concurrent work in *different* workspaces
  proceeds in parallel and per-op latency stops summing.
- **Decide on evidence.** Ship only if a measurement shows the ceiling actually binds a real
  workload. Today it does not (0.4ms/op — 18 writers cost 7ms).

## Non-goals

- **Per-workspace parallelism within one workspace.** Out of scope; the wall is per-workspace, so
  cross-workspace parallelism is the whole prize.
- **Changing the datastore.** SurrealDB only (rule #2).
- **Anything in the drain-backpressure slice.** That fix (bounded caller drains + the ingest
  reactor) is independent and already shipped; it did **not** need this. See below.

## Why this is NOT the ingest bug (recorded, so it is not re-conflated)

The `ingest.write` stall (18.5s for one sample behind a 4,671-row backlog) was **caused by an
unbounded drain on the caller's path**, not by this mutex — the caller committed the whole
workspace backlog inside its own call. Bounding the drain fixed it end-to-end (measured 900.7ms →
66.0ms at that backlog on a real on-disk store) with this mutex fully in place. At 18 writers the
mutex costs single-digit milliseconds; it cannot produce an 18-second call. Do not cite this scope
as the cause of that class of bug. See `scope/ingest/drain-backpressure-scope.md`.

## Intent / approach — candidates, none yet chosen

**The requirement any candidate must meet:** a query must be *structurally unable* to run against a
namespace it did not select. The current design achieves that by serializing; the alternatives
achieve it by removing the shared mutable session.

1. **A connection per workspace (pool).** Each workspace gets its own `Surreal<Db>` handle with its
   namespace already selected, so there is no shared session to race and no lock. Cross-workspace
   work is genuinely parallel. Cost: N connections against one embedded engine — memory, file
   handles, and an eviction policy for idle workspaces; needs a spike to learn whether the embedded
   engine even supports this cheaply. **This is the leading candidate** — it deletes the shared
   mutable state rather than guarding it.
2. **Namespace-qualified statements (no session at all).** If every statement can name its
   namespace inline, `use_ns` disappears and with it the race. Needs a spike: does the embedded
   engine support fully-qualified addressing for everything we issue (incl. DDL, transactions,
   `type::thing`)? If yes this is the cleanest; if it is partial, a mixed model would leave exactly
   the kind of discipline-dependent wall this scope refuses.
3. **A per-workspace mutex instead of a global one.** Small, obvious, and only correct if the
   underlying session is per-workspace too — which it is not. Against one shared session this is
   **wrong** (it permits the exact interleave that leaked). Recorded to be explicitly rejected, so
   nobody re-proposes it.
4. **Do nothing (the current default, and honest).** 0.4ms/op means the ceiling binds only at
   thousands of concurrent ops/sec/node. Revisit when a real workload measures it.

**Recommendation: stay on (4) until a measurement says otherwise; spike (1) and (2) before writing
any code.** The spike is the deliverable — not a refactor.

## How it fits the core

- **Tenancy / isolation:** this IS the tenancy scope. The mutex is currently the enforcement
  mechanism for the hard wall at the store layer.
- **Symmetric nodes:** any change is engine/config-level, identical on edge and cloud — no
  `if cloud`.
- **One datastore:** unchanged, SurrealDB only.
- **Capabilities / MCP / bus / secrets:** N/A — this is below all of them.

## Testing plan

- **Workspace isolation (mandatory, and the gate).** `crates/store/tests/concurrent_ns_test.rs`
  must pass unmodified — 64 workspaces writing concurrently, each reading back only its own. Any
  candidate that needs this test *changed* has changed the guarantee.
- **Isolation under the new concurrency.** The current test passes partly *because* everything
  serializes. A candidate that unlocks parallelism must be re-tested at higher concurrency, on a
  multi-thread runtime, repeated (the original bug was **intermittent** — a single green run proves
  nothing; run it 20×+ and report the distribution, not a best case).
- **The measurement, repeated.** Re-run the probe above and show the scaling curve flattening.
  Report flaky as flaky.
- **The full host isolation suites**, unmodified.

## Risks & hard problems

- **The failure mode is a silent cross-tenant leak**, not a crash. A wrong fix here doesn't fail
  loudly — it serves A's data to B under load and passes tests on an idle box. This is the highest
  blast-radius change in the store.
- **It was already found the hard way, by flakiness.** The original race surfaced as an
  intermittent login failure, not as an isolation test failure. Assume a repeat would be equally
  indirect.
- **A connection pool may not be cheap** on an embedded engine, and idle-eviction adds a lifecycle
  where there is none today.
- **Temptation to fix it while fixing something else.** It looks adjacent to every store perf
  issue. It was adjacent to the ingest bug and was not the cause. Resist.

## Open questions

- Does the embedded SurrealDB engine support **multiple connections** to one on-disk store cheaply
  (candidate 1)? Spike it.
- Can **every** statement we issue be namespace-qualified inline (candidate 2), including DDL and
  transactions?
- What is the **real** concurrency of a busy node today — do we ever approach the point where
  0.4ms/op × N binds? Instrument before optimizing.
- Does the persistent (SurrealKv) engine behave differently from `mem://` here? The probe above is
  in-memory; the live 18× report was on-disk with a much larger constant.

## Related

- `debugging/store/concurrent-use-ns-namespace-race.md` — the bug the mutex fixed. **Read first.**
- `crates/store/tests/concurrent_ns_test.rs` — the regression that must never be weakened.
- `scope/store/persistent-backend-scope.md` — the engine slice.
- `scope/ingest/drain-backpressure-scope.md` — the bug this is NOT.
- `scope/tenancy/` — the workspace wall this defends.
- README **§3** (rule 6: workspace is the hard wall).

## Skill doc

**N/A** — no drivable surface. This is an internal store property with no MCP verb, route, or wire
shape.
