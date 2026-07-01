# Sink `channel`/`inbox` target sent no message on the live canvas — the running node was stale

- **Date:** 2026-07-01
- **Area:** flows (sink node — live canvas vs running binary)
- **Status:** resolved

## Symptom

User on the live canvas: "the sink node doesn't work for me to send a channel message — I add the
channel name." The sink node settled but no item appeared in the target channel.

## Root cause

**Not a code bug — a stale running node.** The sink-channel path had just been reworked as part of the
[flow-message-envelope slice](../../sessions/flows/flow-message-envelope-session.md): the sink now writes
`msg.payload` to `msg.topic ?? config.name`, and a single upstream auto-wires its whole envelope into the
sink's `payload` (`resolve_node_bindings`, D3/D4). That code is green in the test suite
(`flows_sink_test` incl. `sink_destination_uses_msg_topic_over_config_name`).

But the node serving the canvas had been started at **05:04:30** from a binary built at **05:04**, while
the envelope edits to the flows engine landed *after* that — four host files were newer than the running
binary:

```
rust/crates/host/src/flows/run_store.rs   05:06   (resolve_node_bindings auto-wire / carry)
rust/crates/host/src/flows/node_state.rs  05:07
rust/crates/host/src/flows/triggers.rs    05:05
rust/crates/host/src/flows/mod.rs         05:05
```

`run_store.rs` is exactly where the auto-wire that feeds the sink's `payload` lives. So the live canvas
was driving the *old* sink/binding code (the pre-envelope `value` path) while the descriptors/tests were
the new shape — the sink received no `payload` and wrote nothing useful.

This is the same failure class as the note for the original sink shapes
([sink-node-request-shapes-dont-match-target-verbs.md](sink-node-request-shapes-dont-match-target-verbs.md)),
which already warned: *"the fix is in the host binary, so the running dev node must be rebuilt +
restarted for the live canvas to work."*

## Fix

Rebuild + restart the dev node so the canvas runs the current tree:

```
make kill           # frees 8080/5173, stops the stale node + UI
cargo build -p node # green (11.9s) — confirms the tree compiles before restart
make dev            # rebuilds (cached) + relaunches node + UI
```

After restart: gateway serving on `127.0.0.1:8080`, UI on `5173`, and the cron reactor firing
(`flow cron reactor fired ws=acme fired=1`) — the armed flow now runs on the envelope code, so the sink
auto-wires `payload` and records to the channel.

## No new regression test

The sink-channel behaviour is already covered by the envelope slice's real-gateway tests
(`flows_sink_test`, `ProofPanel.gateway.test` `inbox.record → inbox.list` round-trip). The defect here
was operational (stale binary), not a code path. The durable guard is the **mtime check before
declaring the live canvas broken**: if flows `src/*.rs` is newer than `target/debug/node`, restart first.

## Operator note (carry into future flows sessions)

The dev node does **not** hot-reload Rust — only the UI does (Vite). Any change under
`rust/crates/host/src/flows` or `rust/crates/flows` needs `make kill && make dev` before the live canvas
reflects it. A node whose `/proc/<pid>/exe` build time predates the flows source mtimes is stale.
