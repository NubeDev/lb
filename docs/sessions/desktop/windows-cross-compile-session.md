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

---

## Follow-up (2026-07-08): `windows-full` — the FULL standalone .exe cross-builds

**Ask:** get the *full standalone* Windows desktop executable building on Linux (in
Docker) first, then Windows. Same intent as the thin lane above, but with
`LB_SHELL_FEATURES=desktop,full` — the packaged `.exe` boots the node, runs the boot
seeders, and mounts the SSE/HTTP gateway in-process on `127.0.0.1:8800`, so it is a
100% standalone node (login, MCP, SSE, agents, flows, insights) with no external node.

**What ran (no code changes needed — the lane already existed):**

- `make -C desktop windows-image` — built the `windows` cross toolchain image
  (`lazybones-shell-builder-windows:local`); cargo-xwin installed clean.
- `make -C desktop windows-full` — cross-built `--features desktop,full` for
  `x86_64-pc-windows-msvc`. First run populated the `lb-xwin` (MSVC CRT/SDK) and
  `lb-cargo` caches and was cut short by a session boundary — but got *past* every C
  dep and SurrealDB. The warm-cache re-run finished the whole node stack + the final
  `lld-link` in **3m 15s**.

**Verified:**

- `desktop/build/windows-full/lazybones-shell.exe` = `PE32+ executable (GUI) x86-64,
  for MS Windows, 10 sections`, **143 MB**.
- The `full`-only deps (`lb-role-gateway`, `lb-authz`) and the whole node stack
  (surrealdb-core/surrealkv, zenoh, wasmtime, polars, cedar) all cross-compiled to
  MSVC cleanly; the Windows webview layer (`webview2-com`, `tao`, `wry`,
  `tauri-runtime-wry`) linked without error.
- Build output ends with the expected banner: *"Binary (full): …/lazybones-shell.exe
  — login at http://127.0.0.1:8800 as user:ada / acme"*.

**Linux full — built AND proven working (the "get it working in Linux first" ask):**

- `make -C desktop linux-full` → `desktop/build/linux/full/lazybones-shell`, `ELF
  64-bit x86-64 pie`, 168 MB. Finished + linked in 2m 17s.
- `make -C desktop smoke-full` = **the actual proof it works** (not just that it
  compiles): boots the binary under `xvfb-run`, then `curl`s a real
  `POST /login`. Output:
  - `full: seeded 38 core skills @0.1.0`
  - `full: loopback gateway on http://127.0.0.1:8800 (login as user:ada / acme)`
  - `login OK over loopback gateway (http://127.0.0.1:8800)` ← real token returned.
  So the standalone node boots, seeds, mounts the in-process gateway, and answers a
  real client end to end — on Linux, no external node.

**Remaining (recorded, not a gap):** runtime boot of the *Windows* `.exe` on a real
Windows 10/11 box (OS-provided WebView2) — login over the loopback gateway — is the one
manual check the Linux container can't perform on a PE binary. The Linux full binary is
the runnable proof of the identical code path (symmetric nodes); only the runtime
webview differs.

**Decision:** no source or build-script change was required for `full` on Windows —
the `windows-full` Makefile target + `build-windows.sh`'s feature passthrough already
covered it. This confirms the symmetric-node claim (§3.1) holds across the OS boundary:
the *only* Windows delta remains the runtime webview (OS WebView2 vs `webkit2gtk-4.1`),
exactly as `desktop/build/windows-full/README.md` states.

## Follow-up (2026-07-08): demo seed DB wired into the `full` lanes

**Gap found:** `seed-demo` (generates `demo-buildings.db` via `docker/postgres/seed.py
--sqlite`) was only wired into `linux-executable` (opt-in `SEED_DEMO=1`) and
`windows-executable` (unconditional). `linux-full` and `windows-full` shipped the binary
with no demo dataset alongside it.

**Fix:** `desktop/Makefile` — `linux-full` and `windows-full` now call `seed-demo`
unconditionally (matching `windows-executable`'s behavior), writing
`demo-buildings.db` into the same output dir as the binary
(`build/linux/full/`, `build/windows-full/`).

**Verified:**
- `make -C desktop linux-full` → `build/linux/full/{lazybones-shell,demo-buildings.db}` (134 MB db).
- `make -C desktop windows-full` → `build/windows-full/{lazybones-shell.exe,demo-buildings.db}` (134 MB db).
- Re-ran `make -C desktop smoke-full` after the change — still green (login OK over
  loopback gateway), confirming the seed-file addition didn't disturb the boot path.

**Note:** `seed-demo` only *generates the file* — it does not register it as a live
datasource (that needs `datasource.add` against a running gateway, which `full` binaries
uniquely have). `docker/postgres/seed-demo-sqlite.sh` does both (generate + register) but
targets `make dev`'s gateway (`127.0.0.1:8080`) by default; pointed at a running `full`
binary's loopback gateway (`127.0.0.1:8800`) it would register the same file. Left as a
manual runtime step, not part of the build — registration needs the binary to be running,
which the build container doesn't do (mirrors why `smoke-full`, not the build target
itself, is what proves login).
