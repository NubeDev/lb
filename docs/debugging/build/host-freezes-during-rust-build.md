# The whole desktop hard-freezes (forced reboot) during Rust builds — no OOM log

- Area: build / dev-environment
- Status: mitigated (awaiting flight-recorder confirmation of root cause)
- First seen: 2026-06-27
- Session: ../../sessions/build/host-freeze-mitigation-session.md
- Regression test: n/a (host/runner-resource constraint, not a product bug — guarded by build tuning + a memory-capped build wrapper + a userspace OOM killer)

## Symptom

The dev box (Zorin OS 18, 31 GiB RAM, 28 cores, VSCode running as a **Flatpak**)
repeatedly freezes hard enough to require a manual reboot while working on this Rust
workspace. Recurs over and over. Crucially, **the host journal contains no `oom-kill`,
no `hung_task`, and no `earlyoom` SIGTERM** around the freezes — the previous boot ends
with a clean `systemd-reboot` (the user rebooting by hand), with no trace of what blew up.

## Reproduce

A full/parallel `cargo build` (or test) of the workspace. A live snapshot during one such
build caught the smoking gun:

```
rust-lld   ~4.0 GB   (×3 linkers running at once)
rustc      several   (codegen)
Swap:      2.0Gi used / 64Ki free   (100% full)
loadavg    14+
```

Three `rust-lld` linkers at ~4 GB each + VSCode + Chrome + agents → past 31 GiB RAM →
the tiny 2 GiB swapfile fills → the box thrashes.

## Investigation

- `target/` had grown to **168 GB** — accumulated junk, plus default dev builds embed
  FULL debug info, which is why each `rust-lld` needed ~4 GB.
- `/` is a 16 GB **tmpfs** (RAM-disk) and `/tmp` is tmpfs too — temp writes eat real RAM.
- `.cargo/config.toml` already capped `jobs` but the comment was stale (written for a
  15 GB/14-core machine; this box is 31 GB/28 cores) and mold was not installed.
- VSCode is a **Flatpak**: the host's `mold` lives at `/run/host/usr/bin/mold` and is not
  on the sandbox PATH; there is no `clang` in the sandbox.
- **No OOM/earlyoom/hung-task log** despite the freeze. This points away from a clean
  OOM and toward a **swap death-spiral**: RAM fills, the kernel frantically swaps to the
  2 GiB swapfile, every process (including `earlyoom` and `journald`) gets stuck in
  uninterruptible disk-wait, the machine livelocks, and nothing is ever written to disk
  before the manual reboot. earlyoom's default 10% threshold fires too late to escape it.

## Root cause (suspected — being confirmed)

Peak build memory (parallel `rust-lld` linkers, each carrying full debug info) exceeds
physical RAM; the undersized 2 GiB disk swap turns the overflow into an unrecoverable
I/O livelock rather than a clean OOM kill — hence the total freeze with no logs. A
persistent flight recorder is now armed to capture the next event for certainty.

## Fix / mitigation

Layered defense (no single `if` — bound the resource at every level):

1. **Smaller link footprint** — `[profile.dev]` in `rust/Cargo.toml`:
   `split-debuginfo = "unpacked"` + `debug = "line-tables-only"` (kills the ~4 GB/linker
   spikes and shrinks `target/`).
2. **mold linker** — `rust/.cargo/config.toml` sets
   `rustflags = ["-C","link-arg=-fuse-ld=mold"]`. Flatpak workaround:
   `ln -sf /run/host/usr/bin/mold ~/.local/bin/ld.mold` (on PATH) so gcc finds it;
   gcc is the link driver (no clang in sandbox). Changing rustflags invalidates the
   build cache, so this is the moment to `cargo clean` and reclaim the 168 GB.
3. **Hard memory cap on builds** — `~/.local/bin/cargo-safe` runs cargo inside a user
   cgroup: `systemd-run --user --scope -p MemoryMax=22G -p MemorySwapMax=2G cargo "$@"`.
   Use `cargo safe build`; if the build exceeds 22 G only the build is killed, never the
   desktop. (Verified: the user slice has the `memory` controller delegated.)
4. **Userspace OOM killer** — `earlyoom` installed + active; hardened to fire early and
   escape the spiral: `EARLYOOM_ARGS="-m 20 -s 90 -r 60 --prefer '…rustc|rust-lld|cargo|mold…' --avoid '…code|gnome-shell|Xorg…'"`.
5. **Flight recorder** — `~/.local/bin/.../freeze-recorder.sh` (launched detached on the
   host) appends mem/swap + top-RSS every 2 s to `~/freeze-recorder.log` and `sync`s each
   line, so the next freeze is diagnosable post-reboot.

## Verification

- gcc + mold link test: `gcc -fuse-ld=mold … → LINK OK`.
- `cargo-safe` scope test: `systemd-run --user --scope -p MemoryMax=22G true → OK`.
- Recorder confirmed writing to `~/freeze-recorder.log`.
- Pending: a `cargo safe build` full rebuild watched live, and reading the recorder log
  after the next freeze to confirm (or refute) the swap-spiral root cause.

## Update 2026-06-27 — the recurring "stuck" is TWO problems, not one

The flight recorder caught a "stuck again" event and **exonerated the system**: at the
moment of the hang, `MemAvailable` was ~25 GB (78% free), the recorder never gapped (no
freeze), there were **no `rustc`/`rust-lld`/`cargo`** processes, nothing pegged a core,
no `D`-state I/O stall, and no rust-analyzer running. The exthost log shows no errors.

Conclusion: there are **two distinct issues** that were being conflated:

1. **Real OOM hard-freeze during big builds** (the original: 3× `rust-lld` @ ~4 GB +
   2 GiB swap 100% full → reboot) — mitigated by the fixes above (mold, debug trim,
   `cargo safe` cgroup cap, earlyoom).
2. **VSCode "stuck/slow" while the system is healthy** — this is the **Flatpak VSCode**
   app hanging (sandboxed `bwrap`/`zypak` extension host + slow IPC), NOT a Linux freeze.
   No memory/build tuning addresses it because memory was never the bottleneck here.

**Fix for #2:** replace Flatpak VSCode with the native Microsoft `.deb` (apt repo) and
launch `code` from a normal terminal. The build fixes still apply; the native extension
host gets unsandboxed FS/IPC access and stops hanging.

The recorder is left running as a safety net — a *true* future freeze would show a gap +
spike in `~/freeze-recorder.log`, distinguishing a real OOM from an app-level hang.

## Prevention

- **Build with `cargo safe …`**, not bare `cargo`, on this box.
- Keep dev debug info trimmed and mold enabled; don't let `target/` balloon again.
- Consider replacing the 2 GiB swapfile with **zram** (compressed RAM swap) so overflow
  compresses instead of thrashing a slow disk.
- If freezes persist after this, the **Flatpak VSCode** is the next suspect (sandboxed
  rust-analyzer + restricted resources) — switch to the native `.deb`/apt build.
- Related: [bus/cargo-test-workspace-ooms-with-many-peers](../bus/cargo-test-workspace-ooms-with-many-peers.md)
  (the same machine's OOM under test concurrency — bound the resource, not the code).
