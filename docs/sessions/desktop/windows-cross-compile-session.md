# Session: Windows cross-compile from Docker

**Branch:** ext-devkit-updates · **Date:** 2026-07-08

## Ask

Get the Windows desktop binary building from Docker on Linux. Output lands in
`desktop/build/windows/`, with the demo SQLite seed file alongside. Plain
executable (no installer/AppImage-equivalent) for now.

## What shipped

- `desktop/docker/Dockerfile` — new `windows` stage: `base` + clang/lld/nasm/cmake,
  `x86_64-pc-windows-msvc` rust target, `cargo-xwin` (fetches Windows SDK/CRT into
  `XWIN_CACHE_DIR=/usr/local/xwin`, cached via the `lb-xwin` volume).
- `desktop/docker/build-windows.sh` — the cross-build entrypoint, mirroring
  `build.sh`: `pnpm tauri build --no-bundle --runner cargo-xwin --target
  x86_64-pc-windows-msvc -- --features desktop`.
- `desktop/Makefile` — `make windows-image`, `make windows-executable` (binary +
  always-seeded `demo-buildings.db` → `desktop/build/windows/`); `clean` now also
  drops `lb-xwin`.
- `ui/src-tauri/src/main.rs` — `windows_subsystem = "windows"` in release so no
  console window opens behind the app.
- `ui/src-tauri/src/state.rs` — fixed `Claims` initializer drift (new
  `constraint`/`run_id` fields → `None`); was breaking every shell build, not
  just Windows.
- `desktop/docs/windows/README.md` — the build doc (was an empty stub).

## Decisions

- **msvc over gnu triple**: `x86_64-pc-windows-msvc` via cargo-xwin rather than
  mingw — it's the ABI Windows users run, and the shell's only C deps
  (`ring`, `zstd-sys`; SurrealKV is pure Rust, no rocksdb/openssl) build cleanly
  under clang-cl. Rejected mingw: subtly different ABI, and Tauri's documented
  Linux→Windows cross path is cargo-xwin.
- **No installer yet**: `--no-bundle` mirrors the Linux bare-binary slice. MSI/NSIS
  needs nsis+wine in the image — a later slice.
- **Seed always generated** for the Windows lane (no `SEED_DEMO` flag): the ask was
  "make sure the seed sqlite is in there".

## Verified

- `make windows-executable` green end to end: `desktop/build/windows/lazybones-shell.exe`
  is `PE32+ executable (GUI) x86-64, for MS Windows` (113 MB) +
  `demo-buildings.db` (134 MB, lite profile).
- Headless `cargo test -p lazybones-shell` green after the `Claims` fix.
- Runtime boot on a real Windows 10/11 box (WebView2) is the remaining manual check —
  the container can't smoke a PE binary.

## Debugging entries logged

- `docs/debugging/desktop/lb-cargo-volume-permission-denied.md`
- `docs/debugging/desktop/shell-claims-initializer-drift.md`
