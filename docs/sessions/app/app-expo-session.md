# app — Expo bare-modules adoption (session)

- Date: 2026-07-04
- Scope: [app-expo-scope.md](../../scope/app/app-expo-scope.md)
- Stage: post-S10 app track — additive native-module system on the shipped shell
- Status: **partial-done** — module system wired, proof-of-life ported, all runnable
  checks green; **on-device build + native-module smoke deferred** (no iOS/Android
  toolchain in this environment — see "Deferred / not proven here")
- Public: promoted to [public/app/app.md](../../public/app/app.md) (Expo section)

## Goal

Adopt Expo's **bare** module system (`expo-modules-core` + `expo-*`) into the existing
bare RN shell **without giving up Re.Pack + Module Federation** — the exact ask in the
scope. Prove it by porting the session-token store from `react-native-keychain` to
`expo-secure-store` behind the existing one-file keychain seam, and show the real-gateway
suite still green.

## What shipped

1. **Packages (standalone install).** `expo@~57.0.2` + `expo-secure-store@~57.0.0` added to
   `app/shell` via `pnpm add --ignore-workspace`. Standalone isolation held: the shell
   resolves `react@19.2.3` + `@types/react@19.2.17` (single version), the repo-root
   React-18 workspace and its lockfile were **not** touched.
2. **Android native wiring** (module-linking only, Re.Pack bundling untouched):
   - `android/settings.gradle` — `includeBuild` the expo-gradle-plugin (resolved through
     node), apply `id("expo-autolinking-settings")`, call `expoAutolinking.useExpoModules()`.
     Kept RN's own `autolinkLibrariesFromCommand()` (Re.Pack-compatible) rather than
     Expo's `rnConfigCommand` override.
   - `MainApplication.kt` — `ExpoReactHostFactory.getDefaultReactHost` +
     `ApplicationLifecycleDispatcher` hooks.
   - `app/build.gradle` — **unchanged**. `expo-secure-store`/`expo-modules-core` don't need
     the `expoLibs` version catalog (that catalog is only for optional gif/webp Fresco
     deps), and `autolinkLibrariesWithApp()` was already present.
3. **iOS native wiring** (same principle):
   - `Podfile` — `require expo/scripts/autolinking`, `use_expo_modules!`, feed
     `use_native_modules!` the expo react-native-config command.
   - `AppDelegate.swift` — subclass `ExpoAppDelegate` / `ExpoReactNativeFactory` so expo
     modules get AppDelegate lifecycle, **while keeping** `moduleName: "LazybonesShell"`
     and debug `bundleRoot: "index"` (NOT prebuild's Metro `"main"` /
     `.expo/.virtual-metro-entry`).
4. **app.json** — added a minimal `expo` block (name/slug/bundle id/package) beside the
   existing RN top-level `name`/`displayName`. No `scheme`, no expo-router, no OTA config
   — nothing managed.
5. **Proof-of-life port.** `src/features/session/keychain.storage.ts` now uses
   `expo-secure-store` (single entry, same `io.nube.lazybones.sessions` keychainService for
   in-place upgrade, `AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY`). The `SessionStorage` seam
   contract is byte-identical, so the sdk store folds it back unchanged.
6. **Web-preview follow-through.** The Vite alias `react-native-keychain →
   web/keychain-module.web.ts` became `expo-secure-store → …`; the stub now mirrors the
   SecureStore surface. `keychain.storage.web.ts` (localStorage, preview-only) unchanged.

## Key decisions (and the alternative rejected)

- **Expo SDK 57 is the pairing, resolved from the matrix.** Read
  `expo@57.0.2/bundledNativeModules.json`: it pins `react-native@0.86.0` (the shell's exact
  RN) and `expo-secure-store@~57.0.0`. SDK 56 pins RN 0.85.3 — wrong. This closes the
  scope's "which Expo SDK" open question definitively, not by guessing.
- **Hand-applied the native wiring, did NOT use `install-expo-modules`.** That tool's
  latest release (0.16.0) only maps up to SDK 56 / RN 0.85 and errors on RN 0.86
  ("Unable to find compatible Expo SDK version"). Rejected downgrading RN to use the tool
  (contradicts the shell's RN 0.86 pin). Instead ran `expo prebuild` in an **isolated
  scratch copy** purely to *read* Expo's authoritative SDK-57 native files, then
  transcribed the **module-linking subset** into the hand-owned projects — never letting
  prebuild own the tree (scope non-goal).
- **Surgical merge, not wholesale adoption of prebuild output.** Prebuild's generated
  `app/build.gradle`/`AppDelegate`/`Podfile` are *managed-flavored*: they route bundling
  through Expo CLI (`export:embed`, `.expo/.virtual-metro-entry`, moduleName `"main"`) —
  i.e. Metro. Adopting them verbatim would have silently swapped the bundler and broken MF.
  Took only the expo-module autolinking + lifecycle pieces; left every bundler touchpoint
  on Re.Pack.
- **Kept `react-native-keychain` as a dormant dep.** Scope open question recommends dropping
  it "only once the smoke test is green on both platforms" — which needs a device we don't
  have here. No code imports it; it's dropped in the device slice.
- **Install policy moved to `pnpm-workspace.yaml` (new, standalone).** The SDK-57 line
  ships packages published *today*, which pnpm 11's default `minimumReleaseAge` guard
  rejects; and pnpm 11 blocks build scripts by default. Fixed with `minimumReleaseAge: 0`
  + `allowBuilds: {esbuild, puppeteer}` in the shell's own workspace file (it is NOT a
  member list — a `packages:` key would refold it into the workspace and re-break the
  React 18/19 split). Rejected the brittle per-version `minimumReleaseAgeExclude` list the
  installer first generated (every SDK patch bump would break it).

## Tests (real infra, rule 9 — no fakes)

- **Gateway seam regression (headline):** `cd app/sdk && pnpm test:gateway` against the
  **real spawned `test_gateway` node** — **17/17 green**, including the mandatory
  `caps-deny.gateway.test.ts` (capability-deny) and `isolation.gateway.test.ts`
  (workspace-isolation), plus session/SSE-resume. Proves the native change did not disturb
  the gateway seam. (app/sdk imports no expo; it's the seam that matters.)
- **Shell typecheck:** `tsc -p tsconfig.json` — clean, including the ported token store.
- **Web-preview bundle (Federation-adjacent smoke I *can* run without a device):**
  `vite build --config vite.config.web.mts` — 568 modules transformed, no unresolved
  `expo-secure-store`, MF host + all aliases resolve. If the port had broken JS resolution,
  this is where it would show.

## Deferred / not proven here (honest gaps)

This environment has **no iOS/Android toolchain** (`pod`/`xcodebuild`/`adb` absent) and no
simulator, so the genuinely device-only parts of the scope's testing plan could not run:

- **Native build** (`pnpm ios` / `pnpm android` → Gradle/CocoaPods actually compiling the
  Expo-wired projects). The gradle/Podfile/AppDelegate edits are transcribed from Expo's
  own SDK-57 prebuild output but are **not compile-verified**.
- **Native-module smoke** (`expo-secure-store` round-tripping the token on a real
  Keychain/Keystore, then an authenticated call).
- **Federation-intact on-device** (a real MF remote mounting through the built binary).
- **EAS Build** of the bare project (scope step 5 — prove-and-park).

These are the natural first items for the device slice; nothing about them is blocked by a
design decision — only by the absence of a device here.

## Files touched

- `app/shell/package.json`, `app/shell/pnpm-lock.yaml`, `app/shell/pnpm-workspace.yaml` (new)
- `app/shell/app.json`
- `app/shell/android/settings.gradle`,
  `app/shell/android/app/src/main/java/com/lazybonesshell/MainApplication.kt`
- `app/shell/ios/Podfile`, `app/shell/ios/LazybonesShell/AppDelegate.swift`
- `app/shell/src/features/session/keychain.storage.ts`
- `app/shell/vite.config.web.mts`, `app/shell/web/keychain-module.web.ts`

## Related

- [app-expo-scope.md](../../scope/app/app-expo-scope.md) — the ask.
- [app-shell-session.md](app-shell-session.md) — the shell this extends; its "Expo not
  adopted" note is now superseded.
