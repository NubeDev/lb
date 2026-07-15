# Session — a per-series sample cap, a GC driver, and a safe default

Status: **code complete, TESTING INCOMPLETE — not shipped, do not merge without finishing §Testing.**
Scope: [`scope/ingest/series-sample-cap-scope.md`](../../scope/ingest/series-sample-cap-scope.md).
Issue: [#65](https://github.com/NubeDev/lb/issues/65).

## The ask

> A series grows until the disc is full. Nothing bounds the **committed** series plane on any axis.

Three compounding gaps, per the scope: (1) no **count** bound — time doesn't bound bytes, **rate**
does; (2) **`run_gc` has no driver** — called only by tests and the on-demand verb, so shipped
retention evicts *nothing* at boot; (3) the default is **keep-forever**. Any one alone fills a disc.

## What was built

| Part | Change |
|---|---|
| **1. The count bound** | `max_samples: u64` on `Policy` (serde-default `0` = unbounded, so every existing policy row keeps its exact meaning). New `lb-ingest/src/cap.rs`: `cap_series` (FIFO evict-oldest to the bound), `sample_count`, `over_cap_warning`. Batched at `CAP_EVICT_BATCH = 5_000` so a series far over its cap converges over passes instead of one store-stalling transaction. |
| **2. The GC step** | `run_gc` gains a cap pass per series, reported as `capped_raw` on `GcPass`. Rolls the over-cap window into the policy's tiers **before** evicting (reusing the shipped rollup path), so coarse history survives. Time and count are independent bounds: a sample dies if it violates **either**. |
| **3. The driver** | New `host/src/ingest/retention_reactor.rs` → `spawn_retention_reactors`, modelled on `drain_reactor.rs` (`MissedTickBehavior::Skip`, ws-scoped, errors logged-not-fatal). Wired into `node/src/reactors.rs` at `RETENTION_PERIOD` (300s). **This is what makes retention real.** |
| **4. Release-1 default** | `DEFAULT_MAX_SAMPLES = 100_000` is **advisory only** — an unpoliced series past it produces a warning (surfaced on `GcPass.warnings` and logged by the reactor); nothing is evicted without an explicit policy. Release 2 flips the default. |
| **5. Latent bug fixed** | **Longest-prefix-wins.** Today's `run_gc` iterated every policy, so a series matching both `fleet.` and `fleet.eu.` was processed twice with the tighter bound winning *by accident*. Harmless-ish for a time horizon; destructive for a count cap. Now each series is governed by exactly one policy — its longest matching prefix. |

### Design decisions worth keeping

- **Order by `ts`, never `seq`** — the load-bearing detail. `seq` is monotonic per `(series,
  producer)` only; ordering a multi-producer series by it evicts *live* rows and keeps *dead* ones.
  That is exactly [#63](../../debugging/ingest/latest-pinned-to-pre-restart-sample.md). `seq` is used
  only as a tiebreak *within* an equal `ts`.
- **Ties on the cutoff `ts` are not split.** If every over-cap row shares the cutoff timestamp, the
  cap bails rather than pick arbitrarily — bounded overshoot beats evicting a row we can't prove is
  older, and beats an infinite loop.
- **Warnings are returned as data, not logged in `lb-ingest`.** That crate is deliberately
  dependency-light (no `tracing`); the reactor and the verb own the output channel. `GcPass.warnings`
  also means an operator sees them via `series.retention.gc` without reading node logs.
- **Release 1 only.** The cap + reactor ship; the default stays unbounded with a warning. 100k is
  ~1.2 days at 1 sample/sec — flipping it silently would evict real history on the next boot of a
  node whose operator never read the release note. **Release 2 (flipping the default) is part of this
  slice's definition of done, not a follow-up** — forgetting it is precisely how gap #3 was born.

## Testing — INCOMPLETE

**What was verified, with output observed:**

- `cargo test -p lb-ingest --test series_plane_test` → **16/16 green** (12 new cap tests + the 4
  pre-existing retention/paging tests still green).
- `cargo test -p lb-host --test series_cap_reactor_test` → **4/4 green**.
- `cargo build -p lb-host -p lb-node` → clean.

**Revert-checked** (per [`verify-in-product-not-suite`]; a green test that passes on broken code
proves nothing) — each fault injected, test observed failing, then restored:

| Fault injected | Result |
|---|---|
| Cap orders by `seq` instead of `ts` (the #63 bug class) | `the_cap_orders_by_ts_never_seq_across_producers` **FAILS** ✓ |
| Reactor spawns but never calls `run_gc` (the missing-driver class) | both reactor tests **FAIL** ✓ |
| `governs()` returns `true` for all (drop longest-prefix-wins) | `the_longest_matching_prefix_governs_a_series` **FAILS** ✓ |
| **Boot wiring deleted from `node/src/reactors.rs`** | **NOTHING FAILS — the node still builds and every test stays green.** ✗ |

That last row is the important one and is **unresolved**: no test covers `node/src/reactors.rs`, so
the one line that makes this feature exist on a real node is exactly as untested as it was for the
drain bug and for retention itself. This is the bug class repeating at the meta level.

### NOT DONE — required before merge

1. **The full workspace sweep never completed.** `cargo test -p lb-ingest -p lb-host` was started
   three times and killed each time by a harness timeout/teardown before finishing; the partial run
   that did report showed no failures, but **no clean full-sweep result exists**. `cargo test
   --workspace` was never run at all. Use `--no-fail-fast` (cargo is fail-fast) and note
   `rules_test` hangs under box load.
2. **The live-node verification was never run.** Given revert-check row 4, this is not optional
   ceremony — it is the only thing that proves the boot wiring works. Plan: boot the node against a
   real on-disk store (`LB_STORE_PATH=...`, per the `dev` target's env), set a small `max_samples`
   via `series.retention.set`, run a real producer past the bound, and watch the series **plateau**
   with nobody calling `series.retention.gc`. Then measure the pass cost against a realistic series
   count before trusting the 300s cadence.
3. **Disc-growth check** — the honest end-to-end from the scope: measure store size, write well past
   the cap, GC, assert the size plateaus rather than climbs. If SurrealKV doesn't return space to the
   OS promptly, **say so** and assert row count instead, documenting the caveat. A cap that bounds
   rows but not bytes is a partial win and must be reported as one.
4. **`cargo fmt`** was invoked but its result was never confirmed (it ran in the killed batch).

## Open questions (carried from the scope)

- **Reactor cadence.** 300s is a guess, not a measurement. The scope explicitly says *measure before
  shipping the cadence* — a `count()` per series behind the store's global session mutex, up to 10k
  series/ws, is not free, and `debugging/agent/dev-node-cpu-job-scan.md` is the precedent for a fast
  tick over a full scan burning a CPU. **Unmeasured.**
- **Release 2** — flip `DEFAULT_MAX_SAMPLES` from advisory to enforced. Part of this slice's
  definition of done.
- **Per-node vs authoritative eviction** for a synced series. Lean: per-node (the disc being
  protected is per-node). Untouched here.
- **Per-workspace byte budget** — deferred, not rejected; needs per-series size accounting.

## Files

- `rust/crates/ingest/src/cap.rs` (new) — the FIFO cap primitive.
- `rust/crates/ingest/src/gc.rs` — cap step, `capped_raw`, `warnings`, longest-prefix-wins.
- `rust/crates/ingest/src/retention.rs` — `max_samples` + the projection fix in `list_policies`.
- `rust/crates/host/src/ingest/retention_reactor.rs` (new) — the driver.
- `rust/node/src/reactors.rs` — boot wiring (**untested — see above**).
- `rust/crates/host/src/system/catalog.rs` — verb descriptions gain the count axis.
- `rust/crates/ingest/tests/series_plane_test.rs` — 12 new cap tests.
- `rust/crates/host/tests/series_cap_reactor_test.rs` (new) — reactor + deny + isolation.

[`verify-in-product-not-suite`]: ../../debugging/README.md
