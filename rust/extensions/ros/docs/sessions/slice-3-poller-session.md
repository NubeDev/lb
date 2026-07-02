# ROS driver — Slice 3: the reusable poller

Status: **done** — `cargo build --workspace`, `cargo test -p ros-sidecar`, `cargo fmt --check` all
green. The mandatory poller-unit (stub Source), enable-gating, capability-deny, and workspace-isolation
tests pass against a **real** spawned gateway + store + ingest path (only the ROS box faked behind
`RosApi`).

## What shipped

A **driver-agnostic poll engine** under `src/poller/`, plus the one ROS adapter and the runnable verbs
that arm it. Everything except `ros_source.rs` is reusable by a future driver — no ROS vocabulary
reaches the loop, gating, backoff, or batching.

- **`poller/source.rs`** — the reusable `Source` read seam. `PollTarget` carries a leaf's id, its
  fully-qualified `series` (source-owned), and the four enable flags (connection/network/device/point).
  `Reading` is one tick's value. `SourceError` splits `Unreachable` (tick-level backoff) from
  `NotFound`/`Other` (per-target drop). No ROS here.
- **`poller/gating.rs`** — the pure enable-AND rule: a leaf polls iff all four levels are enabled.
  `resolve` keeps the enabled subset in order. Unit-tested for exactness (each level silences
  independently; a network-off drops a whole branch).
- **`poller/sink.rs`** — the `Sink` write seam. `IngestSink` batches a tick's `SampleOut`s into ONE
  `ingest.write` MCP callback (`HostCtx::client()`), never a per-sample write (poll-storm mitigation).
  The `seq` (ingest dedup key half) is engine-owned; the producer is host-forced (un-spoofable).
- **`poller/poller.rs`** — the tick core `poll_once` (pure w.r.t. time: takes the tick `ts` + a
  `SeqState`) and the `Backoff` calculator (base interval on success, exponential on failure, capped).
  Both fully unit-testable with a stub Source + recording Sink — **no box, no gateway, no sleeping.**
- **`poller/run.rs`** — the async poll task (`spawn_poll`: the only time-aware part — spawn/tick/sleep/
  repeat), `PollStatus`, `PollTask` (abort on stop), and `PollRegistry` (`ros_uuid → task`, process-
  lived, `start` idempotent). Drives `poll_once` + `Backoff`; thin glue over the tested core.
- **`poller/ros_source.rs`** — the **ONE** ROS-specific file: adapts `RosApi` into `Source`. Walks the
  box tree via `list_networks(with_tree=true)` (one REST round-trip/tick), flattens to `PollTarget`s
  with the four flags, mints the series id `ros.{ws}.{ros}.{net}.{dev}.{point}`, and reads
  `present_value` per tick. The connection-level enable comes from the shadow (the box doesn't carry
  it), so `ros.update {enable:false}` on the connection silences the whole box next tick.
- **`handlers/poll.rs`** — `ros.start|stop|status|restart` (runnable-trait grammar). Each self-checks
  its `mcp:ros.<verb>:call` cap first (the inbound `native.call` carries no identity — see slice-2
  finding #1). `start` resolves the connection → `RosSource` + `IngestSink` → `spawn_poll` into the
  registry; cadence seeded from the shadow's `poll_rate` (default 5s), backoff cap = 12×.
- **Wiring:** `PollRegistry` threaded through `main.rs` → `call::handle` → `handlers::dispatch`
  (process-lived, one per sidecar). `extension.toml` gains `mcp:ros.{start,stop,status,restart}:call`
  and the `ros.restart` tool descriptor.

## Load-bearing decisions (honoring slice-2 findings)

1. **The seam split is `Poller<Source>` + `Sink`, not a ROS-aware loop.** The scope's central idea: the
   loop/gating/backoff/batching are proven with a `StubSource` (7 unit tests) with zero infrastructure;
   `RosSource` is the only file that knows ROS. Swap it for a `BacnetSource` and the engine is unchanged.
2. **`poll_once` is pure w.r.t. time.** It takes the tick `ts` and the caller-owned `SeqState`, so the
   schedule (`Backoff`) is a separate pure calculator. Result: no wall-clock in the tested core; the
   only sleeping code is the ~40-line `spawn_poll` loop, itself covered by `start_paused` tests.
3. **A partial tick is never committed.** An `Unreachable` mid-walk abandons the whole tick and backs
   off — a half-written cycle would look like real gaps in the series. A per-target `NotFound` only
   drops that leaf. (Tested both ways.)
4. **All-gated-off is a *success*, not a failure.** We polled correctly and there was nothing to write
   → empty batch, no `ingest.write`, no backoff.
5. **Poll values ride `ingest.write` (motion), reached ONLY via the `lb-sidecar-client` callback**
   (`HostCtx::client()`) — honoring slice-2 finding #2 (the poller reaches the host only through the
   callback). One batched call per tick.
6. **Workspace isolation stays structural** (slice-2 finding #1): the sidecar's ws-scoped token walls
   every `ingest.write`; the series-id ws prefix is defense-in-depth. The isolation test confirms ws-B
   cannot read a series ws-A's poller wrote.
7. **`ingest`/`series` gate on `mcp:<verb>:call` only** (no separate `series:` resource cap — verified
   in `host/src/ingest/authorize.rs`). So the manifest needs only the `mcp:series.*:call` grants; the
   cap-deny test removes `mcp:series.latest/read:call` and confirms the reader is refused.

## Tests (green)

Unit (`--lib`, 17 total incl. slice-2 paging):
- `poller::gating::tests` — all-enabled polls; each level off silences independently; resolve keeps
  only fully-enabled in order; network-off drops a whole branch.
- `poller::poller::tests` — one batch per tick of only enabled targets; each gating level silences via
  the real engine path; seq monotonic per series across ticks; unreachable-targets & unreachable-mid-
  walk fail the tick with no partial write; not-found target dropped, rest survive; all-gated-off is a
  clean empty tick; backoff grows-then-resets and caps at max.
- `poller::run::tests` (`start_paused`) — spawn ticks then stop parks; registry start idempotent + stop
  finds/removes.

Integration (`tests/poller_test.rs`, real gateway):
- `poller_writes_present_value_to_series` — arm `ros.start`, a real tick's `present_value` lands on
  `ros.{ws}.{ros}.{net}.{dev}.{point}` (read back via `series.latest`); `ros.status` counts samples;
  `ros.stop` parks.
- `connection_disable_gates_the_whole_box` — a connection created `enable:false` polls NOTHING;
  `ros.update {enable:true}` + `ros.restart` resumes samples. (Integration proof of enable-gating.)
- `reader_without_series_read_cap_cannot_see_values` — a same-ws reader stripped of
  `mcp:series.latest/read:call` is denied the polled value.
- `workspace_isolation_series_invisible_across_ws` — ws-B cannot read a series ws-A's poller wrote.

## Deviations / notes

- **One file per resource-group, per slice-2 convention** — `handlers/poll.rs` holds `start/stop/
  status/restart` (well under 400 lines), matching slice 2's per-resource grouping rather than one file
  per verb.
- **`tokio` `test-util` added to dev-deps** — `start_paused`/`advance` drive the poll loop
  deterministically (no real-time flakiness) in the `run.rs` unit tests.
- **No debugging entry needed** — nothing broke that outlived the same edit (the one test-authoring
  slip, a `RecordingSink` asserting no-empty-batch, was a test bug fixed in place, not a code defect).

## Next: Slice 4 — `point.write` (setpoint → outbox)

`handlers/point.rs` gains `write {point_uuid, slot, value|null}`: cap-check `mcp:point.write:call`,
then stage a **must-deliver outbox effect** (`outbox.enqueue` via the callback) that PATCHes the box's
priority array — idempotent at the slot; retries until acked (box-unreachable path). The `RosApi`
`write_point_slot` seam and the `RosFake` `writes()` recorder are already in place from slice 1.
