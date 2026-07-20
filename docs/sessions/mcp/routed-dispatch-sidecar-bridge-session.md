# Session — routed dispatch over the sidecar/HTTP bridge

**Date:** 2026-07-20
**Scope:** [`../../scope/mcp/routed-dispatch-sidecar-bridge-scope.md`](../../scope/mcp/routed-dispatch-sidecar-bridge-scope.md)
**Builds on:** [routed-node-dispatch](routed-node-dispatch-session.md) (#81, BUILT 2026-07-20)
**Downstream consumer:** ems gateways slice 2 (blocked on this seam)

## What shipped

Routed dispatch (#81) shipped the *engine* but stopped at the library boundary: `lb_mcp::call_on_node`
had **zero non-test callers**, so an HTTP caller could be told a call was `Ambiguous` (409) but had no
way to answer. This session threads the target-node axis through the two in-repo seams between a
sidecar and that engine.

- **`lb_host::call_tool_on_node(node, principal, ws, tool, input, Option<&NodeId>)`** — the additive
  targeted entry (`crates/host/src/tool_call.rs`). Threads `target_node` through
  `call_tool_at_depth_on_node` → `dispatch_at_depth`, selecting `lb_mcp::call_on_node` vs
  `call_with_ctx` at the bottom. `call_tool` is now a thin `None` delegate, so its wide fan-out
  (agent loop, gateway routes, reach path, `viz`, `nav`, `callback`) is **untouched**.
- **`POST /mcp/call` gains an optional `node`** (`role/gateway/src/routes/mcp.rs`). `McpCall` gets
  `#[serde(default)] node: Option<String>`; a malformed id is `400 BadInput` before dispatch.
- **`lb_host` re-exports `NodeId`/`NodeIdError`** so the bridge can name a node without taking its own
  `lb-bus` dependency — the node id is part of this crate's public signature.

**Not in this repo:** seam 1, `SidecarClient::call_tool_on_node`, lives in **`NubeDev/lb-ext-sdk`**
(`crates/lb-sidecar-client/src/client.rs`), pinned here at `sdk-v0.3.0`. It is a separate PR + a new
`sdk-v*` tag. **ems must bump both pins together** — see "Version skew" below.

## Resolved open questions

- **OQ1 — additive vs `Option` param.** **Additive**, as the scope recommended, and the fan-out
  confirmed it: `call_tool` has ~40 call sites and `call_tool_at_depth` has 4 more in `viz`/`nav`/
  `callback`. Threading a required param through all of them would have maximised the surface for the
  exact bug this scope fears — a `Some(node)` silently degrading to `None`.
- **OQ2 — is the signature agent-ready?** Yes. `call_tool_on_node` is the same shape the agent loop
  would adopt (`Option<&NodeId>` as a trailing param); wiring it there needs no further signature
  change. Not wired here, per scope.
- **OQ3 — `Ambiguous` candidates parseable over the bridge.** Verified already true:
  `ToolError::Ambiguous { ext, candidates }` carries the node ids as structured data, and the
  untargeted-still-ambiguous test asserts both hosts appear in `candidates`. No change needed.

## The finding this session added to the scope

**A targeted call to a host-native verb silently ran locally.** The scope threaded the axis but did
not say what `node` means for `store.*` / `series.*` / `undo` / `telemetry.*` / `tools.*` — verbs that
run against **this** node's store. `is_host_native` returns from `dispatch_at_depth` ~250 lines
*before* the routed check, so `node` was parsed, carried, and then **ignored**: `store.write` targeted
at `gw-01` executed locally and returned a plain `200`, indistinguishable from success.

That is the scope's own "silent-fallback regression" risk in its worst form — worse than the
misprovisioning bug, because there is no second node involved to even race.

**Decision (author): refuse it.** `Some(node)` + a host-native verb → `ToolError::BadInput` (→ `400`),
naming the verb and the target. Rejected alternatives: *routing them* (genuinely useful for remote
store/series reads, but it makes every host-native verb a remote surface with its own serving-side
dispatch — a separate scope), and *documenting the current behaviour* (leaves the silent-fallback bug
in place). The refusal reveals nothing privileged: which verbs are host-native is a static build fact,
identical for every caller.

## Testing

Per `scope/testing/testing-scope.md`. Real nodes, real Zenoh, real store, real capability checks — no
mocks (rule 9).

**`crates/host/tests/routed_host_entry_test.rs`** (5 tests) — two real `Node`s on two in-process Zenoh
peers linked over loopback TCP, both hosting one ext, plus a caller edge. Harness shape inherited from
`routed_ambiguity_test.rs`.

| Test | Proves |
|---|---|
| `the_host_entry_lands_every_targeted_call_on_the_node_named` | **40 calls, 100% on the node named, 0 fallback** |
| `an_untargeted_host_call_is_still_ambiguous_and_dispatches_nothing` | `None` unchanged; `candidates` name both hosts |
| `a_capless_targeted_host_call_is_denied_with_no_existence_signal` | **cap deny (mandatory)** — `Denied` for real *and* invented nodes |
| `a_ws_b_caller_cannot_target_a_node_serving_only_ws_a` | **ws isolation (mandatory)** — `NodeUnreachable`, the key space is the wall |
| `an_unknown_target_through_the_host_entry_never_falls_back` | absent node → refusal, never another host |

**`role/gateway/tests/mcp_call_target_node_test.rs`** (5 tests) — real gateway, real router, real token
verification: bad node id → `400` (not `403`); no `node` → unchanged `200`; well-formed `node` reaches
the routed path (not ignored, not rejected); targeted host-native → `400`; capless targeted → `403`
identically for plausible and invented nodes.

### The determinism guard is mutation-checked

A test that cannot fail proves nothing, and "a targeted call succeeded" would pass on degraded code.
So the thread was **deliberately broken** — one line, `target_node` → `None` in `call_tool`'s hop to
`call_tool_at_depth_on_node`, the exact hazard the scope names — and the suite re-run:

```
test result: FAILED. 2 passed; 3 failed
  the_host_entry_lands_every_targeted_call_on_the_node_named ... FAILED
    last error: Ambiguous { ext: "fleet-hostdet", candidates: [...gw-01, ...gw-02] }
  a_ws_b_caller_cannot_target_a_node_serving_only_ws_a ... FAILED
  an_unknown_target_through_the_host_entry_never_falls_back ... FAILED
```

`Ambiguous` is precisely the silent-degradation signature. The mutation was then reverted and the
suite re-confirmed green (5/5).

**Worth recording:** the *first* mutation attempt silently failed to apply (an indentation mismatch in
the patch), and the suite passed — which momentarily looked like "the tests don't catch it." Verifying
the mutation actually landed (`diff` against a backup) is part of the technique, not an optional step.

- **Hot-reload:** N/A — no durable instance state introduced.

### Two pre-existing failures, both confirmed NOT caused by this change

Neither is a regression; both are recorded so the next session does not re-derive the triage.

1. **`routed_ambiguity_test::a_live_node_that_drops…`** fails when its 12-test file runs together
   (11 pass / 1 fail) and passes alone every time. Because this session changes the dispatch path
   that suite exercises, it had to be ruled out properly: the same suite run in a **detached worktree
   at `36ae877d`** (the base commit, none of these changes present) failed **identically**, same
   11/1 split. Triaged in
   [`debugging/bus/drop-test-flakes-under-suite-parallelism.md`](../../debugging/bus/drop-test-flakes-under-suite-parallelism.md).
2. **`cargo test --workspace` does not compile** — `role/cli/tests/ext_publish_test.rs` `include_bytes!`s
   `extensions/hello-v2/target/…/hello_v2_ext.wasm`, a build artifact that is not present. Unrelated to
   this change (a missing wasm build, in a crate this session does not touch). The per-crate suites
   (`-p lb-host`, `-p lb-role-gateway`) build and pass.

**Two method notes worth keeping**, both of which nearly produced a false "all clear":
- The first clean-tree comparison used `git stash`; popping the stash **killed the backgrounded test
  run**, leaving empty output that could have been read as "no failure on a clean tree." A detached
  worktree is the right tool — it cannot be disturbed by the working tree.
- A `cargo test … | grep …` pipeline reports the **grep's** exit status. An "exit code 0" notification
  on such a command says nothing about whether the tests passed; the captured output must be read.

## Risks carried forward

- **Version skew (the sharp one).** A sidecar sending `node` to an **old** host: the old bridge has no
  `node` field, serde ignores the unknown key, and the call runs **untargeted — i.e. silently local**.
  That is the failure this scope exists to prevent, reachable through a *partial* bump. ems must move
  the `sdk-v*` **and** `node-v*` pins together; a partial bump must not be mistaken for routing.
- **Discovery is still owed.** This lets a caller name a node it already knows. Learning the fleet
  (and the `targeted_dispatch` flag that gates `NodeTooOld`) remains
  [`node-roles/fleet-presence-scope.md`](../../scope/node-roles/fleet-presence-scope.md), Findings A/B.
- **The WASM guest `host.call-tool` import** still gets no routed target, per scope non-goals — a local
  guest's callback identity is node-local by construction.

## Follow-ups

1. **`lb-ext-sdk` PR:** `SidecarClient::call_tool_on_node(tool, input, &NodeId)` — same POST, same
   token, `node` added to the body; `call_tool` delegates with `None` (byte-for-byte the old body).
   Ship as a new `sdk-v*` tag.
2. **Tag `node-v*`** with the bridge + host change, and land both pins in ems together.
3. Promote to `doc-site/content/public/mcp/routed-node-dispatch.md` (extending it) once the SDK half
   ships — the public page's "What is not here yet" names this gap.
