# Routed dispatch: duplicate node ids across concurrent tests → 1-in-5 flake

**Area:** mcp / routed-node-dispatch (#81)
**Found:** 2026-07-20, during the #81 implementing session
**Status:** fixed (test-side); the production code behaved correctly throughout

## Symptom

`crates/host/tests/routed_ambiguity_test.rs` passed 10/10 in isolation and under
`--test-threads=1`, but failed roughly **1 run in 5** under the default parallel runner.
Always the same test, `a_targeted_call_lands_on_the_named_node`, and always with a
tell-tale runtime: **~21s instead of ~1.2s** — the test's 20-second reachability deadline
being exhausted, not a fast assertion failure.

Re-running the single test in isolation 15 times did **not** reproduce it. That is the
signal that matters: a failure that vanishes when the test runs alone is an interaction
between concurrently-running tests, not a defect in the test that reports it.

## Root cause

The test helper `two_hosts_one_ext` hardcoded the node ids `node:gw-01` / `node:gw-02` for
**every** test in the file. Each test correctly used a unique *workspace*, but node ids
were shared.

In-process Zenoh peers share a keyspace (`debugging/bus/in-process-peers-share-the-keyspace.md`),
and these tests link over loopback TCP. So two tests running at the same moment each stood up
a hub declaring the queryable

```
ws/{their-own-ws}/mcp/{ext}/node:gw-01/call
```

with the **same ext id** (`fleet`) — leaving two live queryables that a `get` could match.
The caller then received two replies for a key that only one node should declare.

**The production code was right.** This is precisely the condition
`BusError::MultipleResponders` was added to detect: two nodes announcing the same node id.
The new runtime check fired, `dispatch` mapped it to
`"routing fault: more than one node answered … — duplicate node id?"`, and the retry loop
kept retrying (the error is not a success) until the deadline. The flake was the check
**working**, on a genuine duplicate-id collision that the test harness had manufactured.

Worth stating plainly: had this check not been added in the same slice, the tests would have
passed silently by keeping whichever reply arrived first — the exact hazard #81 exists to
remove — and the id collision would have gone unnoticed.

## Fix

Namespace the node ids by the (unique-per-test) ext id, mirroring the existing
unique-workspace discipline, and give each test its own ext id:

```rust
let (id_a, id_b) = (
    NodeId::new(format!("node:{ext}-gw-01")).expect("key-safe id"),
    NodeId::new(format!("node:{ext}-gw-02")).expect("key-safe id"),
);
```

No production change. `crates/host/tests/routed_ambiguity_test.rs` carries a comment at the
id construction pointing here, so the next person to add a test to that file inherits the
constraint instead of rediscovering it.

## Regression cover

The suite itself is the cover: the collision is reproduced by *any* two tests in this file
sharing a node id, so a future test that hardcodes one will flake the same way. The comment
at the construction site is the load-bearing part — a unique workspace is **not** sufficient
on this path, and that is not obvious.

## Lesson

Two ideas worth carrying:

1. **A failure that disappears in isolation is a cross-test interaction.** 15 clean solo runs
   next to a 1-in-5 parallel failure is diagnostic, not inconclusive — don't spend the budget
   re-running the accused test.
2. **On this bus, "unique workspace" is not the whole isolation story.** Anything that becomes
   a *key segment* — now including node ids — must be unique per test too, for exactly the
   reason workspaces already are.
