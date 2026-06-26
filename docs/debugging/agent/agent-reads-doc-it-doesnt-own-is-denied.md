# The agent is Denied reading a substrate doc the caller owns

- Area: agent
- Status: resolved
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/agent/ai-core-session.md
- Regression test: rust/crates/host/tests/agent_test.rs
  (`an_edge_user_invokes_the_agent_which_calls_the_gateway_and_a_granted_tool`)

## Symptom

The S5 local exit-gate test failed: invoking the agent with a shared **doc** as substrate returned
`AgentError::Denied`, even though the caller (`user:ada`) owned the doc and held `store:doc/*:read`,
and the agent's own caps also held `store:doc/*:read`.

```
agent runs to completion: Denied   (at agent_test.rs, the happy-path invoke)
```

The invoke-deny and skill-grant-deny tests passed — only the path that actually *reads* a doc the
caller owns failed.

## Reproduce

1. `put_doc` a doc owned by `user:ada` in ws A.
2. Invoke the agent as `user:ada` (with `mcp:agent.invoke:call`, `store:doc/*:read`) passing
   `doc: Some("spec")` as substrate, agent caps also including `store:doc/*:read`.
3. The agent's substrate read returns `Denied`.

## Investigation

The capability gate (gate 2) clearly passed — both the caller and the agent held `store:doc/*:read`,
so the intersection did too. That left gate 3, the S4 **membership** gate (`may_read_doc`): owner /
shared-team-member / linked-channel-grantee. Reading `visibility.rs`, the owner path is
`principal.sub() == doc.owner`. The doc's owner is `user:ada` (set from the caller at `put_doc`).
But the agent was reading under its **derived principal**, whose sub is `agent:session` — so
`agent:session != user:ada`, no share, no link → `Denied`. The capability intersection was right;
the *identity* used for the membership check was wrong.

Ruled out: a caps-intersection bug (the deny tests proved the intersection; and gate 2 isn't the
failing gate). Ruled out: a store/namespace issue (the doc was readable directly as `user:ada`).

## Root cause

Substrate reads (docs, skills) are **membership/ownership/grant-gated** (gate 3), and the agent was
reading them under a *distinct* `agent:session` identity — so it owned nothing and was a member of
nothing. The agent should read substrate **on the caller's behalf**: as the caller's identity (so
gate 3 resolves), bounded by the intersected caps (so gate 2 can't widen). Conflating "who the agent
*is* for audit" (`agent:session`) with "whose access it reads under" (the caller) was the error.

## Fix

`crates/host/src/agent/substrate.rs`: build the substrate-read principal as
`caller.derive(caller.sub(), agent_caps)` — the **caller's sub** (gate 3 resolves as the caller)
with the **intersected caps** (gate 2 stays bounded; the agent still can't read a doc its own grant
excludes). Tool calls in the loop keep the `agent:session` sub, because they are ws + capability
gated only (no membership), so a distinct audit identity there is correct. The split is now explicit
in `substrate.rs` and `run.rs` doc comments.

## Verification

`agent_test::an_edge_user_invokes_the_agent_which_calls_the_gateway_and_a_granted_tool` now passes
(the doc + skill substrate read succeeds and the loop completes), and
`agent_isolation_test::an_agent_in_ws_b_cannot_read_ws_a_substrate_doc` confirms the
caller's-behalf read still cannot cross the workspace wall (gate 1 fires first). All 11 agent tests
green.

## Prevention

The regression is the happy-path test itself (it fails before the fix, passes after). The guardrail:
`derive` can only ever NARROW (caps = intersection, ws inherited), so reading "as the caller" can
never grant the agent *more* than the caller — the on-behalf-of identity is safe precisely because
the capability bound is unchanged. A follow-up (agent scope open question) is a "shared with me"
doc listing so the agent's substrate is discoverable, not just addressable.
