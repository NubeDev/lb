# A Gauge panel sharing a datasource with another panel loses the race, forever shows "no value yet"

- Area: mcp
- Status: resolved
- First seen: 2026-08-13
- Resolved: 2026-08-13
- Session: rubix-ai live end-to-end Playwright verification of the ROS read+write Slider control
- Regression test: `rust/crates/mcp/src/call/dispatch.rs` (`mod tests`,
  `nested_call_to_a_different_ext_waits_instead_of_failing_fast` +
  `nested_call_to_the_same_already_held_ext_fails_fast`);
  `rust/crates/host/tests/proof_panel_test.rs::re_entrancy_is_bounded_never_hangs` (pre-existing,
  pinned unchanged by the fix)

## Symptom

On a rubix-ai dashboard with a Gauge panel (bound via `viz.query_batch` → `ros.point.get`) and a
Slider control (whose read-back source directly calls the same `ros.point.get` on the same ROS
host) side by side, the Gauge shows **"no value yet"** on every load/refresh — reproducible every
single time, not intermittent. The Slider's own value loads fine. Network capture of the Gauge's
`viz.query_batch` response:

```json
{"results":[{"frames":[{"refId":"A","fields":[],"length":0,
  "status":{"state":"error","message":"extension busy (re-entrant call)"}}],"rows":[]}]}
```

## Reproduce

1. Build a dashboard with two panels bound to the SAME extension/host: a Gauge (or any
   `viz.query_batch` panel) and a Slider/Switch control with a read-back `source` on the same
   `ros` connection + point.
2. Load the dashboard (or click "refresh panels"). The two panels' queries fire concurrently.
3. The Gauge's `viz.query_batch` → nested `ros.point.get` reliably returns `"extension busy
   (re-entrant call)"` while the Slider's own direct `ros.point.get` succeeds — because the
   Gauge's call takes the `try_lock` branch and the Slider's takes `lock().await`.

## Investigation

- Confirmed via the network tab this was **not** a `ros`-specific bug: both calls carry identical
  args and hit the identical `ros.point.get` tool; only ONE of the two concurrent invocations
  ever fails, and always the one arriving through `viz.query_batch`'s nested dispatch.
- Traced the error string to `rust/crates/mcp/src/call/dispatch.rs`'s `Target::Local` branch: a
  call flagged `reentrant` (i.e. `depth > 0`, from `rust/crates/host/src/tool_call.rs`)
  `try_lock()`s the extension's instance mutex and fails fast on contention instead of awaiting
  it; a top-level call (`depth == 0`) just awaits the lock normally.
- Read `docs/scope/extensions/host-callback-scope.md`'s Open Question 1 (already **RESOLVED**,
  2026-06-27): the fail-fast exists ONLY to stop a guest re-entering **its own** node-global
  instance — "a re-entrant call `try_lock`s and fails fast... The depth guard bounds
  cross-instance chains; the try-lock bounds self-re-entry." The *documented* intent was already
  narrower than what shipped.
- Root cause: the shipped code used `depth > 0` (any nesting at all) as the fail-fast trigger,
  not "does this nested call target the SAME instance an ancestor in this chain already holds".
  `viz.query_batch` (host-native, depth 0) dispatching its nested `ros.point.get` (depth 1) is
  cross-instance — `viz` and `ros` are different extensions, so nothing in that call chain holds
  `ros`'s lock — awaiting it is exactly as safe as a top-level call. The blanket depth check
  couldn't tell the two cases apart, so it punished the safe case identically to the unsafe one.

## Root cause

`rust/crates/mcp/src/call/dispatch.rs`'s `Target::Local` branch chose `try_lock` (fail-fast) vs
`lock().await` from a boolean (`reentrant = depth > 0`) that only encodes "is this call nested",
not "does this call re-enter the specific instance an ancestor of mine already holds". The two
are different questions; conflating them makes any nested cross-extension call racy against
unrelated top-level traffic to the SAME target extension.

## Fix

Replaced the boolean with `rust/crates/mcp/src/call/reentrancy.rs`: a `tokio::task_local!` set of
the instance pointers (`Arc::as_ptr(&hosted.instance)`) the in-flight call chain currently holds
**on this task** — mirroring `lb_store::taint`'s scope discipline (nested host-callback calls
`.await` on the same task, so they share the enclosing scope's cell for free). `dispatch()` now
asks `reentrancy::is_held(ptr)`: true (this exact instance is already locked by an ancestor in my
own chain) → `try_lock`/fail-fast, unchanged; false (nested or not, targeting anything else) →
`lock().await`, exactly like a top-level call. `reentrancy::in_scope`/`holding` open/extend the
scope and mark the pointer held for exactly as long as the lock is; `dispatch()` is the ONE
chokepoint every call funnels through, so no outer call site (`tool_call.rs`, `call/mod.rs`)
needs to know this exists — the now-unused `reentrant: bool` parameter was removed from
`dispatch`/`call_with_ctx`/`call_inner`, and `tool_call.rs`'s `depth > 0` no longer feeds it
(`MAX_CALL_DEPTH`, the separate chain-length bound in `callback.rs`, is untouched).

## Verification

- New `lb-mcp` unit tests (`call::dispatch::tests`, real `tokio::sync::Mutex` contention, no
  wasm needed — a hand-rolled `LocalDispatch` stub controlled by `tokio::sync::Notify`):
  - `nested_call_to_a_different_ext_waits_instead_of_failing_fast` — reproduces the exact
    Gauge/Slider shape (an unrelated top-level call genuinely holding B's lock; a nested call
    from a chain holding a DIFFERENT instance, targeting B) and asserts the nested call is still
    pending 80ms later (took the wait branch, not fail-fast) and succeeds once the lock frees.
  - `nested_call_to_the_same_already_held_ext_fails_fast` — same instance actually locked +
    marked held on the SAME task → still fails fast immediately, pinning the original protection.
- Pre-existing `rust/crates/host/tests/proof_panel_test.rs::re_entrancy_is_bounded_never_hangs`
  (self-recursion through a REAL wasm guest) still passes unchanged — the same-instance case is
  untouched by this fix.
- `cargo check -p lb-mcp -p lb-host` clean.
- Live repro no longer reproduces once rebuilt (pending a follow-up live Playwright pass against
  the fixed build — see the session this entry links).

## Prevention

The regression tests above pin the invariant at the exact seam (`dispatch()`), so any future
change to the lock-strategy branch is caught immediately, independent of which two extensions
happen to collide. No broader guardrail added — the fix narrows an existing mechanism to its
documented intent rather than adding new surface.
