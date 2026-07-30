# A raise's declared `tags` never reached the tag graph unless the producer also held `mcp:tags.add:call`

- Area: insights / tags / caps
- Status: resolved
- First seen: 2026-07-30 (while building the tag echo — the echo is what made it visible)
- Resolved: 2026-07-30
- Session: ../../sessions/insights/insight-tag-echo-session.md
- Scope: ../../scope/insights/insight-tag-echo-scope.md
- Regression tests: `rust/crates/host/tests/insight_tag_echo_test.rs::echo_is_the_union_across_raises_not_this_raises_declaration`,
  `::the_echo_never_crosses_the_workspace_wall`, `::the_echo_is_not_caller_writable`

## Symptom

Nothing — and that is the entry. `insight.raise {tags: {...}}` returned success, the record landed,
subscriptions with tag filters matched, and the tags were **not in the tag graph**. The only way to
notice was to ask the graph a question nobody asked: `tags.find`/`tags.of` on an insight raised by an
ordinary producer came back empty, while the matcher had just matched on those very facets.

## Reproduce

Raise as a principal holding `mcp:insight.raise:call` and **no** tags capability (the shape of every
rule that isn't also a tag author — and of the real member token the running product mints: it
carries `tags.add` + `tags.find` but not `tags.of`):

```
insight.raise { dedup_key: "k", tags: { building: "chullora-dc" }, … }   → ok
lb_tags::of(store, ws, "insight:<id>")                                   → []
```

## Investigation

`host/src/insight/raise.rs` applied each declared tag through the **capability-gated host verb**:

```rust
let _ = crate::tags::tags_add(&node.store, principal, ws, &entity, &tag, &prov).await;
```

`tags_add` runs `authorize_tags(principal, ws, "tags.add")` first, so a producer without that grant
was denied — and `let _ =` swallowed the denial whole. Deliberately best-effort ("a tag hiccup must
not fail the raise", which is right), but it cannot distinguish a store hiccup from a policy denial,
and the denial was the common case.

It stayed invisible because the one consumer of those tags papered over it. `materialize_facets`
reads the graph and **falls back to `RaiseInput.tags` when the read fails or returns empty** — so
the subscription matcher received the right facets for *this* firing regardless, and every matcher
test passed. The fallback is correct as a hiccup guard; it just happened to mask a permanent state.
Two independent gates, same shape: the read side (`tags_of`, gated on `mcp:tags.of:call`) would have
degraded the same way even after the write was fixed.

The tag echo is what forced it into the open: an echo built from the fallback is one raise's
*declaration*, not the union across raises — silently wrong in exactly the way the scope's resolved
decision 2 exists to prevent.

Ruled out: the tag-node cap (nowhere near 10k), `lb_tags::add` itself (correct), and the entity
format (`insight:<id>` is what `tags.find` strips in `insight_list`).

## Root cause

A host-internal effect of one verb was gated on **another verb's capability**. `tags` is a declared
field of `insight.raise`'s own input, applied to an entity that same call just minted — so
`mcp:insight.raise:call` is the authority for it. Requiring `mcp:tags.add:call` on top meant the
documented `tags` field of `insight.raise` didn't work under the cap that authorizes `insight.raise`.

## Fix

`rust/crates/host/src/insight/raise.rs` — apply tags with the **raw** graph op
(`lb_tags::add(.., DEFAULT_TAG_NODE_CAP)`), and materialize with the raw `lb_tags::of`, both
host-internal behind the already-passed `mcp:insight.raise:call` gate. The per-workspace tag-node cap
is preserved (it is an argument to `add`, not part of the capability gate). A failure now logs a
`tracing::warn!` instead of vanishing into `let _ =`.

This is the same reasoning `insight_list` already used one file over: it resolves its facet filter
through the raw `lb_tags::find` because `mcp:insight.list:call` authorized the workspace read. No
capability was widened — a caller still cannot reach `tags.*` for any other entity; the raise path
writes tags only on the insight it just created.

## Verification

Revert-check: restore the gated `crate::tags::tags_add` call and re-run — `union` (the echo becomes
one raise's declaration), `workspace_wall`, and `not_caller_writable` all go **red**; restored →
11/11 green. Live: two raises with disjoint tag maps under a token with no `tags.of` grant, then one
`insight.list` returns the union on the row.

## Prevention

- **`let _ = <gated verb>` is a silent-drop generator.** A best-effort side effect must still
  distinguish "the store hiccuped" from "you were denied" — log the error, and ask whether the
  denial is even the right policy.
- **A fallback added as a hiccup guard will mask a permanent failure.** `materialize_facets`'
  fallback made a completely empty graph indistinguishable from a healthy one for the matcher. When
  writing a fallback, ask which *steady state* it would hide, and pin that state with a test that
  can only pass through the real path (here: the union across raises — a declaration can't fake it).
- **A host-internal effect of verb A must gate on A's capability**, not on the capability of
  whatever subsystem stores it. If a declared field of a verb's input needs a second grant to take
  effect, the verb's contract is a lie for every caller that lacks it.
