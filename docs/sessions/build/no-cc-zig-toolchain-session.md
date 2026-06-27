# Session: get the Rust workspace building on a box with no C compiler

- Date: 2026-06-27
- Area: build / dev-environment
- Outcome: resolved — `cargo build --workspace` and tests green via a self-contained zig toolchain.

## The ask

"Fix this" — Rust builds were failing while the node ran fine on :8080. The user
saw "AI compiling" failing.

## What was wrong

Not the code. Every crate compiled; the build died only at the **link** step:
`error: linker 'cc' not found`. This box has no system C compiler (`cc`/gcc/clang),
no `zig`/`tcc`, and **no root** to `apt install` one. On top of that, `ring 0.17`
uses `cc-rs` to compile C+asm, so a linker-only workaround is insufficient — a real
C compiler is required.

A leftover Flatpak Freedesktop SDK gcc exists (from a since-removed VSCode flatpak)
but **segfaults** outside the flatpak sandbox.

## What I did

1. Confirmed the diagnosis: rustc compiled everything, only linking failed;
   a `rust-lld` linker shim built most of the workspace but `ring` needed a real
   C compiler.
2. Asked the user how to get a compiler (no root); chose **install zig (no root)**.
3. Downloaded zig 0.13.0 to `~/.local/zig-linux-x86_64-0.13.0`; created
   `~/.local/bin/zigcc` (rewrites the `x86_64-unknown-linux-gnu` triple to zig's
   `x86_64-linux-gnu`) and `~/.local/bin/zigar`.
4. Wired both into `rust/.cargo/config.toml` (`linker` + `[env]` CC/AR), removed the
   now-defunct mold `-fuse-ld=mold` rustflag.

## Tests (per testing-scope)

Ran with a clean env to prove the config is self-sufficient:

- `cargo build --workspace` → Finished.
- `cargo test -p lb-host --test proof_panel_test` → **9 passed; 0 failed**, including
  the mandatory capability-deny and workspace-isolation tests.

## Debugging log

- [build/no-c-compiler-linker-cc-not-found.md](../../debugging/build/no-c-compiler-linker-cc-not-found.md)

## Follow-ups

- If a native gcc/clang is installed later (with root), the `linker`/`[env]` block in
  `rust/.cargo/config.toml` can be dropped to use the system toolchain.
