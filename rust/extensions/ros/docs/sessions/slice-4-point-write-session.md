# ROS driver — Slice 4: point.write (setpoint → outbox), and the sidecar-drivable relay

Status: **done** — `cargo build --workspace`, `cargo fmt --check`, `cargo test -p ros-sidecar`, and
`cargo test -p lb-host --test outbox_relay_ops_test` all green. (Pre-existing `proof_panel`/`workflow`
host tests fail in this checkout for an unrelated reason — they need a `proof_panel_ext.wasm` artifact
that was never built here; my changes are pure Rust dispatch + re-export and do not touch that path.)

## Result

- **Part A (host, core change): 5 tests green** — `outbox.due`/`mark_delivered`/`mark_failed` with
  cap-deny, workspace-isolation, target-filter, delivery-lifecycle, and retry/backoff.
- **Part B (ROS): 4 integration tests green** — `point.write` stages an effect (no inline REST write) →
  relay delivers it to the fake box → marked delivered (not re-sent); a down box retries then delivers
  on recovery; cap-deny refuses before enqueue; ws-B's relay sees none of ws-A's effects.
- Full ros-sidecar suite (28 tests: 17 unit + 3 crud + 4 poller + 4 point_write) still green.

## Files

- Host: `crates/host/src/outbox/relay_ops.rs` (new), wired in `outbox/mod.rs`, `lib.rs`, and the
  `outbox.*` branch of `tool_call.rs`. Tests: `crates/host/tests/outbox_relay_ops_test.rs`.
- ROS: `handlers/point.rs::write` (+ `write_effect_id`, `ROS_TARGET`, `WRITE_ACTION`);
  `poller/ros_target.rs` (delivery adapter); `poller/relay.rs` (`relay_pass` + `spawn_relay`); armed in
  `main.rs`. Manifest caps added in `extension.toml`. Tests: `tests/point_write_test.rs`.

## No debugging entry needed

Nothing broke that outlived its edit — the only friction was two test-harness details (`call_tool`
takes `&Arc<Node>`; Zenoh needs the multi-thread flavor), fixed in the test file as written, not code
defects.

---

## Design record (as built)

## The load-bearing decision (user steer: "do whatever is best long term")

`point.write` must be **must-deliver** (a dropped setpoint is a physical-world safety bug), so it stages
an **outbox** effect rather than writing the box inline. But delivering that effect needs the `RosApi`
box client, which lives in the **sidecar** — the host relay (`relay_outbox`/`due`/`mark_*`) is
store-side, and before this slice there were only two outbox MCP verbs: `outbox.enqueue` (stage) and
`outbox.status` (read). A sidecar could stage an effect but nothing could ever deliver it.

**Rejected — enqueue-only:** honest but leaves setpoints permanently `Pending` (no consumer). **Rejected
— inline write + outbox fallback:** violates must-deliver-first (a crash between the box ack and our
record is a dropped write) and still has no retry consumer.

**Chosen — build the missing platform primitive.** Add a **sidecar-drivable relay MCP surface** to the
host so ANY native driver (not just ROS) can own a `Target` and run its own relay loop over its own
effects through `call_tool`, symmetric with how `github-workflow` drives `relay_outbox` in-process. This
keeps must-deliver-first intact, makes the outbox a real end-to-end path for the native tier, and reuses
the existing `Effect`/`due`/`mark_*` machinery unchanged. It is a **core-crate change** (three new host
verbs + caps), scoped and tested here, but it is the correct long-term shape — the relay seam was always
meant to be target-provided (`workflow/target.rs`: "new targets extension-provided without touching the
relay").

## Part A — host: the sidecar-drivable relay verbs (core change)

Three new MCP verbs in `host/src/outbox/`, dispatched from `tool_call.rs`'s `outbox.*` branch, each
gated `mcp:<verb>:call` (workspace-first §7), each re-running the gate inside the verb (defense in depth):

- **`outbox.due {target?, now}`** — the schedulable-and-past-backoff effects for this workspace, wrapping
  `lb_outbox::due`. Optional `target` filter so a ROS relay pulls only `ros`-targeted effects (a native
  driver never sees another target's effects — and cannot, ws-scoped). Returns `{effects:[Effect…]}`.
- **`outbox.mark_delivered {id}`** — wraps `lb_outbox::mark_delivered` (ack → terminal Delivered).
- **`outbox.mark_failed {id, now}`** — wraps `lb_outbox::mark_failed` (attempt++, backoff or dead-letter);
  returns the resulting `{status}` so the relay can tally without a re-read.

New caps (declared where roles/extensions request them): `mcp:outbox.due:call`,
`mcp:outbox.mark_delivered:call`, `mcp:outbox.mark_failed:call`. These are relay-operator caps — only a
target-service/driver holding them can drive delivery; a normal caller still only has enqueue/status.

**Why a target filter (not per-target namespaces):** effects already carry `target`; filtering `due` by
it is a pure predicate over the existing set, no schema change, and keeps one outbox table. The ws wall
still applies first — the filter narrows *within* the workspace.

## Part B — ROS: point.write + RosTarget + the sidecar relay loop

- **`handlers/point.rs::write {ros_uuid, point_uuid, slot, value|null}`** — cap-check
  `mcp:point.write:call` first; validate slot ∈ 1..=16; stage an outbox effect via `outbox.enqueue`
  with `target:"ros"`, `action:"point.write"`, a payload of `{ros_uuid, point_uuid, slot, value}`, and a
  **stable idempotency id** `ros/{ros_uuid}/{point_uuid}/{slot}` so re-writing the same slot upserts the
  same effect (idempotent at the priority slot — the ROS priority-array model already is). No REST call
  leaves the node here (the deny test asserts this).
- **`poller/ros_target.rs`** — the `Target`-shaped delivery adapter for ROS effects: given an effect,
  resolve its connection's `RosApi` and call `write_point_slot(point_uuid, slot, value)`. `Ok` → the
  box acked; `Err(Unreachable)` → transient, the relay retries. Idempotent on the slot (re-delivery is a
  no-op on the box). It does NOT implement the host `Target` trait (that's store-side); it is the
  sidecar's own delivery seam, driven by the relay loop below.
- **`poller/relay.rs`** — the sidecar relay loop: periodically `outbox.due {target:"ros"}` via the
  callback, deliver each through `RosTarget`, then `outbox.mark_delivered`/`mark_failed`. One loop per
  sidecar (all connections), armed at sidecar start (like the poll registry). Stateless: a respawn
  re-reads the durable `due` set (nothing lost).

## Tests (planned)

Host verbs (`host/tests/`): cap-deny (no `mcp:outbox.due:call` → refused), ws-isolation (ws-B's relay
sees none of ws-A's effects), delivery lifecycle (enqueue → due → mark_delivered → not due), retry
(mark_failed → not due until backoff → due again), target filter (a `ros` relay's `due` excludes a
`github` effect).

ROS (`tests/point_write_test.rs`, real gateway + outbox): `point.write` stages an effect (asserted via
`outbox.status`) and NO REST write leaves the node; the sidecar relay delivers it and `RosFake.writes()`
records the PATCH; a box-unreachable case retries (stays schedulable) rather than dropping; cap-deny
(no `mcp:point.write:call`) refuses before enqueue; ws-isolation (ws-A's setpoint never reaches ws-B).
