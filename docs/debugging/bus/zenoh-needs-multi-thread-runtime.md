# Booting a Node in a test panics: "Zenoh runtime doesn't support Tokio's current thread scheduler"

- Area: bus
- Status: resolved
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/core/s0-s1-spine-session.md
- Regression test: rust/crates/host/tests/spine_test.rs (all four boot a Node)

## Symptom

Every `#[tokio::test]` that booted a `Node` panicked inside Zenoh:

```
Zenoh runtime doesn't support Tokio's current thread scheduler. Please use multi thread
scheduler instead, e.g. `#[tokio::main(flavor = "multi_thread", worker_threads = 1)]`
```

The four S1 spine tests all failed at `Node::boot()` (which opens a Zenoh peer), before any
assertion ran.

## Reproduce

`#[tokio::test] async fn t() { Node::boot().await.unwrap(); }` — panics in
`zenoh-runtime-1.9.0/src/lib.rs:149`.

## Investigation

- The failure was in `Bus::peer()` → `zenoh::open`, not in our code.
- `#[tokio::test]` defaults to the **current-thread** runtime. Zenoh spawns its own runtime
  machinery that asserts a multi-thread scheduler. The `node` binary (`#[tokio::main]`,
  multi-thread by default) was never affected — only the tests.

## Root cause

A dependency runtime requirement, not a logic bug: Zenoh requires a multi-thread Tokio
scheduler. The default test runtime is single-thread, so the requirement is violated only in
tests.

## Fix

Annotate every test that boots a `Node` with
`#[tokio::test(flavor = "multi_thread", worker_threads = 1)]`. One worker thread is enough —
the point is the multi-thread *scheduler*, not parallelism. Recorded as a standing constraint
in `scope/bus/bus-scope.md` so future bus-touching tests use it from the start.

## Verification

`cargo test -p lb-host` — all four spine tests pass (output in the session doc).

## Prevention

The four spine tests are the regression guard. Documented constraint in the bus scope; any
new test that boots a Node (or a bus peer directly) must use the multi-thread flavor. A
follow-up could add a tiny `#[lb_test]` macro that bakes in the flavor, if this bites often.
