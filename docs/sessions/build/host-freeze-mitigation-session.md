# Session — host hard-freezes during Rust builds (mitigation)

- Date: 2026-06-27
- Area: build / dev-environment
- Outcome: mitigated; flight recorder armed to confirm root cause on next freeze.

## The ask

The dev box keeps freezing hard enough to require a manual reboot, repeatedly, while
working on the Rust workspace in VSCode. "It's killing me … ongoing over and over."

## What we found

- Live snapshot during a build: 3× `rust-lld` at ~4 GB each, swap (2 GiB) 100% full,
  loadavg 14 — memory pressure from parallel linkers.
- `target/` = 168 GB; dev builds embed full debug info (why each linker is ~4 GB).
- VSCode is a **Flatpak**: host `mold` at `/run/host/usr/bin/mold`, not on sandbox PATH;
  no `clang` in sandbox.
- **No OOM/earlyoom/hung-task in the journal** around freezes → suspected swap
  death-spiral (livelock on the tiny disk swap), not a clean OOM. earlyoom's default 10%
  threshold fires too late to escape it.

## What we did

1. `[profile.dev]` debug trim (`split-debuginfo=unpacked`, `debug=line-tables-only`) in
   `rust/Cargo.toml`.
2. Enabled mold in `rust/.cargo/config.toml`; symlinked `~/.local/bin/ld.mold →
   /run/host/usr/bin/mold` for the Flatpak case; gcc as link driver. Verified link OK.
3. Set `jobs = 8`, fixed the stale (15 GB/14-core) comment.
4. Installed `earlyoom` (active); provided a hardened `EARLYOOM_ARGS` (fire at 20% RAM /
   90% swap, prefer killing rust toolchain, avoid VSCode) — pending the user's sudo.
5. `~/.local/bin/cargo-safe`: `cargo safe …` runs cargo in a 22 GB-capped user cgroup.
   Verified the user slice has the `memory` controller delegated.
6. Flight recorder (`~/freeze-recorder.sh`, detached on host) logging mem/top every 2 s
   to `~/freeze-recorder.log` with `sync`, to capture the next freeze.

## Next

- After the next freeze: read `~/freeze-recorder.log` tail to confirm swap-spiral.
- Build with `cargo safe build`; reclaim `target/` with `cargo clean` on the next rebuild.
- If still freezing: replace 2 GiB swapfile with zram; consider native (non-Flatpak)
  VSCode. See debug entry: ../../debugging/build/host-freezes-during-rust-build.md
