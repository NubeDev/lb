# A ws-B caller can run an extension only ws-A installed

- Area: extensions
- Status: resolved (a documented property, not a bug — the isolation test now asserts the real wall)
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/extensions/github-bridge-session.md
- Regression test: rust/crates/host/tests/github_bridge_test.rs
  (`ws_b_ingest_lands_in_ws_b_never_ws_a`)

## Symptom

The github-bridge isolation test, written to assert "ws-B cannot reach ws-A's installed tool", failed:

```
ws_b_cannot_see_or_use_ws_a_bridge ... FAILED
assertion failed: matches!(ingest_via_bridge(&node, &b_user, ws_b, …).await, Err(WorkflowError::Denied))
```

ws-B held the `mcp:github-bridge.normalize:call` + `mcp:workflow.ingest_issue:call` grants in *its own*
workspace, the bridge was installed only in ws-A — and yet ws-B's `ingest_via_bridge` **succeeded**.

## Reproduce

1. `install_from_registry("github-bridge", …)` in ws-A only.
2. As a ws-B principal holding the normalize + ingest grants (in ws-B), call `ingest_via_bridge` for ws-B.
3. The normalize tool resolves and runs; the call succeeds (it does not return `Denied`).

## Investigation

`lb_mcp::call` runs two phases: `authorize` then `resolve`. `authorize` (gate) is workspace-first via
`caps::check` — but it checks the caller's grant *in the caller's own workspace*; ws-B's principal does
hold `mcp:github-bridge.normalize:call` in ws-B, so the gate passes. `resolve` then maps the qualified
tool to a target via `registry.get(ext_id)` — and the runtime `Registry` is keyed by **`ext_id` alone**
(`crates/host/src/load.rs`: `node.registry.register(manifest.id, tools, instance)` — no `ws`). So the
loaded wasm instance is **node-global**: any workspace whose caller is granted the call can reach it.

Ruled out: a grant bug (the grant is correctly scoped to ws-B; that's *why* the gate passes). Ruled out:
a cache leak (the registry *cache* and the `Install` record ARE per-ws — `installed(ws_b, …)` is `None`,
which the test still asserts and passes).

## Root cause

The runtime instance registry has been keyed by `ext_id` (not `(ws, ext_id)`) since S1 — by design. An
extension instance is **stateless** (§3.4): it holds no workspace data, so sharing one pure instance
across workspaces is sound. The workspace wall is enforced at the layers that carry state and authority:
- **capability gate** (`caps::check`, workspace-first) — a caller must hold the grant *in its own ws*;
- **the store** — every record the tool's effects touch is `(ws, …)`-namespaced.

So ws-B running the shared *transform* changes nothing in ws-A: `ingest_issue` writes ws-B's inbox. The
test's premise ("ws-B can't run the instance") was the error — the real, stronger property is "ws-B's
*effects* can never touch ws-A, and an un-granted ws-B caller is still denied."

## Fix

Rewrote the isolation test to assert the property that actually holds (and is the meaningful one):
- ws-B has **no `Install` record** (store wall) — unchanged assertion, passes;
- a granted ws-B caller **may** run the node-global stateless instance, but its ingest lands in **ws-B's**
  inbox and **ws-A's is untouched** (the data wall);
- an **un-granted** ws-B caller is still `Denied` (the capability wall still bites).

No production code changed — the behavior is correct; only the test's expectation was wrong.

## Verification

`github_bridge_test::ws_b_ingest_lands_in_ws_b_never_ws_a` passes: `a_items.is_empty()` (ws-A untouched),
`b_items.len() == 1` (ws-B's own write), and the no-grant ws-B caller is `Denied`. All 7 github-bridge
tests green.

## Prevention

Documented here so the node-global instance is never re-investigated as a leak. The guardrail for any
future *stateful* tier (it does not apply to wasm, which is stateless): if an extension ever holds
per-ws runtime state, the registry key must become `(ws, ext_id)` — but for stateless wasm, sharing is
correct and the wall lives one layer down (caps + store). A follow-up worth noting: if per-ws *instance
accounting* is ever wanted (e.g. resource limits per workspace), that is a deliberate registry-keying
change, not a bug fix.
