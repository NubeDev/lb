# app — get the RN shell running on a real Android device (session)

- Date: 2026-07-04
- Scope: continuation of [app-expo-scope.md](../../scope/app/app-expo-scope.md) (on-device
  Android run — the piece the Expo-bare session explicitly deferred as "no toolchain here")
- Stage: post-S10 app track — first real device build/run of the shell
- Status: **done** — `make android` is green end-to-end; app runs on a physical device
  (`SM-A566B` / `R5GYB2T2GYH`) against Metro, no JS crash
- Debugging: [build/android-run-repack-flow-enum-and-missing-deps.md](../../debugging/build/android-run-repack-flow-enum-and-missing-deps.md)

## Goal

The developer had an Android device connected and `make android` failing. Get the RN
0.86 + Re.Pack shell building, bundling, and running on the device, and make `make
android` work without hand-editing a shell profile.

## What was wrong (4 layers, each masking the next)

The Gradle build + APK install **always succeeded** — every failure was after install:

1. **`adb: not found` / `SDK location not found … ANDROID_HOME`.** SDK is at
   `~/Android/Sdk` (Gradle finds it via `android/local.properties` `sdk.dir`), but
   `platform-tools` was never on PATH, so the RN CLI's `adb reverse` + `am start` failed.
2. **Metro won't boot:** `@module-federation/enhanced` (peer of Re.Pack's MF2 plugin)
   not installed.
3. **Bundle fails:** `@swc/helpers/_/_interop_require_wildcard` unresolved (needed by
   `react-native-screens`' pre-compiled source).
4. **Bundle fails:** SWC syntax error on `export enum VirtualViewMode` in RN 0.86's
   `VirtualView.js`. Re.Pack's `flow-loader` (`flow-remove-types` 2.321.0) strips Flow
   **types** but leaves Flow **enums** intact; SWC then chokes. Metro survives this only
   via `@react-native/babel-preset`'s `babel-plugin-transform-flow-enums`.

## What shipped

1. **`app/Makefile`** — `ANDROID_HOME ?= $(or $(ANDROID_SDK_ROOT),$(HOME)/Android/Sdk)`;
   the `android` target asserts `$(ANDROID_HOME)/platform-tools/adb` exists (clear error
   if not) and runs `pnpm android` with `ANDROID_HOME` + `platform-tools`/`emulator`
   prepended to PATH. Works with no `.bashrc` edit; override via
   `make android ANDROID_HOME=/path`. (The `.bashrc` route was intentionally avoided —
   the project file is the durable, reviewable fix.)
2. **Two deps** into the standalone shell (per app-shell-standalone-pnpm-policy):
   `@module-federation/enhanced@0.22.0` (match locked MF runtime) and `@swc/helpers`,
   both via `pnpm add --ignore-workspace --config.minimumReleaseAge=0` — the age flag is
   required because `--ignore-workspace` also drops the shell's own
   `pnpm-workspace.yaml` `minimumReleaseAge:0`, so fresh Expo-57 lockfile entries would
   otherwise fail the supply-chain policy.
3. **`app/shell/rspack.config.mjs`** — a Babel `pre`-loader scoped to
   `react-native` + `@react-native` (`Repack.getModulePaths(...)`) running
   `babel-plugin-syntax-hermes-parser` + `babel-plugin-transform-flow-enums` before
   flow-loader/SWC, lowering Flow enums to `flow-enums-runtime` calls. All plugins were
   already installed transitively via `@react-native/babel-preset`.

## Verification (real, on device)

- `make android` → **BUILD SUCCESSFUL**, `adb reverse` + `am start` run, **exit 0**.
- App launches on `SM-A566B`; process alive (pid observed), no `FATAL` /
  `AndroidRuntime` / `ReactNativeJS` error in `adb logcat`; app issues gateway DNS
  requests (JS bundle loaded and running).
- Android bundle compiles clean: `curl 'localhost:8081/index.bundle?platform=android'`
  → HTTP 200, ~8.1 MB, no `Syntax Error` / `enum VirtualViewMode` in output.

## Notes / follow-ups

- Non-fatal ⚠ during bundling: `Can't resolve '@react-native-masked-view/masked-view'`
  — an **optional** peer of `@react-navigation/elements`, only used if installed. Add
  only if a screen needs masked headers.
- No core Rust / gateway / extension change; no core UI-shell change. Pure app-shell
  toolchain + one Makefile target.
- Rules touched: none violated — this is bundler/build config, not a core mediation
  seam or an extension branch.
