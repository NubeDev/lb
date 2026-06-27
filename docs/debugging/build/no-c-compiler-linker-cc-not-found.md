# Rust build fails: `linker 'cc' not found` (no C compiler on the box)

- Area: build / dev-environment
- Status: resolved
- First seen: 2026-06-27
- Session: ../../sessions/build/no-cc-zig-toolchain-session.md
- Regression test: n/a (host/runner-toolchain constraint, not a product bug — guarded by `rust/.cargo/config.toml` wiring `zig cc` as the compiler+linker; if it regresses, `cargo build` fails immediately and loudly, which is its own check)

## Symptom

```
error: linker `cc` not found
  = note: No such file or directory (os error 2)
error: could not compile `node` (bin "node") due to 1 previous error
```

All Rust crates compile; the build dies only at the **link** step. The node binary
itself runs fine on :8080 (it was built in another environment), so this is purely a
local-toolchain gap, not a code problem.

## Reproduce

`cd rust && cargo build --workspace` (or `cargo test`) on this dev box.

## Investigation

- No `cc`/`gcc`/`clang`/`zig`/`tcc` anywhere on PATH; **no root** to `apt-get install`
  (`dpkg` lock / not root).
- rustup ships `rust-lld` + `mold` is present, so a *linker* exists — but rustc invokes
  a `cc`-style **driver** to link, and there is none.
- A linker-only shim (`rust-lld` wrapped to translate `-Wl,` / `-m64` / `-shared` /
  crt-object args) got linking to work and built most of the workspace — but then
  **`ring 0.17` failed**: it uses `cc-rs` to *compile* C + asm, which lld cannot do.
  So a real C compiler is genuinely required, not just a linker.
- The leftover Flatpak `org.freedesktop.Sdk` gcc (15.2.0) **segfaults** when run outside
  the flatpak sandbox (it expects the runtime's mounted fs layout). The VSCode flatpak it
  came from has been removed. Dead end — do not use it.

## Root cause

The machine has no usable system C toolchain and no privilege to install one. `ring`'s
build script needs an actual C compiler; rustc needs a `cc` driver to link.

## Fix

Use a **self-contained `zig` toolchain** (no root). `zig cc` is a full clang-based C
compiler *and* linker in one ~45 MB download.

1. Download zig 0.13.0 to `~/.local/zig-linux-x86_64-0.13.0`.
2. Wrappers on PATH:
   - `~/.local/bin/zigcc` → `zig cc`, **also rewriting the target triple**
     `x86_64-unknown-linux-gnu` → zig's `x86_64-linux-gnu` (cc-rs passes the gnu
     triple; zig's clang rejects the `unknown` vendor with
     `UnknownOperatingSystem` / `unable to parse target query`).
   - `~/.local/bin/zigar` → `zig ar`.
3. Wire it into `rust/.cargo/config.toml` so plain `cargo` Just Works, no shell exports:
   ```toml
   [target.x86_64-unknown-linux-gnu]
   linker = "/home/user/.local/bin/zigcc"

   [env]
   CC = "/home/user/.local/bin/zigcc"
   AR = "/home/user/.local/bin/zigar"
   CC_x86_64_unknown_linux_gnu = "/home/user/.local/bin/zigcc"
   ```
   The old mold `-fuse-ld=mold` rustflag was removed — zig's bundled lld does the
   linking now, and that flag was written for a gcc-driver setup that no longer exists.

## Verification

Run with a clean env (`env -u CC -u AR -u RUSTFLAGS …`) to prove the config is
self-sufficient:

- `cargo build --workspace` → **Finished** (ring + all crates + node).
- `cargo test -p lb-host --test proof_panel_test` → **9 passed; 0 failed**, including
  the mandatory capability-deny (`*_denied_without_the_grant`) and
  workspace-isolation (`workflow_surface_is_workspace_isolated`,
  `workspace_isolation_series_and_ping`) tests.

## Prevention

- If you later install a real gcc/clang (e.g. native `.deb` toolchain with root), you
  can delete the `linker`/`[env]` block to fall back to the system compiler.
- Do **not** reach for the Flatpak SDK gcc — it segfaults outside its sandbox.
- Related: [build/host-freezes-during-rust-build](host-freezes-during-rust-build.md)
  (same box, the mold/debug-trim build tuning; its mold-driver note is now superseded
  by the zig linker for the no-`cc` case).
