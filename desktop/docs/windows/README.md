# Windows build — cross-compiled from Linux, in Docker

The Windows lane of the desktop packaging workspace. One command, from any Linux
host with Docker:

```sh
cd desktop && make windows-executable
```

Output — the canonical pick-up location for the Windows binary:

```
desktop/build/windows/
  lazybones-shell.exe    # PE32+ (GUI subsystem), x86_64-pc-windows-msvc
  demo-buildings.db      # the demo-building SQLite dataset (seed.py --sqlite, lite profile)
```

## How it works

- **Toolchain**: the `windows` stage of [`../../docker/Dockerfile`](../../docker/Dockerfile) —
  the shared `base` (Rust + Node + pnpm) plus clang/lld/nasm and
  [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin), which downloads the
  Windows SDK/CRT (cached in the `lb-xwin` Docker volume) and drives
  clang-cl + lld-link for the `x86_64-pc-windows-msvc` target. We chose msvc over
  the `-gnu` (mingw) triple: it is the ABI Windows users actually run, and the
  shell's only C deps (`ring`, `zstd-sys`) build cleanly under clang-cl.
- **Entrypoint**: [`../../docker/build-windows.sh`](../../docker/build-windows.sh) —
  `pnpm tauri build --no-bundle --runner cargo-xwin --target x86_64-pc-windows-msvc -- --features desktop`.
- **Plain executable, no installer**: `--no-bundle` mirrors the Linux bare-binary
  slice — no MSI/NSIS (those need wine/nsis in the image; a later slice). Unlike
  Linux there is no webkit runtime contract: Windows 10/11 provide **WebView2**,
  so the exe is genuinely standalone.
- **Seed data**: `make windows-executable` always generates `demo-buildings.db`
  alongside the exe (the generation half of `docker/postgres/seed-demo-sqlite.sh`;
  registering it as a datasource still happens at runtime against a live node).

## Notes

- The exe is built with `windows_subsystem = "windows"` in release, so no console
  window opens behind the app.
- Not smoke-testable in the container (no Windows runtime); boot it on a real
  Windows 10/11 machine.
- `make clean` also drops the `lb-xwin` SDK cache volume.
