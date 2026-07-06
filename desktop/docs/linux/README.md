# Linux desktop build

The runnable distillation of
[`../../../docs/scope/desktop/desktop-packaging-scope.md`](../../../docs/scope/desktop/desktop-packaging-scope.md)
for Linux. This slice ships the **plain executable** (`linux-x86-64`); installer types
(deb/rpm/AppImage) come later — see
[`../../build/linux/`](../../build/linux/).

The shell is the `lazybones-shell` crate at
[`../../../ui/src-tauri/`](../../../ui/src-tauri). `tauri` is an **optional cargo
dependency behind the `desktop` feature** (`Cargo.toml`), so the headless command-layer
builds/tests without the webkit toolchain; the windowed build turns the feature on.

## Prerequisites (the webkit2gtk-4.1 toolchain)

Tauri v2 requires **webkit2gtk 4.1** (not 4.0). On Debian/Ubuntu (22.04+ — older
distros ship 4.0 only and will not build):

```bash
sudo apt-get install \
  build-essential \
  pkg-config \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  librsvg2-dev
```

Equivalent package names on Fedora / Arch / etc. — match by soname
(`libwebkit2gtk-4.1`, `libsoup-3.0`, `libjavascriptcoregtk-4.1`). A working `cc`
toolchain is required (the sandboxed dev box links via `zig` per
`rust/.cargo/config.toml` and **cannot install these system packages** — the windowed
build runs on a real dev box or CI; see scope "Intent / approach" §1).

## Build the executable

From `ui/`:

```bash
cd ui
pnpm tauri build --no-bundle -- --features desktop
```

- `pnpm build` runs first (the `beforeBuildCommand` in `tauri.conf.json`) → `ui/dist/`.
- cargo builds `lazybones-shell` with `--features desktop` → `tauri-build` codegen runs
  (`ui/src-tauri/build.rs` gates on `CARGO_FEATURE_DESKTOP`).
- `--no-bundle` skips every installer type (the config still says `bundle.targets: "all"`
  by design — see the scope's open question on whether to flip it); only the bare ELF is
  produced.

**Output:** [`../../../ui/src-tauri/target/release/lazybones-shell`](../../../ui/src-tauri/target/release/lazybones-shell)
— a dynamically-linked ELF for `x86_64-unknown-linux-gnu`.

## Runtime contract

- The binary **dynamically links `webkit2gtk-4.1`** and friends — the target machine
  must have the runtime libraries installed (`libwebkit2gtk-4.1-0`, `libgtk-3-0`,
  `libsoup-3.0-1`, `libjavascriptcoregtk-4.1-0`, `librsvg2-2`). This is the trade of
  "no AppImage": smaller, simpler CI, but the user's distro provides the webview. State
  the minimum distro line (Ubuntu 22.04+, Debian 12+, …) wherever you ship the binary.
- It is a **node** — SurrealDB (`mem://` or a config path) and Zenoh boot in-process
  (`ui/src-tauri/src/desktop.rs`). No external services required for a single-window
  session.

## Smoke test

```bash
xvfb-run -a ./ui/src-tauri/target/release/lazybones-shell
# in another terminal / test harness: assert the process stays up for N seconds
# and an IPC round-trip (channel_post → channel_history) against the in-process node.
```

`xvfb-run` because the binary opens a window — there is no display in CI. The smoke
uses the **real** store + bus (rule 9 — no mocks); the only "fake" thing here is the
virtual display. See the scope's testing plan.

## Verify the headless path is untouched

The whole point of the `desktop` feature seam is that every other CI lane stays
webkit-free. On a machine *without* the webkit toolchain this must still succeed:

```bash
cd ui/src-tauri && cargo build -p lazybones-shell            # no feature
cd ui/src-tauri && cargo test  -p lazybones-shell            # command-layer unit tests
```

## Build types status

| Type | Status | Where |
| --- | --- | --- |
| **executable** (bare ELF) | **this slice** | `pnpm tauri build --no-bundle -- --features desktop` from `ui/` |
| `.deb` | future (covers Ubuntu) | [`../../build/linux/deb/`](../../build/linux/deb/) |
| `.rpm` | future | `../../build/linux/rpm/` (not yet created) |
| AppImage | future | `../../build/linux/appimage/` (not yet created) |
| snap | **rejected — will not build** | n/a |

**No snap.** It needs `snapd` on the host (a runtime dep the bare-binary contract avoids)
and drags Canonical store coupling. Ubuntu runs the bare executable directly and will
`apt install` the `.deb` — both first-class, neither snap. See
[`../../build/linux/README.md`](../../build/linux/README.md).

Each installer type will live as its own dir under
[`../../build/linux/`](../../build/linux/) — control files, maintainer scripts, desktop
entry, icons, and a build script — and flip `--no-bundle` off for that one job.

## Related

- Scope: [`../../../docs/scope/desktop/desktop-packaging-scope.md`](../../../docs/scope/desktop/desktop-packaging-scope.md)
- Shell crate: [`../../../ui/src-tauri/`](../../../ui/src-tauri) (`Cargo.toml`,
  `tauri.conf.json`, `src/lib.rs` command layer, `src/desktop.rs` window wiring).
- Platform-targets axis: [`../../../docs/scope/platform-targets/platform-targets-scope.md`](../../../docs/scope/platform-targets/platform-targets-scope.md).
- Status: [`../../../docs/STATUS.md`](../../../docs/STATUS.md) — "native desktop window
  (webkit toolchain)" un-built item.
