# build/linux/executable/

The **bare-executable** build type — the plain ELF `lazybones-shell`, no installer wrapper.
This is the current Linux desktop deliverable (the packaging scope's first slice).

`make -C desktop linux-executable` builds the binary in the container and copies it here:

```
desktop/build/linux/executable/lazybones-shell      ← the ELF (132 MB, dynamically linked)
desktop/build/linux/executable/lazybones-shell-linux-x86_64.tar   ← CI tarball (`make artifact`)
desktop/build/linux/executable/demo-buildings.db    ← demo dataset (only with SEED_DEMO=1)
```

All three are gitignored — regenerated per build, never committed. Only this README is tracked.

## Demo dataset (`SEED_DEMO=1`)

`make -C desktop linux-executable SEED_DEMO=1` (or the standalone `make -C desktop seed-demo`)
runs `docker/postgres/seed.py --sqlite` to generate the demo-building dataset alongside the
binary: 8 sites, ~70 meters, ~330 points, ~1M readings (lite profile — 1 month @ 15-min,
~134 MB, ~90s). The file is what the federation sidecar's `sqlite` engine queries; the "DSN"
at registration time is this file path.

**Registration is a runtime step, not a build step.** This target only generates the file.
To register it as a `kind:"sqlite"` datasource a node can query, you need a running node with
the federation sidecar + an admin caller — today that's `make dev` + the root Makefile's
`make seed-demo-sqlite` (which curls `datasource.add` over HTTP). The desktop shell's
datasource/admin command layer is still being wired (STATUS.md "next up" #4); until it lands,
this file is ready-but-unregistered next to the binary.

## What lands here vs `ui/src-tauri/target/`

Cargo/tauri builds the raw ELF at `ui/src-tauri/target/release/lazybones-shell` (the
standard cargo output dir, gitignored). The Makefile **copies** it here so the canonical
"pick up the binary" path is stable and grouped with the other build types — you don't
hunt through cargo's `target/` tree. The copy is a one-way `cp`: the cargo original stays
in place so incremental builds stay warm.

## Run it

The binary needs the webkit2gtk-4.1 runtime libs on the host (Ubuntu 22.04+, Debian 12+):

```bash
./desktop/build/linux/executable/lazybones-shell
```

Or boot it headless in the container (no host deps): `make -C desktop smoke`.

## Runtime contract

Dynamically links `libwebkit2gtk-4.1.so.0`, `libgtk-3.so.0`, `libsoup-3.0.so.0`,
`libjavascriptcoregtk-4.1.so.0` — the target machine must have them. This is the "no
AppImage" trade: smaller, simpler CI, the user's distro provides the webview. State the
minimum distro line (Ubuntu 22.04+, Debian 12+) wherever you ship the binary.

## Future build types

When `.deb`/`.rpm`/`.AppImage` land, each gets its own sibling dir (`../deb/`, `../rpm/`,
`../appimage/`) holding the type's control files + build script, and its output lands in
that dir — same pattern as this one. See [`../README.md`](README.md).

## Related

- Build command + container: [`../../../docker/README.md`](../../docker/README.md)
- Linux build docs: [`../../../docs/linux/README.md`](../../docs/linux/README.md)
- Scope: [`../../../../docs/scope/desktop/desktop-packaging-scope.md`](../../../../docs/scope/desktop/desktop-packaging-scope.md)
