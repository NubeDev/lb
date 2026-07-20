# Adding a host-side call timeout silently disabled restart-and-retry for hung children

**Area:** extensions (native Tier-2 transport) · **Status:** closed (2026-07-20)
**Session:** [`sessions/extensions/native-call-concurrency-session.md`](../../sessions/extensions/native-call-concurrency-session.md)
**Scope:** [`scope/extensions/native-call-concurrency-scope.md`](../../scope/extensions/native-call-concurrency-scope.md)

## Symptom

While adding the scope's host-side per-call bound (`CALL_TIMEOUT`, `host/src/native/call.rs`), the
supervised **restart-and-retry stopped happening for a child that stops answering**. Surfaced as
`a_real_restart_is_still_retried` failing with:

```
assertion `left == right` failed: the child was really replaced, so the one retry must proceed
  left: 1
 right: 2
```

Only **one** `call` frame ever reached the child — the retry never ran.

## Root cause

The fault arm of `call_once_or_restart` matched exactly one variant:

```rust
match first {
    Ok(out) => Ok(out),
    Err(SupervisorError::Transport(_)) => { on_fault().await?; /* retry */ }
    Err(other) => Err(other),          // ← Timeout landed here
}
```

Before the timeout existed, a child that stopped answering could only surface **two** ways: a
`Transport` error (EOF — the pipe closed) or an indefinite hang. The `Transport` arm therefore covered
every recoverable case by construction.

Introducing `CALL_TIMEOUT` created a **third** way for the same physical condition to present:
`SupervisorError::Timeout`. It fell through to the catch-all `Err(other) => Err(other)` — so a hung
child got no recovery and no retry, which is precisely the case supervision exists for. The timeout
converted a *hang* into a *silent supervision hole*.

This is the dangerous shape of the bug: the new error variant was added for a good reason, correctly
constructed, and correctly returned. Nothing about it was wrong except that an **existing** `match`
elsewhere had been exhaustive over the old world and became a silent behaviour change in the new one.

## Fix

`Timeout(_)` joins `Transport(_)` in the fault arm — the same condition ("the child is not
answering"), so it takes the same recovery path:

```rust
Err(SupervisorError::Transport(_)) | Err(SupervisorError::Timeout(_)) => {
    on_fault().await?;
    ...
}
```

`Child(_)` deliberately stays **out** of it: an error *reply* over a healthy line (a failed SQL query,
a bad arg) is not a fault. Restarting on it once burned the whole restart budget on five failed
queries and took federation dark mid-run while the child was never down — the regression `call.rs`
documents in blood.

## Regression test

`crates/host/src/native/call.rs::tests::a_real_restart_is_still_retried` — a child that answers the
handshake, counts `call` frames, and never replies; the recovery performs a genuine `restart()`, so
exactly **2** frames must be observed (attempt + retry). Fails-before with `left: 1`.

Its complement, `a_noop_recovery_does_not_retry_the_same_generation`, asserts **1** frame when the
recovery is a no-op, so the pair pins both directions: the retry must happen when the child was
really replaced, and must not when nothing recovered it.

Both run in ~0.4 s because `call_timeout()` is `#[cfg(test)]`-overridden to 200 ms — the production
45 s bound would otherwise make the fault paths untestable in practice (the first version of this
test took 90 s and would have been deleted or ignored).

## Lesson

**Adding an error variant can silently change behaviour in a `match` you did not touch.** A
non-exhaustive `match` with a catch-all arm (`Err(other) => Err(other)`) does not fail to compile when
a new variant starts flowing through it — it just quietly routes the new case to the default. The
compiler protects exhaustive matches; it cannot protect a catch-all.

So when adding a variant, **grep for every `match` on that error type and ask which arm the new
variant lands in** — especially where a catch-all encodes "everything else is not recoverable." Here
the new variant meant *exactly* the recoverable condition, and the catch-all said the opposite.

Corollary: **a new timeout is not a purely additive safety feature.** It reclassifies a failure that
previously presented as something else, and everything downstream that branched on the old
classification is now wrong until re-checked.
