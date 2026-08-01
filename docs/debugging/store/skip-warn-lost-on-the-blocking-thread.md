# The boot pass's "I skipped it" warning never reached the subscriber — a loud guard that was silent

- Area: store (`crates/store/src/open.rs::open_with` → `spawn_blocking` → `boot_pass.rs`)
- Found: 2026-08-01, while building the boot memory guard (issue
  [#128](https://github.com/NubeDev/lb/issues/128)) — by the test written to prove the skip is loud.
- Severity: observability — but of the one event this whole scope exists to make visible.
- Status: fixed + regression assertion.

## Symptom

`skip_is_loud_and_recorded_and_the_node_still_opens` installed a `tracing` subscriber over a
capturing writer, opened a store with too little memory headroom, and asserted the WARN line. The
returned `CompactionRecord` carried `skipped: Some(reason)` — correct — but the captured log was
**empty**. Not a missing field, not a wrong level: nothing at all.

## Root cause

The boot compaction pass is deliberately run off the async workers:

```rust
tokio::task::spawn_blocking(move || boot_compact(&owned, available_ram))
```

A `tracing` subscriber installed with `set_default` (or `with_default`) is **thread-local**. The
blocking pool thread inherits nothing, so every event the pass emits — including the skip warning —
is dispatched to the no-op subscriber and dropped on the floor.

This is not a test artefact. Any embedder that scopes a subscriber (rather than installing one
globally) would have had the skip decision vanish silently — while the scope's entire premise is
"skipping is loud, never silent". The record would still say `skipped`, but only for someone who
thought to call `store.status`; the operator reading `journalctl` after an OOM incident would see
nothing.

## Fix

Carry the caller's dispatcher onto the blocking thread:

```rust
let dispatch = tracing::dispatcher::get_default(|d| d.clone());
let boot_pass = tokio::task::spawn_blocking(move || {
    tracing::dispatcher::with_default(&dispatch, || boot_compact(&owned, available_ram))
})
.await
.ok();
```

Globally-installed subscribers (the node binary's) were unaffected either way; scoped ones now work.

## Regression test

`crates/store/tests/boot_memory_guard_test.rs::skip_is_loud_and_recorded_and_the_node_still_opens`
asserts on the **captured log text** (that it contains "SKIPPING the boot compaction pass", the log
size and the available-RAM figure), not merely on the returned record — so a future change that
moves the pass to another thread or drops the line goes red. The test runs on a `current_thread`
runtime on purpose: with a multi-threaded runtime the awaiting thread is not the test's thread, and
the assertion would be testing the wrong hop.

## Lessons

- **`spawn_blocking` (and any thread hop) drops a scoped `tracing` subscriber.** If an event is part
  of a feature's contract, emit it where a subscriber can hear it — or carry the dispatcher.
- **"It's logged" is a claim, not a mechanism, until a test reads the log.** Asserting the returned
  struct would have passed happily while the operator-facing half was broken.

## Cross-links

- Session: [`sessions/store/boot-memory-guard-session.md`](../../sessions/store/boot-memory-guard-session.md)
- The incident this guard closes:
  [`boot-compaction-oom-kills-the-box.md`](boot-compaction-oom-kills-the-box.md)
