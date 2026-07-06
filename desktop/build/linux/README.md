# build/linux/

Per-build-type home for the Linux desktop binary. One subdir per **build type**
(executable, deb, rpm, AppImage, …); each holds whatever that type needs — config files
and maintainer scripts for installer types, the built ELF for the bare-executable type —
plus a README explaining what lands there.

The bare-executable build is wired now: `make linux-executable` builds the ELF in the
container and copies it to [`executable/lazybones-shell`](executable/). The installer
types (deb/rpm/AppImage) are future work — their dirs hold control files + a build script
when their slices land.

## Arch handling

A native binary is built for one target triple; an installer wraps that one binary. So
**arch is a property of the build run, not a directory level** — `build/linux/deb/`
produces `lazybones-shell_<version>_amd64.deb` for `x86_64` today and would be run
again for `arm64`/`aarch64` when that target ships (see
[`../../../docs/scope/platform-targets/platform-targets-scope.md`](../../../docs/scope/platform-targets/platform-targets-scope.md)
— the OS/arch axis; this slice proves `linux-x86-64` only). If a fat/multi-arch
installer ever becomes worth it, that decision lands in the scope first, not here.

## Subdirs

| Dir | Status | Produces |
| --- | --- | --- |
| [`executable/`](executable/) | **active** | the bare ELF `lazybones-shell` (+ CI tarball) |
| [`deb/`](deb/) | future | `.deb` package (Debian/Ubuntu) |
| `rpm/` | not yet created | `.rpm` package (Fedora/RHEL/SUSE) |
| `appimage/` | not yet created | `.AppImage` (self-contained, no runtime deps) |
| `snap/` | **rejected — will not build** | n/a |

**snap is a non-goal.** It requires `snapd` on the host (a runtime dep the bare-binary
contract deliberately avoids), drags in Canonical store / auto-update coupling, and is
widely disliked on Linux desktops. **Ubuntu is supported first-class via the bare
executable (run directly) and the `.deb`** — snap adds nothing those two don't cover.
AppImage remains the self-contained option if a no-runtime-deps format is later wanted.

## When adding a build type

1. Cut a subdir named after the type (`deb/`, `rpm/`, `appimage/`, …).
2. Put the type's control files + a single build script in it — folder-of-verbs over a
   monolithic script (see [`../../../docs/FILE-LAYOUT.md`](../../../docs/FILE-LAYOUT.md)).
3. Flip `tauri.conf.json` `bundle.targets` (or pass the right `--bundles <type>` flag)
   for that one build job; the open question of config-vs-flag is tracked in the scope.
4. Add a row to the table in [`../../docs/linux/README.md`](../../docs/linux/README.md)
   and a smoke test alongside the build.

Status: executable type active (binary lands in [`executable/`](executable/)); the
installer types are scaffolded for when their slices land.
