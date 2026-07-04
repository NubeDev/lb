# `make android` fails three ways before the app runs (Re.Pack + RN 0.86)

- Area: build / app-shell (Android device run)
- Status: resolved
- First seen: 2026-07-04
- Session: ../../sessions/app/app-android-run-session.md
- Regression test: n/a (dev-environment + bundler-config; a break fails `make android`
  loudly at the same step. The Flow-enum rule is guarded by the fact that a bundle
  that hits `VirtualView.js` won't compile without it — the failure is its own check.)

## Symptom

`cd app && make android` with a real device connected. The Gradle build + APK install
**succeed**, but the run fails at one of three later steps (each surfaced only after
the previous was fixed):

1. `/bin/sh: 1: adb: not found` → `SDK location not found … ANDROID_HOME` — the RN CLI
   can't find `adb`/`emulator`.
2. Metro (Re.Pack/Rspack) won't boot:
   `[RepackModuleFederationPlugin] Dependency '@module-federation/enhanced' is required, but not found`.
3. Bundle fails: `Module not found: Can't resolve '@swc/helpers/_/_interop_require_wildcard'`
   (from `react-native-screens`).
4. Bundle fails: `× Expected ',', got 'ident'` on `export enum VirtualViewMode` in
   `react-native/src/private/components/virtualview/VirtualView.js` — SWC choking on a
   Flow `enum`.

## Root cause

- **(1) PATH.** The Android SDK is installed at `~/Android/Sdk` but `ANDROID_HOME` was
  never exported and `platform-tools` was never on PATH. Gradle finds the SDK via
  `android/local.properties` (`sdk.dir=…`, already present) so it *builds* — but the RN
  CLI shells out to `adb`/`emulator` by name for `adb reverse` + `am start`, which fail.
- **(2)/(3) missing deps.** `@module-federation/enhanced` is a peer of Re.Pack's MF2
  plugin and `@swc/helpers` is required by `react-native-screens`' pre-compiled source;
  neither was in the shell's standalone `node_modules`.
- **(4) Flow enums.** RN 0.86 core ships Flow `enum`s. Re.Pack's `flow-loader` uses
  `flow-remove-types` (2.321.0), which only **strips type annotations** — it leaves a
  Flow `enum` intact (verified: it re-emits `enum VirtualViewMode`). SWC then parses the
  `enum` keyword and errors. Metro's own pipeline handles this because
  `@react-native/babel-preset` includes `babel-plugin-transform-flow-enums` (backed by
  hermes-parser); Re.Pack's SWC path has no equivalent.

## Fix

1. **PATH — in the project, not the shell profile.** `app/Makefile` now resolves
   `ANDROID_HOME ?= $(or $(ANDROID_SDK_ROOT),$(HOME)/Android/Sdk)`, checks
   `$(ANDROID_HOME)/platform-tools/adb` exists (clear error if not), and runs
   `pnpm android` with `ANDROID_HOME` + `platform-tools`/`emulator` prepended to PATH.
   `make android` now works with no shell-profile edit; override with
   `make android ANDROID_HOME=/path`.
2. **Deps** (shell is a standalone pnpm project — memory: app-shell-standalone-pnpm-policy):
   ```
   cd app/shell
   pnpm add --ignore-workspace --config.minimumReleaseAge=0 @module-federation/enhanced@0.22.0
   pnpm add --ignore-workspace --config.minimumReleaseAge=0 @swc/helpers
   ```
   `--config.minimumReleaseAge=0` is **required**: `--ignore-workspace` also ignores the
   shell's own `pnpm-workspace.yaml` (which sets `minimumReleaseAge:0`), so the freshly
   published Expo 57 entries in the lockfile otherwise trip the supply-chain age policy.
   Match the MF runtime version already in the lockfile (`0.22.0`).
3. **Flow enums.** A Babel `pre`-loader in `app/shell/rspack.config.mjs`, scoped to
   `react-native` + `@react-native` via `Repack.getModulePaths(...)`, runs
   `babel-plugin-syntax-hermes-parser` + `babel-plugin-transform-flow-enums` **before**
   flow-loader/SWC, lowering the enums to `flow-enums-runtime` calls. All four plugins
   were already installed (transitively via `@react-native/babel-preset`).

## Verification

- `make android` → **BUILD SUCCESSFUL**, `adb reverse` + `am start` run, exit 0.
- App launches on device `SM-A566B` (`R5GYB2T2GYH`), process alive, no `FATAL`/
  `AndroidRuntime`/`ReactNativeJS` error in logcat; app makes gateway DNS calls (JS
  bundle loaded).
- Android bundle compiles clean: `curl 'localhost:8081/index.bundle?platform=android'`
  → HTTP 200, ~8.1 MB, no `Syntax Error`/`enum VirtualViewMode` in the output.

## Prevention / notes

- One remaining ⚠ (non-fatal): `Can't resolve '@react-native-masked-view/masked-view'`
  — an **optional** peer of `@react-navigation/elements`; only used if installed. Add it
  only if a screen needs masked headers.
- The Makefile default assumes the Linux Android-Studio path (`~/Android/Sdk`). On macOS
  the same default holds; elsewhere pass `ANDROID_HOME=`.
- Do **not** commit the deps into the root workspace lockfile — they belong to the
  standalone shell (memory: app-shell-standalone-pnpm-policy).
