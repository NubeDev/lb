# Workspace fails to build: modules referenced but never declared (`mod …` missing)

- Area: store / host / gateway (build wiring)
- Status: resolved
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/store/persistent-backend-session.md
- Regression test: `cargo build --workspace` (the build itself is the gate)

## Symptom

At the start of the S9 ingest/tags work, `cargo build --workspace` failed before any of the new code
compiled:

```
error[E0433]: cannot find `channel_registry` in `crate`   (crates/host/src/channel/post.rs)
error[E0432]: unresolved import `crate::session`          (role/gateway/src/routes/*.rs)
error[E0061]: this function takes 0 arguments but 1 …     (node/src/main.rs → Gateway::boot)
```

## Reproduce

Check out the working tree as inherited and run `cargo build --workspace`.

## Investigation

- `crates/host/src/channel_registry/` and `role/gateway/src/session/` existed on disk (with `mod.rs` +
  verb files) but were **never declared** with `mod channel_registry;` / `mod session;` in their
  crate roots — an in-progress collaboration slice from a prior/parallel session left half-wired.
- `Gateway::boot` had been changed to take no args (workspace now comes from the request token), but
  `node/src/main.rs` still called `boot(&ws)`.
- Independent of the S9 slices; it blocked the whole workspace, so it had to be fixed first.

## Root cause

New modules added to the tree without the barrel `mod` declarations + one stale call site — a
mechanical wiring gap, not a logic bug.

## Fix

- `crates/host/src/lib.rs` — `mod channel_registry;` + re-exports (`channel_create`, `channel_list`,
  `register_on_post`, `ChannelRecord`).
- `role/gateway/src/lib.rs` — `mod session;` + re-exports (`authenticate`, `dev_claims`,
  `verify_token`, `AuthRejection`).
- `node/src/main.rs` — `Gateway::boot()` (no arg); workspace comes from the bearer token per request.

## Verification

`cargo build --workspace` is green; `cargo test --workspace` passes (with the `echo-sidecar` example
built — a separate prerequisite, see below).

## Prevention

A module directory is not wired until its parent declares `mod …` — treat a "cannot find module that
clearly exists" error as a missing barrel declaration. Note: the S7 `native_test` cases require
`cargo build -p echo-sidecar` first (they panic with that exact instruction if the sidecar binary is
absent) — build it before `cargo test --workspace`.
