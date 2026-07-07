# build/linux/deb/

The `.deb` packaging of `lazybones-shell` for Debian/Ubuntu (and deb-based distros).
**Future** — the first desktop slice ships only the
[bare executable](../../../docs/linux/README.md); deb/rpm/AppImage are scoped as
follow-ups in
[`../../../../docs/scope/desktop/desktop-packaging-scope.md`](../../../../docs/scope/desktop/desktop-packaging-scope.md)
under "Non-goals".

## Why deferred

The scope deliberately ships a plain ELF — `tauri build --no-bundle` — to prove the
windowed build end-to-end with minimum toolchain friction. Bundling adds
`dpkg-deb`/`dpkg-buildpackage` tooling, a control file, maintainer scripts, desktop
entry, icon install, mime-type registration, and CI surface — none of it product code,
all of it pay-only-when-you-need-it. Cut this dir when a real user asks for
`apt install lazybones`.

## What goes here when wired

- `control/` — `control`, `postinst`, `prerm`, `postrm`, `conffiles`, the
  `debian/changelog` source. Depends must list the webkit2gtk-4.1 runtime set
  (`libwebkit2gtk-4.1-0`, `libgtk-3-0`, `libsoup-3.0-1`, `libjavascriptcoregtk-4.1-0`,
  `librsvg2-2`) — the same set the bare-binary runtime contract names.
- `lazybones-shell.desktop` — the XDG desktop entry (Name, Exec, Icon, Categories,
  StartupWMClass, MimeType).
- `icons/` — PNG/SVG at the standard XDG sizes (`/usr/share/icons/hicolor/…`).
- `build-deb.sh` — one script: `tauri build --bundles deb -- --features desktop` (or
  flip `tauri.conf.json` `bundle.targets: ["deb"]` for this job) + rename the artifact
  to `lazybones-shell_<version>_amd64.deb`.

## Arch

One deb per arch. `amd64` (=`x86_64`) for this slice; rebuild for `arm64` when that
target lands (per
[`../../../../docs/scope/platform-targets/platform-targets-scope.md`](../../../../docs/scope/platform-targets/platform-targets-scope.md)).
Tauri emits `lazybones-shell_<version>_<arch>.deb`; the arch suffix maps Rust triples to
Debian arches (`x86_64-unknown-linux-gnu` → `amd64`).

## Minimum distro line

webkit2gtk **4.1** ships from **Ubuntu 22.04 LTS / Debian 12** onward. State that as
the `Depends` floor and in the release notes; older distros only have 4.0 and will not
install cleanly.

Status: **empty stub** — no control files, no build script yet. Wired when the
installer slice lands.
