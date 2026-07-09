# Session — persistent store for the standalone `full` desktop build

Scope: [`docs/scope/desktop/desktop-persistent-store-scope.md`](../../scope/desktop/desktop-persistent-store-scope.md).

## The ask

Reported: "when I restart the desktop it's lost all my work — does it have a db, or is everything in
memory?" Answer: everything was in memory. `Node::boot` → `open_store()` opens `Store::memory()`
unless `LB_STORE_PATH` is set, and the desktop shell never set it — every launch was a fresh,
ephemeral node. This was a known, already-recorded non-goal of the standalone-backend scope ("full
still boots an in-memory node per launch... Leaning: yes [persist], but as a separate [scope]"), not a
regression — but a shipped desktop app needs a real DB, full stop.

## Decision

Two choices, both taken as recommended:

- **DB location: the OS-standard per-user data dir** (`dirs::data_dir()/lazybones/store` — Windows
  `%APPDATA%`, Linux `~/.local/share`, macOS `~/Library/Application Support`), not beside the exe.
  Survives app updates/reinstalls, no Program-Files write-permission problem on Windows, one store
  per OS user. `dirs` was already a workspace dep (`host_tools/fs/home.rs`); just needed adding to
  the shell crate.
- **Seeders keep running every boot, idempotently.** They're already LWW-upserts, so an app update
  refreshes built-in roles/skills/agents without touching user data — a returning user keeps their
  channels/dashboards/datasources and gets current built-ins. (Rejected: skip seeding when a store
  already exists — that would ship stale built-ins to existing users on every update.)

## The one design call that mattered: WHERE the default is set

`Node::boot()`/`NodeHandle::boot()` are called directly by the shell's own tests (`commands_test.rs`,
`full_loopback_test.rs`, `full_federation_test.rs`) — those need isolated, ephemeral stores per test
run. Defaulting `LB_STORE_PATH` inside `NodeHandle::boot` would make every test share one on-disk
store (cross-contamination, flaky concurrent runs). So the default is resolved and set **only at the
windowed binary boundary** (`desktop.rs::run`, before `NodeHandle::boot("acme")`) — the one code path
only the shipped app takes. `Node::boot`/`open_store` are **completely unchanged**; they already did
the right thing whenever `LB_STORE_PATH` was set (used today by `make cloud`/`edge`).

## What shipped

- [`ui/src-tauri/src/store.rs`](../../../ui/src-tauri/src/store.rs) — `resolve_store_path()` (data
  dir → `lazybones/store`, falls back to exe-adjacent `./store` if the data dir can't be resolved,
  never silently falls to in-memory) and `ensure_store_path()` (fills `LB_STORE_PATH` if unset;
  honors an explicit path or an explicit empty value as-is; returns a log string).
- [`ui/src-tauri/src/desktop.rs`](../../../ui/src-tauri/src/desktop.rs) — one line in `run()`, before
  `NodeHandle::boot`: `println!("full: {}", store::ensure_store_path())`.
- `dirs = { version = "5", optional = true }` added to the shell crate, pulled in by the `full`
  feature only (the thin shell needs nothing here).

## Tests (real infra, no mocks — rule 9)

[`ui/src-tauri/tests/full_persist_test.rs`](../../../ui/src-tauri/tests/full_persist_test.rs) — the
headline regression: sets an explicit temp `LB_STORE_PATH`, boots `full`, registers a datasource the
seeders do NOT create (`my-source`, so the proof is about USER data, not the idempotent
`demo-buildings` reseed), drops the node + gateway, **re-boots at the same path**, and asserts
`my-source` is still listed. Before this fix a re-boot on `memory()` would find nothing.

```
cargo test --features full --test full_persist_test      # 1 passed
cargo test --features full --test full_loopback_test     # 3 passed (still in-memory — no LB_STORE_PATH set)
cargo test --features full --test full_federation_test   # 1 passed (still in-memory)
```

The other `full_*` tests staying green with no `LB_STORE_PATH` set is itself proof the default lives
at the right seam — if it had leaked into `Node::boot`, those tests would start sharing state.

**Manual verification (user, real `linux-full` build):** confirmed on the actual packaged binary —
data now survives a restart.

## Docs

- Public [`doc-site/content/public/desktop/desktop.md`](../../../doc-site/content/public/desktop/desktop.md)
  updated: the standalone build now persists by default; documents the per-OS path and the
  `LB_STORE_PATH` override.
- `docs/STATUS.md` updated with a "just shipped" entry.
- No debugging entry: this was a recorded scope gap being closed on schedule, not a regression caught
  mid-session.

## Open (from the scope, unchanged)

- **Persisted signing key** — data survives, but a restart mints a fresh gateway key, so the user is
  logged out (a stored browser token 401s). Recorded as the next natural follow-up: same data-dir, a
  `signing_key::resolve` that reads/writes a key file before `Gateway::new_live`.
- **Store migrations, multi-instance locking, backup/reset UI** — all explicit non-goals, noted not
  built.
