# build/linux/

Per-build-type packaging configs for the Linux desktop executable. One subdir per
**build type** (deb, rpm, AppImage, …); each holds the control files, maintainer
scripts, icons, desktop entries, and a build script that type needs.

The **plain executable** has no dir here — it is the bare
`tauri build --no-bundle -- --features desktop` run from
[`../../../ui/`](../../../ui/) (documented in
[`../../docs/linux/README.md`](../../docs/linux/README.md)). This dir is only for the
**installer/bundle types** the bare-binary slice skips.

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

Status: scaffolded only — no build configs yet. The bare-executable slice (the scope's
current ask) does not need anything here.
