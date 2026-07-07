# Desktop scope — package the Tauri shell as a plain executable (Linux + Windows)

Status: scope (the ask). Promotes to `public/desktop/desktop.md` once shipped.

The Tauri v2 shell (`ui/src-tauri`, crate `lazybones-shell`) already exists, compiles headlessly,
and is unit-tested — but nobody has ever produced the actual **windowed desktop executable**
(`docs/STATUS.md` lists "the native desktop window (webkit toolchain)" as un-built). This scope
ships that packaging step: a **plain executable** — *not* an AppImage/deb/MSI — for
**Linux x86-64** and **Windows x86-64**, built repeatably (CI), so the `workstation` persona
(README §5) stops being theoretical. The shell IS a node with a window attached (symmetric nodes,
§3.1); this scope adds **zero product code** — it is toolchain, build wiring, and proof.

## Goals

- A repeatable build that emits `lazybones-shell` (Linux ELF) and `lazybones-shell.exe`
  (Windows PE) as **bare binaries** — `tauri build --no-bundle` with the `desktop` cargo
  feature enabled — plus a boot smoke-proof that each binary opens the window and renders the
  real UI against its in-process node.
- CI packaging lane: a GitHub Actions matrix (`ubuntu-latest` + `windows-latest`) that builds
  both and uploads them as artifacts. Local one-liner documented for each OS.
- The `desktop` feature flows through the whole chain (`pnpm tauri build` → cargo) without the
  headless default breaking: `cargo test -p lazybones-shell` (no feature, no webkit) stays green.
- Document the runtime contract honestly: Linux binary **dynamically links webkit2gtk-4.1**
  (target machines must have it — that is the trade of "no AppImage" and we accept it);
  Windows uses the OS-provided WebView2, so the exe is genuinely standalone on Win10/11.

## Non-goals

- **Installers/bundles** (AppImage, deb, rpm, MSI, NSIS) — explicitly out; the ask is bare
  executables. `tauri.conf.json`'s `bundle.targets: "all"` gets bypassed with `--no-bundle`
  (flipping the config is an open question below).
- **snap — rejected outright, not just deferred.** It requires `snapd` on the host (a
  runtime dep the bare-binary contract avoids), drags Canonical store / auto-update
  coupling, and is widely disliked on Linux desktops. Ubuntu is supported first-class via
  the bare executable (run directly) and the future `.deb` (Ubuntu is deb-based); snap
  adds nothing those two don't already cover. AppImage stays the self-contained option.
- **macOS / ARM targets** — the axis is recorded in `platform-targets/platform-targets-scope.md`;
  this slice proves `linux-x86-64` + `windows-x86-64` only.
- **Cross-compiling Windows from Linux** (`cargo-xwin`/mingw). Rejected for this slice: the shell
  embeds the full node in-process (`lb-host` → SurrealDB, Zenoh, wasmtime — a heavy native tree),
  so per-crate cross-target fights are likely; a `windows-latest` runner costs nothing. Revisit
  only if CI-less offline Windows builds become a requirement.
- **Closing the desktop command-layer gap.** The Tauri IPC layer mirrors only a subset of verbs
  (channels etc.); `agent_invoke`, `assets_*`, `workflow_*`, `registry_*`, and the desktop session
  (workspace switcher) are tracked in `docs/STATUS.md` "next up" — separate ask, separate scope.
  This slice ships the *packaging*; the packaged app is knowingly behind the browser UI.
- Auto-update, code-signing certificates (Windows SmartScreen), release channels — follow-ups.

## Intent / approach

Everything hard was already designed for: `tauri` is an **optional dependency behind the
`desktop` cargo feature** (`ui/src-tauri/Cargo.toml`) precisely so headless CI never needs the
webkit toolchain, and `tauri build` always produces the bare binary *before* bundling — so
"plain executable" is just `--no-bundle` on a machine with the right toolchain. The slice is:

1. **Linux toolchain** — a documented apt set (`pkg-config`, `libwebkit2gtk-4.1-dev`,
   `libgtk-3-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `librsvg2-dev`,
   `build-essential`) + the build command. Note: the sandboxed dev machine links via zig
   (`rust/.cargo/config.toml`) and cannot install these — the windowed build runs on a real
   dev box or CI, as `ui/src-tauri/Cargo.toml` already anticipates.
2. **Feature plumbing** — make the `desktop` feature the packaging default without touching the
   headless path: a `pnpm` script (e.g. `tauri:build`) that runs
   `tauri build --no-bundle -- --features desktop`, so nobody has to remember the flag.
3. **Windows lane** — native build on `windows-latest` (MSVC + VS Build Tools + pnpm), same
   command. WebView2 is OS-provided; no runtime deps to ship.
4. **CI workflow** — one `desktop-build.yml` matrix job; artifacts named
   `lazybones-shell-linux-x86_64` / `lazybones-shell-windows-x86_64.exe`. Build-only gate at
   first; the smoke test (below) joins it once stable.
5. **Smoke proof** — Linux: launch the binary under `xvfb-run`, assert the window process stays
   up and the in-process node answers an IPC command (real store, real bus — rule 9). Windows:
   launch + liveness check. Depth is an open question below.

Alternative rejected: keeping `bundle.targets: "all"` and shipping the AppImage as "the Linux
build". The ask is a bare executable; AppImage also drags in `linuxdeploy` tooling and larger CI
time for an artifact nobody asked for.

## How it fits the core

- **Symmetric nodes:** this is the strongest proof of rule 1 — the *same* crates, one more binary
  persona (`workstation` = node + window). No `if desktop` in core crates; the only switch is the
  `desktop` cargo feature in the shell crate itself, which gates *window attachment*, not behavior.
- **Placement:** local-only by definition (edge `workstation`). The binary embeds the node; it
  talks to no cloud unless the node's own config says so.
- **Tenancy / capabilities / MCP / data / bus / sync / secrets:** N/A as *new* surface — the shell
  reuses the node's existing wall verbatim; packaging adds no verbs, records, subjects, or grants.
  (The one adjacent item: desktop secrets are meant to use the OS keychain via `keyring`, README
  §6 — already scoped elsewhere, unaffected by packaging.)
- **No mocks:** the smoke test boots the real binary with its real embedded store/bus. Nothing to
  fake; there is no external here.
- **SDK/WIT impact:** none. **One responsibility per file:** CI workflow, pnpm script, and docs
  are each their own file; no shell code changes expected.

## Example flow

1. Developer on Linux runs `cd ui && pnpm tauri:build` (deps installed per the doc).
2. Vite builds `ui/dist`; cargo builds `lazybones-shell` with `--features desktop`; no bundler runs.
3. `ui/src-tauri/target/release/lazybones-shell` exists; `./lazybones-shell` opens the Lazybones
   window; the UI talks to the in-process node over Tauri IPC; data lands in the local SurrealDB.
4. CI does the same on both matrix legs and uploads the two binaries as workflow artifacts.
5. A Windows user downloads `lazybones-shell-windows-x86_64.exe` and double-clicks it — no
   installer, WebView2 already on the OS.

## Testing plan

Per `scope/testing/testing-scope.md`: capability-deny and workspace-isolation are already covered
by the shell's existing headless command-layer tests (`cargo test -p lazybones-shell`) — this
slice must keep them green **feature-off**, proving the optional-dep seam still holds. New:

- **Build gates (the real deliverable):** both CI matrix legs produce a binary; failure blocks.
- **Boot smoke (Linux):** `xvfb-run ./lazybones-shell` + assert process alive after N seconds and
  one real IPC round-trip against the embedded node. Real store (`mem://` or a temp dir), rule 9.
- **Boot smoke (Windows):** launch + liveness; IPC round-trip if achievable headlessly (open Q).
- **Regression:** plain `cargo build -p lazybones-shell` (no feature) on a machine *without*
  webkit must still succeed — the property that keeps every other CI lane webkit-free.

## Risks & hard problems

- **The heavy in-process node meets a second platform.** SurrealDB/Zenoh/wasmtime have never been
  compiled for `x86_64-pc-windows-msvc` in this repo; expect some crate/feature friction on the
  first Windows build. This — not Tauri — is the likely time sink.
- **Linux runtime-dep drift.** webkit2gtk **4.1** is the Tauri v2 requirement; older distros ship
  4.0 only. The doc must state the minimum distro line (e.g. Ubuntu 22.04+) plainly.
- **`beforeBuildCommand` coupling:** `pnpm build` must produce the same `dist/` the gateway serves;
  any env-specific base-path assumptions in the Vite build surface here for the first time.
- **Windows SmartScreen** will warn on an unsigned exe — acceptable for this slice, but say so in
  the doc rather than letting it read as a bug.

## Open questions

- Flip `tauri.conf.json` `bundle.targets` from `"all"` to `[]` (making `--no-bundle` the config
  default) or keep the config as-is and rely on the flag? Leaning: flag only, so a future
  installer slice doesn't have to re-edit config.
- Windows smoke depth: is a headless IPC round-trip feasible on `windows-latest`, or is
  launch-liveness the honest first gate?
- Where do CI artifacts graduate to — GitHub Releases on tag, or workflow artifacts only for now?
- Does the first windowed boot expose the "shell fixes its workspace" limitation badly enough to
  pull the desktop-session scope forward? (Decide after the first smoke run, not before.)

## Skill doc

Yes — this is an automatable, drivable task. The implementing session writes
`docs/skills/desktop-build/SKILL.md` (grounded in a live build on at least the Linux leg): deps,
the one-liner per OS, where the binary lands, and the smoke command.

## Related

- `ui/src-tauri/Cargo.toml` — the `desktop` optional-feature seam this scope activates.
- `docs/STATUS.md` — "native desktop window (webkit toolchain)" un-built item; the Tauri
  command-layer gap ("next up" items) this scope deliberately does not close.
- `docs/scope/platform-targets/platform-targets-scope.md` — the OS/arch axis; this slice proves
  two targets on it.
- `docs/scope/node-roles/node-roles-scope.md` + README §5 — the `workstation` persona.
- README §6.13 (Frontend — UI delivery: Tauri-local vs browser-remote), §3 rule 1 (symmetric nodes).
- `docs/public/desktop/desktop.md` — the public doc this promotes to.
