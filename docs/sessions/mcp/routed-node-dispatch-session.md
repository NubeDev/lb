# Session — routed MCP dispatch to a named node (#81)

**Date:** 2026-07-20
**Scope:** [`../../scope/mcp/routed-node-dispatch-scope.md`](../../scope/mcp/routed-node-dispatch-scope.md)
**Issue:** [lb#81](https://github.com/NubeDev/lb/issues/81)
**Downstream consumer:** ems gateways scope slice 2 (blocked on this seam only)

## What shipped

A routed MCP call can now name a **target node**, and an untargeted call to a
multiply-hosted extension is **refused** instead of silently coin-flipped.

- `NodeId` (`crates/bus/src/node_id.rs`) — a key-expression-safe node identity, validated at
  construction. Minted here, **owned by fleet-presence**.
- `Target::Remote { node, tools }` and a registry that maps one ext id to **N targets** — the
  change that makes a multiply-hosted ext *representable*, and therefore checkable.
- The ambiguity guard in `resolve` (calling side), returning structured
  `ToolError::Ambiguous { ext, candidates }`.
- Node-qualified bus key `mcp/{ext}/{node}/call`, declared **per workspace**; all remote
  dispatch goes here. The legacy shared key survives for mixed-version callers only.
- `lb_bus::query` now **enforces** "exactly one responder" (`BusError::MultipleResponders`)
  instead of asserting it in a comment.
- `lb_mcp::call_on_node(...)` — targeting as an explicit parameter. No new capability grammar.

## Phase 0 — what the entry gate found (and why it changed the plan)

Two findings, both verified rather than assumed, are written into the scope doc.

**Finding A — the routed path has ZERO production wiring.** The scope said remote registration
was hand-wired; it is worse and symmetric. *Neither* half has a non-test caller:

| Seam | Production callers | Test callers |
|---|---|---|
| `register_remote_extension` | **0** | 3 |
| `serve_ext` | **0** | ~12 |

(`grep serve_ext` outside tests hits only `serve_ext_ui`, an unrelated static-file route.)

This **corrected the scope's severity claim**. The "supervisor provisions the wrong physical
box" scenario is **latent, not active** — no production caller can reach a second host today.
The urgency is *ordering*: the guard must land before fleet-presence's discovery arms the
hazard. The scope now says so; claiming a live production bug would have misstated the evidence.

It also **bounds what the hazard proof proves** — two responders require two `serve_ext` calls,
which only a test can make. Recorded plainly rather than presented as a shipped bug.

**Finding B — `NodeId` did not exist, and nothing could stand in.** Verified zero hits for
`NodeId` / `declare_node_presence` / `nodes.list` / `nodes.watch`. Every candidate fails:
`Node.key` is per-boot and *secret*; `gateway_url` is `None` when headless; zenoh's ZID sits
behind the `unstable` feature the workspace declined.

**Decision: mint `NodeId` here, as fleet-presence's primitive.** Deferring entirely was cleaner
on paper but blocks #81 behind an unstarted slice, and ems behind that. The fork both docs warn
about is avoided *structurally*: the type lives in `lb-bus` (below `lb-mcp` and `lb-host`), built
to fleet-presence's stated constraints, so that scope **widens it** rather than minting a second.
`fleet-presence-scope.md` is amended to say exactly this.

All eight open questions are resolved in the scope doc with reasoning. The two that most shaped
the code: **Q6** (per-workspace node-key declaration — a real key-space wall) and **Q3**
(`Target::Remote` singular, because a plural target defeats the guard downstream of resolve).

## The hazard, proven before it was fixed

Against **unmodified** production code, two real nodes both hosting one ext, 40 identical
untargeted calls:

```
=== ROUTED-CALL NONDETERMINISM (pre-#81) ===
40 identical untargeted calls to `fleet.whoami`, two hosts serving:
  node-a: 25
  node-b: 15
  distinct responders observed: 2
The caller cannot tell which physical box ran the tool. Nothing errors.
```

A **25/15 split**, no error. That is the coin flip, measured — not argued. The test needed a
`whoami` fixture because `hello.echo` returns its input verbatim and so cannot identify its
responder; the nodes, bus, queryables and dispatch path are all the production ones.

## Tests — 11, all green, all revert-checked

`crates/host/tests/routed_ambiguity_test.rs`, on two real in-process Zenoh peers linked over
loopback TCP (no mocks, no fake transport):

- `untargeted_call_to_a_multiply_hosted_ext_is_ambiguous` — the headline regression
- `a_targeted_call_lands_on_the_named_node` — 10 rounds per target; one hit could be luck
- `an_unknown_target_is_refused_and_never_falls_back`
- `a_capless_targeted_call_is_denied_indistinguishably_for_real_and_fake_nodes` — **mandatory
  deny**, plus the sharp form: identical error *and* identical rendering, so a capless caller
  cannot enumerate a fleet
- `authorize_precedes_resolve_so_ambiguity_never_leaks_to_the_unauthorized`
- `a_ws_b_caller_cannot_reach_a_node_that_serves_only_ws_a` — **mandatory isolation**, asserted
  in its STRONG form (unreachable, not merely refused), which only the Q6 per-workspace wall
  supports
- `a_reload_keeps_the_node_addressable_and_targeting_recovers` — **mandatory hot-reload**
- `a_single_host_untargeted_call_still_just_works` — the unchanged fast path
- `a_node_targeting_itself_runs_locally_with_no_bus_hop`
- `a_local_host_wins_an_untargeted_call_over_remote_peers`
- `forgetting_a_host_makes_an_ambiguous_ext_callable_again` — a shrinking fleet recovers

**Every gate was proven non-vacuous by planting the defect it exists to catch:**

| Planted defect | Tests that went red |
|---|---|
| Guard auto-picks the first candidate (the pre-#81 bug) | `untargeted…is_ambiguous`, `forgetting_a_host…` |
| `KEY_UNSAFE` emptied (NodeId charset) | 3 of 5 `node_id` unit tests, incl. the wildcard case |
| Q6 reverted to the `ws/*` wildcard | `a_ws_b_caller_cannot_reach…` — **only** that one |
| authorize/resolve order swapped | `a_capless_targeted…`, `authorize_precedes_resolve…` |
| Reload stacks instead of swapping | `a_reload_keeps_the_node_addressable…`, `a_capless_targeted…` |

The Q6 check is the most valuable of these: it proves the strong isolation assertion genuinely
depends on per-workspace declaration and would have been a **false claim** under the wildcard.

## A bug found by the new check

The suite flaked ~1-in-5 in parallel, green alone and serially. Root cause: the helper hardcoded
node ids across tests, so concurrent tests declared the same node key (in-process peers share a
keyspace). The **production code was correct** — this is exactly what `MultipleResponders`
exists to catch, and it fired on a genuine duplicate-id collision.

Worth stating: **without** that check the suite would have passed silently by keeping the first
reply — the very hazard #81 removes. Fixed test-side (ids namespaced per test).
→ [`../../debugging/mcp/duplicate-node-ids-across-concurrent-tests.md`](../../debugging/mcp/duplicate-node-ids-across-concurrent-tests.md)

Separately: several runs hung with every network test stalled. That was **box load from an
unrelated `ems` test loop**, not this change — 11/11 in 1.47s once quiet. Noted so the next
session doesn't chase it.

## Invariants held

- **No new capability grammar.** A targeted call authorizes as `mcp:<ext>.<tool>:call`.
  Addressing is not authorization.
- **authorize strictly precedes resolve** — preserved in a single `call_inner` so the targeted
  and untargeted paths cannot drift.
- **The single-host untargeted path is unchanged in cost** — no new bus hop, no added lookup.
  The `MultipleResponders` drain adds no wait (the reply channel closes once all matching
  queryables answer).
- **A disconnected target is a refusal**, never a queue and never a fallback.
- **Rule 10** — no `if node == …`, no per-node tool names; the id is opaque data throughout.
- **WIT/guest ABI untouched** — verified. Routing errors reach a guest as opaque `Failed`; a
  guest has no target parameter and no node identity, so there was nothing to thread.

## For ems

The seam ems's `Provisioner` needs is stable:

```rust
lb_mcp::call_on_node(&registry, &bus, &principal, ws, "<ext>.<tool>", input_json, &node_id)
```

with `NodeId` from `lb_bus`, and three errors to react to: `Ambiguous { ext, candidates }`,
`NodeUnreachable { node }`, `NodeTooOld { node }`. Over HTTP those map to **409 / 503 / 502**.

**ems should pin the tag this lands under** (per the family workflow: lands here → tagged →
ems pins). The tag is not cut yet — see below.

## Not done / deliberately deferred

- **`NodeTooOld` is defined but never returned.** It needs the `targeted_dispatch` flag in the
  presence payload, which is fleet-presence's to publish (Q7). Shipping the variant now is
  deliberate — it cannot be retrofitted honestly later, since an old node cannot be taught to
  announce a flag retroactively. **Today an old node reads as `NodeUnreachable`**, which is the
  dishonest-refusal case the scope names; it closes when fleet-presence lands.
- **The ext-hosting announce** (`ws/{id}/nodes/{node}/ext/{ext}`) is still fleet-presence's, and
  is what will populate `register_remote_extension` in production. Until then the guard's
  candidate set comes only from wiring — the guard is **armed but not yet load-bearing in
  production**. This is the honest status, and the reason Finding A matters.
- **No tag cut.** Version bump + tag is a separate, deliberate step.
- **`agent/serve.rs` has the identical wildcard multi-responder shape** via `agent_call_key`
  (found in the entry gate, out of scope here). It should get the same treatment; flagged rather
  than silently widened.
