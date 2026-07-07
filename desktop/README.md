# desktop/

Packaging workspace for the **`lazybones-shell`** desktop executable — the Tauri v2
window attached to the in-process node (the `workstation` persona, `../README.md` §5).
The shell itself is built from `../ui/src-tauri/` (crate `lazybones-shell`); this dir
holds only **packaging** — per-OS build wiring, build-type configs, and docs. Zero
product code lives here.

One binary, one shell — symmetric nodes (`../README.md` §3, rule 1): the desktop
binary IS a node with a window attached, configured not branched. Packaging adds no
verbs, records, or capabilities; the packaged app reuses the node's existing wall
verbatim.

## Layout

```
desktop/
  build/
    linux/            # per-build-type packaging configs (deb/, rpm/, …)
      deb/            # .deb packaging (future — see its README)
  docs/
    linux/           # build + run docs for Linux (read this first)
    darwin/          # macOS (future)
    windows/         # Windows (future)
  docker/            # the Linux build container — toolchain + Dockerfile + entrypoint
  Makefile           # one-liner entrypoints: `make linux-executable`, `make artifact`, …
```

- **`build/<os>/<build-type>/`** — one dir per build type under each OS. The plain
  executable needs no dir of its own (it is the bare `tauri build --no-bundle`); each
  *installer/bundle* type gets its own dir (deb, rpm, AppImage, MSI, …) holding
  control files, maintainer scripts, icons, desktop entries, and a build script.
- **`docs/<os>/`** — the human-facing build/run docs per OS. The authoritative ask is
  [`../docs/scope/desktop/desktop-packaging-scope.md`](../docs/scope/desktop/desktop-packaging-scope.md);
  these docs are the runnable distillation of it.

## Current focus: the plain Linux executable

The first slice ships a **bare binary** for `linux-x86-64` — `tauri build --no-bundle`
with the `desktop` cargo feature on. No installer, no AppImage, no deb/rpm. Linux
dynamically links `webkit2gtk-4.1` (the runtime contract); the target machine must
have it — the honest trade of "no bundle", accepted in the scope.

Read [`docs/linux/README.md`](docs/linux/README.md) for the toolchain, the one-liner,
where the binary lands, and the smoke command. **The recommended build path is the
container** (`make linux-executable`, see [`docker/README.md`](docker/README.md)) — no
host webkit2gtk-4.1 install required, same binary, host stays clean.

## What's later

- **Other Linux build types:** `.deb`, `.rpm`, AppImage — `build/linux/<type>/`
  scaffolded now, content later. Each adds the installer tooling the bare-binary slice
  deliberately skips; flipping `tauri.conf.json` `bundle.targets` (or the `--no-bundle`
  flag) is the open question tracked in the scope.
- **darwin / windows:** the OS/arch axis is recorded in
  [`../docs/scope/platform-targets/platform-targets-scope.md`](../docs/scope/platform-targets/platform-targets-scope.md).
  Windows uses the OS-provided WebView2 (genuinely standalone on Win10/11); macOS needs
  the WebKit toolchain + notarization. `docs/windows/` and `docs/darwin/` are empty
  stubs until those slices land.

## Pointers

- **The shell crate:** [`../ui/src-tauri/`](../ui/src-tauri) — `Cargo.toml` (the
  `desktop` optional-feature seam), `tauri.conf.json`, `src/` (command layer + window).
- **The build container:** [`docker/`](docker) — `Dockerfile` + `build.sh` + README; the
  recommended build path. Scope: [`../docs/scope/desktop/desktop-build-container-scope.md`](../docs/scope/desktop/desktop-build-container-scope.md).
- **The ask (scope):** [`../docs/scope/desktop/desktop-packaging-scope.md`](../docs/scope/desktop/desktop-packaging-scope.md).
- **What shipped (public):** [`../docs/public/desktop/desktop.md`](../docs/public/desktop/desktop.md).
- **Status:** [`../docs/STATUS.md`](../docs/STATUS.md) — "native desktop window
  (webkit toolchain)" is listed as un-built; this workspace is the slice that builds it.

Status: **in progress** — Linux executable slice (architecture scope done, build wiring
not yet landed). When the first binary builds, replace this line with the version +
commit that produced it.
