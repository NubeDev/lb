# App scope — adopt Expo modules without losing Module Federation

Status: scope (the ask). Promotes to `public/app/` once shipped.

Bring the parts of Expo that are worth having — the prebuilt native module library
(`expo-*`) and, optionally, Expo's cloud build/submit service (EAS Build) — into the
**existing bare React Native shell** (`app/shell/`) **without giving up Re.Pack +
Module Federation**. The shell keeps Re.Pack as its one and only bundler; Expo is
adopted the "bare" way (`expo-modules-core` installed into the hand-owned `android/`
and `ios/` projects), never the managed workflow. This is additive: today the shell has
no Expo, and the whole point of this scope is that turning it on changes nothing about
how extensions are built, published, discovered, or mounted.

## Goals

- The shell can install and use any `expo-*` native module (e.g. `expo-secure-store`,
  `expo-notifications`, `expo-camera`, `expo-file-system`) as an ordinary dependency,
  with `expo-modules-core` wired into the existing native projects.
- Re.Pack 5 (Rspack) + Module Federation 2 remain **unchanged** — the MF2 host contract
  in `app/shell/rspack.config.mjs` and the JS-only remote model
  (`app-extensions-scope.md`) are untouched. Metro is never introduced.
- EAS Build remains **available** for CI binary builds/signing/submission of the bare
  project, as an option — not a required part of the pipeline.
- The native-dependency contract stays explicit: an `expo-*` module a remote wants must
  be shipped by the **shell**; a remote can never pull in native code (rule holds from
  `app-extensions-scope.md`).
- A single migration slice proves it: adopt Expo's module system, port **one** existing
  shell native dependency to its Expo equivalent as the proof-of-life, and show all the
  gateway integration tests still green.

## Non-goals

- **No managed workflow, ever.** No `expo start`, no Expo Go, no `expo prebuild` as the
  source of truth for native config, no expo-router. These are Metro-coupled and would
  displace Re.Pack — the one thing this scope exists to protect. (See "Intent" for why
  each is out.)
- **No EAS Update (OTA).** Expo's OTA ships Metro bundles and is incompatible with
  Re.Pack output. The shell already has a better OTA story: federated remotes load live
  from the gateway (`app-extensions-scope.md`). This is a decision, not a gap.
- No Expo module exposed to extensions as a native capability — remotes stay JS-only;
  anything an extension needs from an Expo module is either shell-mediated UI or reached
  through a gateway verb.
- No re-platforming of the build: `app/shell` stays outside the pnpm workspace, keeps
  its own lockfile, and keeps Re.Pack. This scope adds a native library system beside
  the bundler; it does not touch the bundler.
- Not a mandate to adopt any *specific* `expo-*` module. This scope makes them
  *available* and proves one; each real adoption (push notifications, camera, …) is its
  own later slice with its own capability/permission story.

## Intent / approach

**The core tension, stated plainly:** Expo's managed workflow and Module Federation both
want to own JavaScript bundling, and a project has exactly one bundler. MF requires
Re.Pack (Rspack/webpack); the managed workflow requires Metro. They cannot coexist.
Extensions are a must-have (they depend on Re.Pack), so the managed workflow is out. But
"Expo" is three separable things, and only one of them owns the bundler:

| Expo layer | What it is | Bundler-coupled? | This scope |
|---|---|---|---|
| **Expo modules** (`expo-modules-core` + `expo-*`) | A native module library + autolinking | **No** — plain native deps | **Adopt** (bare install) |
| **EAS Build / Submit** | Cloud build & signing of the binary | No — supports bare projects | **Keep available** |
| **Managed workflow** (`expo start`, Expo Go, expo-router, EAS Update) | Metro dev server, OTA, routing | **Yes — Metro** | **Reject** (kills MF) |

The approach is the **bare install** path: run `npx install-expo-modules@latest` in
`app/shell`, which adds `expo` + `expo-modules-core`, patches `MainApplication` /
`AppDelegate` and the Gradle/Podfile wiring so Expo modules autolink into the *existing*
native projects, and leaves `index.js`, `rspack.config.mjs`, and the whole federation
seam alone. From that point `expo-*` modules install like any other native dependency
and Re.Pack bundles the JS exactly as before.

**Why the managed pieces are individually out (not hand-waved):**

- **Metro** would replace Rspack — MF2 remotes could no longer be built or loaded. Hard
  blocker; the entire scope is about avoiding this.
- **Expo Go** cannot load custom native modules or non-Metro bundles — useless to a
  federated bare app.
- **expo-router** is Metro-dependent and duplicates the shell's existing React
  Navigation (`app-shell-scope.md`).
- **EAS Update** ships Metro bundles; the gateway-served remotes are the OTA channel and
  are strictly more capable (per-extension, live).

**Rejected alternative — stay pure-bare, never touch Expo.** Viable (it's today's
state), but it permanently forecloses the large, well-maintained `expo-*` module library
and the EAS build convenience, forcing a hand-rolled native module or a lesser community
package every time a device capability is needed. The bare-install path costs one small
slice now and one extra item in the RN-upgrade checklist (below), in exchange for
durable access to that library. Worth it. The blocker people fear ("Expo breaks MF") is
real **only** for the managed workflow, which we are not adopting.

**RN-upgrade coupling (the one recurring cost).** Adding `expo-modules-core` couples the
native projects to an Expo SDK version, so an RN version bump now also checks the
Expo-SDK↔RN compatibility matrix (Expo publishes it per SDK). This is one line added to
the existing "RN + Rspack toolchain drift" upgrade checklist in `app-shell-scope.md`, not
a new class of problem — the shell already treats RN/Re.Pack upgrades as their own
slices.

## How it fits the core

- **Tenancy / isolation:** N/A at the platform layer — this is a client build/native
  concern. No workspace keys, records, or tokens change. The mandatory isolation test
  still passes unchanged because the gateway seam is untouched (that's the point of the
  regression run in the testing plan).
- **Capabilities:** **no new platform capabilities.** An `expo-*` module that touches a
  device resource (camera, notifications) will need an **OS permission**, which is a
  native-manifest concern (Info.plist / AndroidManifest), *not* a platform capability —
  and every such adoption is a separate later slice. This scope adds none; it only wires
  the module system in and ports one benign module.
- **Placement:** N/A — the app is a client of whichever node it points at; symmetry
  (rule 1) is unaffected because no core crate changes.
- **MCP surface:** none — no verbs added or consumed. `ext.list` and the bridge are
  untouched; the host still branches on no extension id (rule 10 holds trivially, since
  no core code changes at all).
- **Data (SurrealDB) / Bus (Zenoh):** none touched.
- **Sync / authority:** unchanged — the node stays authoritative; nothing here adds
  client state.
- **Secrets:** if a later slice adopts `expo-secure-store` it becomes an *alternative*
  keychain backend for the session token (today: `react-native-keychain`). The
  invariant holds either way — the token lives in the OS secure store, never in
  AsyncStorage, and no provider keys ever reach the device. This scope may port the
  token store to `expo-secure-store` as its proof-of-life (see below) or leave it; the
  invariant is the test, not the library.
- **Stateless extensions / SDK / WIT:** **zero** impact — the WIT ABI, the federation
  contract mirrors (`@nube/app-sdk`, `federationWidget.ts`, devkit template), and the
  JS-only remote rule are all untouched. Flagged explicitly because a reader's first
  fear is "does adding Expo move the contract?" — it does not.

## Example flow

The proof-of-life migration (one slice):

1. In `app/shell`, run `npx install-expo-modules@latest`. It adds `expo` +
   `expo-modules-core`, patches `MainApplication.kt` / `AppDelegate.mm`, the root
   Gradle/Podfile, and `app.json`; it does **not** touch `index.js` or
   `rspack.config.mjs`.
2. Rebuild the dev binary the existing way (`pnpm ios` / `pnpm android`, which run
   `react-native run-*` → Re.Pack dev server). App boots; MF host still initializes;
   `ext.list` still discovers and mounts a remote — federation demonstrably intact.
3. Add `expo-secure-store`. Point the shell's token store at it behind the existing
   keychain seam (one file, one responsibility — FILE-LAYOUT). `react-native-keychain`
   is removed only once parity is proven, or kept; either is fine.
4. Run the full real-gateway suite (`pnpm test:gateway`) — login, workspace switch,
   capability-deny, workspace-isolation, SSE resume — all green, proving the native
   change did not disturb the gateway seam.
5. Build a release binary via EAS Build against the bare project (optional, proven once)
   to confirm the cloud pipeline accepts the Expo-enabled bare app.
6. A remote extension is loaded and exercised after the change to prove the JS-only MF
   path is byte-for-byte unaffected.

## Testing plan

Per `scope/testing/testing-scope.md` — real infra, rule 9, no fakes. This is a build/
native change, so the test strategy is **regression-first**: prove the existing behavior
survives, plus one new native-module assertion.

- **Federation-intact regression (the headline):** after the Expo install, a remote
  extension still builds (Re.Pack) and mounts through the shell — run the
  `app-extensions` mount path against the **real spawned node** (`test_gateway` +
  `vitest.gateway`, the shipped pattern). If MF broke, this is where it shows.
- **Mandatory capability-deny:** unchanged and re-run — a token missing
  `mcp:channel.post:call` still denies through the app client with a typed error. Proves
  the gateway seam is undisturbed.
- **Mandatory workspace-isolation:** unchanged and re-run — two workspaces, two tokens,
  no cross-visibility through the app seam.
- **Session + SSE:** the full `pnpm test:gateway` suite (login → workspace switch →
  old-token reject; post/stream/resume) stays green.
- **Native-module smoke:** the ported module (e.g. `expo-secure-store`) round-trips the
  session token — store, read back, and a subsequent authenticated call succeeds on a
  real device/simulator. This is the one genuinely new assertion.
- **No new mock/fake** is introduced (rule 9). Expo modules are real native code
  exercised on a real simulator; the gateway is the real spawned node.

## Risks & hard problems

- **Metro creeping back in.** The single biggest risk is an `expo-*` module (or a
  tutorial) that assumes Metro config, or a contributor running `expo prebuild` and
  overwriting the hand-owned native config. Mitigation: document the bare posture at the
  top of `app/shell/README.md`, never add `expo start` to `package.json` scripts, and
  treat any Metro config file appearing in the tree as a finding.
- **Autolinking vs Re.Pack’s module resolution.** Expo autolinking is a *native* build
  step (Gradle/CocoaPods); Re.Pack owns *JS* resolution. They operate on different
  layers and don't normally collide, but a shared-singleton misconfig could surface as a
  duplicate-module runtime error. Mitigation: the federation-intact regression test
  catches it immediately; keep the MF2 `shared` list authoritative.
- **Expo SDK ↔ RN version matrix.** Bumping RN now also constrains the Expo SDK version.
  Mitigation: add it to the existing RN-upgrade slice checklist; pin the Expo SDK and
  bump it deliberately.
- **EAS Build config drift for a bare project.** EAS supports bare, but a bare project
  gives EAS less to infer; the `eas.json` + credentials must be maintained by hand.
  Mitigation: prove EAS once in this slice, commit a working `eas.json`, and treat build
  config as code.
- **Per-module OS permissions are their own beast.** Each device-capability module
  (notifications, camera) drags an OS-permission and store-review story. Mitigation:
  explicitly out of scope here — this slice only wires the system and ports a benign
  module (secure-store, no user-facing permission prompt).

## Open questions

- **Proof-of-life module:** port the session-token store to `expo-secure-store`, or pick
  a zero-permission utility module (`expo-application`/`expo-constants`) as the smaller,
  lower-risk proof? Recommendation: `expo-secure-store`, because it exercises the one
  invariant that matters (secure token storage) and would replace an existing dep rather
  than adding a throwaway.
- **Keep or drop `react-native-keychain`** once `expo-secure-store` reaches parity?
  Recommendation: drop it after parity, to avoid two secure-store paths — but only once
  the smoke test is green on both platforms.
- **Adopt EAS Build now, or just prove-and-park it?** Recommendation: prove it once,
  commit `eas.json`, but keep the local `pnpm ios`/`pnpm android` Re.Pack path as the
  primary dev loop; EAS is CI/release convenience, not the day-to-day.
- **Which Expo SDK version** pairs with RN 0.86.0 / React 19.2.3? Resolve against Expo's
  published RN-compatibility matrix at implementation time and pin it.

## Skill doc

N/A — this scope exposes no new agent-/API-drivable surface. It wires a native module
system into the shell and consumes no new verbs. If a *later* slice adds a drivable
surface on the back of an Expo module (e.g. push-notification device registration
verbs), that slice names its own `skills/<name>/SKILL.md`.

## Related

- `app-shell-scope.md` — the bare RN shell this extends; its "Expo not adopted" note
  (open questions) is superseded by *this* scope's bare-modules decision.
- `app-extensions-scope.md` — the JS-only MF remote model this scope must leave intact;
  the federation-intact regression is its guarantee.
- `app-sdk-scope.md` — the contract package; unaffected (flagged in "How it fits").
- `../extensions/ui-federation-scope.md` — the web federation counterpart (also
  bundler-owned; same "one bundler" logic).
- README §3 (rules 1, 5–10), and `app/shell/rspack.config.mjs` (the MF2 host contract
  this must not disturb).
