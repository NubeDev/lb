# Backgrounding `flows.run` failed to compile: drive future "not `Send`" (opaque-type recursion)

- Area: flows
- Status: resolved
- Session: [`sessions/flows/flow-runtime-control-session.md`](../../sessions/flows/flow-runtime-control-session.md)
- Scope: [`scope/flows/flow-runtime-control-scope.md`](../../scope/flows/flow-runtime-control-scope.md)

## Symptom

Making the manual run a background job — `tokio::spawn` the `coordinator::drive` future from
`flows_run_async` — failed to compile:

```
error: future cannot be sent between threads safely
  ...
note: future is not `Send` as it awaits another future which is not `Send`
  --> crates/host/src/flows/mod.rs:110   (the `flows.run` dispatch arm)
note: fetching the hidden types of an opaque inside of the defining scope is not supported.
      You can try moving the opaque type ... into a new submodule
```

The compiler kept pointing back at `flows_run_async` itself, not at any concrete non-`Send` value.

## Root cause

It is **not** a genuine non-`Send` capture — a standalone `assert_send(coordinator::drive(...))`
passed. It is an **async opaque-type recursion cycle**. A flow's generic `tool` node can call
`flows.run`, so the call graph is:

```
flows_run_async → tokio::spawn(drive) → coordinator::drive → execute_node → call_tool
   → (a `tool` node whose verb is `flows.run`) → dispatch → flows_run_async → ...
```

Every link is an `async fn` returning an **opaque** `impl Future`. To prove the spawned future is
`Send`, the compiler must compute the hidden type of `drive`, which requires the hidden type of
`flows_run_async`, which contains the spawn of `drive`… — an infinitely-recursive opaque type it can
neither *size* nor prove `Send`. Boxing the recursive `.await` sites didn't help, because
`Pin<Box<impl Future>>` keeps the *inner opaque type* (the cycle is in the type, not the indirection).
The agent loop spawns tool-calling work fine precisely because it does **not** recurse into itself.

## Fix

**Type-erase the recursion edge to a *concrete* boxed `dyn Future + Send` at the dispatch boundary.**
Added `call_flows_tool_boxed` whose **signature** is
`Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send>>` — a named, sizeable type, not an
opaque `impl Future`. The host dispatcher (`tool_call.rs`) routes `flows.*` through it. Because the
boundary type is concrete, the cycle `… → dispatch → flows_run_async` no longer references an opaque
type defined in the same scope, so the compiler sizes it and proves `Send`. The `Send` bound is
honest — `dispatch` only ever touches `Arc`/`Store`/`Bus`, which is why the standalone `drive` was
`Send` all along.

`flows_run_async` then spawns a **named** `drive_run_task` (its own future type, not an anonymous
closure nested in the caller) — belt-and-suspenders against re-introducing the cycle.

## Regression coverage

Compilation IS the regression test (the workspace builds + all flows tests link and run). The
behavioral guarantees the async path unlocks are covered by
`crates/host/tests/flows_runtime_control_test.rs`:
`run_is_a_background_job_returns_before_terminal_then_settles` and
`cancel_status_written_before_drive_is_honored_deterministically`.

## Lesson

A "future is not `Send`" that points at the *function itself* (and survives boxing the `.await`s) is
an **opaque-type recursion**, not a captured non-`Send` value. Cut it by giving the recursion edge a
**concrete** type — a `fn` returning `Pin<Box<dyn Future + Send>>` — not by boxing call sites. The
compiler's own hint ("move the opaque type into a new submodule" / "fetching the hidden types of an
opaque inside of the defining scope") is the tell.
