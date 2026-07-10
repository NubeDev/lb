# Extensions scope — out-of-tree extensions: split repos + published SDKs (Rust · WASM · UI)

Status: scope (the ask). Promotes to `public/extensions/extensions.md` (SDK/dev-flow section) once shipped.

Move the product extensions out of this repo. `rust/extensions/` today holds a dozen extensions that
core, by rule 10, must know nothing about — yet they live inside the core workspace, reach core through
`../../crates/*` path deps, and publish via a Makefile that hand-copies UI bundles. We want: **one
`lazybones-extensions` repo** holding every product extension (all of `rust/extensions/*` except
`federation`, which stays), and **three published SDK surfaces** — a WASM SDK (the WIT world), a native
Rust SDK (the sidecar wire + callback), and a UI SDK (the mount/widget contract + build preset) — so an
extension builds, packs, and publishes against **versioned contracts** instead of a sibling directory.
This is the proof of rule 10: if core truly knows no extension, the extensions can live anywhere.

## Goals

- **`rust/extensions/` reduced to `federation`.** Every other extension moves to a new
  `lazybones-extensions` repo. Minimal **test fixtures** (`hello`, `hello-v2`, `echo-sidecar`) move to
  `rust/fixtures/ext/` so `cargo test --workspace` stays green with no external clone (rule 10 allows
  test fixtures; product extensions are what leave).
- **Three SDK surfaces, versioned and published from the core repo:**
  - **WASM:** `lb-sdk` (exists, `rust/sdk/`) becomes the published crate an out-of-tree wasm extension
    depends on — it owns `wit/world.wit`, `WORLD_MAJOR`, and a wit-bindgen re-export/helper so a guest
    writes `lb_sdk::export!` instead of hand-wiring bindgen against a copied `.wit`.
  - **Rust (native):** a new facade crate **`lb-ext-native`** (`rust/sdk-native/`) re-exporting exactly
    the child-side surface: the stdio wire protocol (init/health/call/shutdown serve loop from
    `lb-supervisor`), the host-callback client (`lb-sidecar-client`), and the `LB_EXT_TOKEN` self-check
    (from `lb-auth`). Extensions never import `lb-supervisor` directly again.
  - **UI:** a new package **`@nube/ext-ui-sdk`** (`packages/ext-ui-sdk/`) — the **single source** of the
    page contract (`RemoteMount`, `ctx`, `bridge`), the widget contract (`WidgetCtx` v4, frames-in,
    `ctx.theme`), and a `defineExtConfig()` Vite preset encoding the import-map externals
    (react/react-dom/jsx-runtime) and the css-isolation rules. The host's `ext-host/federation.ts` and
    `dashboard/builder/federationWidget.ts` import their types **from this package**, killing the
    "three mirrors" copy problem (`app/contract.ts` per extension dies).
- **The artifact carries the UI bundle** (Artifact v2). Out-of-tree publish must be one call; the
  current "POST wasm, then hand-copy `ui/dist/` into `LB_EXT_UI_DIR`" cannot work from another repo
  against a remote node.
- **A thin `lb-ext` CLI** over the existing `lb-devkit` library (`new` / `build` / `pack` / `publish`),
  shipped with the SDK release — the out-of-tree replacement for `make publish-ext`. The in-shell
  Extension Studio keeps working (`LB_DEVKIT_ROOT` points at an extensions-repo checkout).
- Devkit **templates emit SDK version deps**, not `../../crates` path deps, and become the
  authoritative source of the reference shapes once proof-panel/fleet-monitor move out.

## Non-goals

- **No new trust model or publish path.** The signed Ed25519 `Artifact`, `verify_artifact` →
  `VerifiedArtifact` → cache → install, `ext.publish` cap, and `LB_TRUSTED_PUBKEYS` are unchanged.
  Artifact v2 is an additive field on the same envelope, not a second format.
- **No change to the WIT world's contents.** `lazybones:ext@0.2.0` ships as-is; this scope formalizes
  its *ownership and distribution*, not its shape. Same for the manifest — `extension.toml` and
  `lb-ext-loader::Manifest` are untouched.
- **No remote registry service / marketplace.** The catalog (`CatalogEntry`) stays what it is; how a
  team hosts artifact files between repos is their CI's business.
- **Not splitting `federation` or the host's `crates/host/src/federation/*` module.** Federation is
  effectively part of the data plane: the host holds a first-class `federation.*` surface (query,
  datasource CRUD, endpoint gating, dbschema) and `FED_ENDPOINTS` config. Extracting it means first
  formalizing that host module as an API — a separate, later scope if ever. It stays a workspace member.
- **Not the generator/Studio features themselves** — `ext-sdk-scope.md` shipped those; this scope
  re-points what they emit and adds the CLI face it explicitly deferred.

## Intent / approach

**SDK source of truth stays in the core repo; extensions consume published versions.** The rejected
alternative — an SDK-first repo that core consumes as a git dep — breaks atomic evolution: host bindgen
and guest bindings generate from the same `world.wit`, and the supervisor wire is shared verbatim so
host/child ABI can't drift; putting that file a release-dance away from the host makes every contract
change a two-repo commit. Instead the core repo keeps owning `rust/sdk`, `rust/sdk-native`, and
`packages/ext-ui-sdk`, and a **release step publishes them**: git tags on the core repo consumed as
`{ git, tag }` deps now (same org, private is fine), crates.io/npm when the contracts are public. The
runtime stays the enforcement point — a `WORLD_MAJOR` mismatch is already refused at load; the native
tier gets the same treatment (below).

**Facade crates, not raw internals.** `lb-supervisor` contains both the host-side supervision
machinery and the child-side protocol; publishing it whole would freeze host internals as public API.
`lb-ext-native` exports only the child face. Same logic for the UI: `@nube/ext-ui-sdk` exports the
contract types and the build preset, not host shell components.

**One extensions repo, not repo-per-extension.** `lazybones-extensions` is a folder-per-extension
monorepo where each extension is **standalone** — own `Cargo.toml`, own lockfile, own `ui/` with its
own `pnpm-lock.yaml`. This is not new structure: it generalizes the pattern the wasm tier already has
(all workspace-excluded, self-contained) to the four natives currently riding the core workspace.
Rejected: twelve repos of CI/release boilerplate for one team. `control-engine` moves in too — its
private `rubix-ce` git dep is exactly what the shipped container-build token mount
(`devkit-container-build-scope.md`) exists for. The `ds-hidden-*`/`ds-pick-*` dirs are untracked
tooling scratch, deleted with prejudice.

**Artifact v2: the UI bundle rides the artifact.** Add an optional `ui_bundle: Option<Vec<u8>>`
(tar.zst of `ui/dist/`) to `lb-registry::Artifact`, covered by the same digest + signature; the
install path unpacks it into `LB_EXT_UI_DIR/<ext>/` where the gateway already serves it. Additive
serde-default — a v1 artifact (no bundle) still verifies and installs; `lb-pack`/`lb-ext pack` include
the bundle when `ui/dist` exists. Rejected: a second upload route for the bundle (two calls, two
signatures, a torn-install window).

**The native tier gets a wire-protocol version.** WASM has `WORLD_MAJOR` and a refuse-at-load check;
the supervisor's `init` handshake has none. Once native extensions pin a published `lb-ext-native`,
drift becomes possible, so the handshake carries an explicit protocol major (constant in
`lb-ext-native`, checked by the host at `init`) and a mismatch is refused as loudly as a `world`
mismatch. Without this, splitting the repo turns ABI drift from impossible into silent.

### Repo layout (github.com/NubeDev)

**The SDKs publish directly from the `lb` monorepo — there is no separate SDK repo.** crates.io and
npm publish a single crate/package out of a larger repo without ceremony (tokio, serde, et al. do
exactly this): `cargo publish -p lb-sdk` / `-p lb-ext-native` / `-p lb-ext`, and `npm publish` from
`packages/ext-ui-sdk`. The published `.crate`/tarball **is** the public artifact — consumers get the
source in it — so a standalone "SDK repo" would be a pure mirror: a second copy, sync CI, and issues
landing in the wrong place. **Rejected** (this was an earlier draft's mistake): a mirror repo buys
nothing publishing-from-the-monorepo doesn't already give, and a pre-release consumer who wants the
unpublished tip pins a git dep at `lb`'s subdir directly (`{ git = ".../lb", tag = "sdk-v0.2.0" }`).

| Repo | Contents | Publishes / consumed as |
|---|---|---|
| `lb` *(existing)* | Core **and** the SDK sources: `rust/sdk` (`lb-sdk`, WIT world), the new `rust/sdk-native` (`lb-ext-native`), `rust/tools/ext-cli` (`lb-ext`), `packages/ext-ui-sdk` (`@nube/ext-ui-sdk`). Tagged `sdk-vX.Y.Z`. | **publishes** the SDK crates → crates.io + the UI package → npm, straight from the monorepo |
| `lb-extensions` | The extensions monorepo — everything leaving `rust/extensions/`: proof-panel, echarts-panel, fleet-monitor, ros, mqtt, github-bridge, energy-dashboard, thecrew, control-engine (container-build token mount for `rubix-ce`). | built + published as signed Artifacts to a node |

**Downstream consumer org — `github.com/NubeIO`** (the first real proof both seams work; lb-side
repos must not special-case it, rule 10):

| Repo | Contents | Consumes |
|---|---|---|
| `NubeIO/rubix-ai` | A product **host/node**: embeds lb via the `BootConfig`/`NodeBuilder` seam (`../node-roles/embed-node-scope.md`), git-dep on `NubeDev/lb`. | `lb` (git tag) |
| `NubeIO/rubix-ai-extensions` | That product's extensions, built against the **published SDKs** exactly like `lb-extensions` — same templates, same signed-Artifact publish, zero lb-repo access needed once the SDKs are on crates.io/npm. | `lb-sdk`/`lb-ext-native` (crates.io) + `@nube/ext-ui-sdk` (npm) |

Publishing accounts are **confirmed and named**: crates.io publishes under **`NubeDev`**, npm under
**`aidanpick`** — the registries are the destination. Until the first `sdk-v*` release lands there,
consumers pin `{ git = ".../lb", tag }` at the subdir. `@nube/app-sdk` stays inside `lb` until the app
contract settles (open question below).

**Templates become the authoritative shapes.** Today the devkit templates are "cut from proof-panel /
fleet-monitor"; once those move out, the embedded templates in `lb-devkit` are the in-core source of
truth, and the extensions repo's reference extensions are CI-checked to still build against each SDK
release — the direction of authority inverts, deliberately.

## How it fits the core

- **Tenancy / isolation:** unchanged — publish still lands the catalog entry + `Install` in the
  caller's workspace only; the extensions repo is outside the wall entirely (it produces artifacts,
  which enter through the unchanged gate).
- **Capabilities:** **no new verbs.** `lb-ext publish` signs locally (the `lb-pack` key idiom) and
  POSTs `/extensions` through the existing `ext.publish` cap gate; `lb-ext build` shells the local
  toolchain directly (it *is* the developer's box — the devkit's local-only wall guards the *node's*
  build verb, not a developer's own shell). Deny paths (403 cap, 422 untrusted/tampered) are re-asserted
  over Artifact v2.
- **Placement:** N/A for the split itself; `devkit.*` / Studio stay local-only per `ext-sdk-scope.md`.
- **MCP surface (API shape §6.1):** no new tools. The CLI is a client of existing surfaces
  (`POST /extensions`; optionally `devkit.*` when driving a node build). N/A for CRUD/feed/batch.
- **Data (SurrealDB):** one additive field on the registry `Artifact` (v2 `ui_bundle`); catalog/install
  records unchanged. No new persistence.
- **Bus (Zenoh):** N/A — no new motion (devkit build logs already ride `bus.publish`).
- **Sync / authority:** N/A.
- **Secrets:** unchanged custody — publisher seed in `LB_DIR/keys` (node-side for Studio, developer
  keyfile for the CLI, never in the extensions repo); the container-build GitHub token mount covers
  private deps (`rubix-ce`) per the shipped scope.
- **Stateless extensions / state-vs-motion / symmetric nodes / one datastore:** untouched by the split.
- **No mocks:** the conformance tests below run real builds against a real spawned node; the extensions
  repo's CI publishes to a real node binary, not a stub registry.
- **SDK/WIT impact — this scope IS the SDK boundary, flag it loudly:** `lb-sdk` becomes a published
  contract with semver; `lb-ext-native` freezes the child-side wire as public API and adds the protocol
  major to the `init` handshake; `@nube/ext-ui-sdk` freezes `RemoteMount`/`WidgetCtx`. From this point a
  contract change is a **versioned release**, not an edit — the FILE-LAYOUT "three mirrors" comment on
  `federationWidget.ts` collapses to one source.

## Example flow

1. A developer clones `lazybones-extensions`, runs `lb-ext new cooler-panel --tier native --features ui,ingest`.
   The generator (same `lb-devkit` templates) emits a standalone crate whose `Cargo.toml` pins
   `lb-ext-native = { git = "…/lb", tag = "sdk-v0.2.0" }` and whose `ui/package.json` pins
   `@nube/ext-ui-sdk@0.4.x` — no path deps, no copied `contract.ts`.
2. `lb-ext build` — cargo (host target or `wasm32-wasip2`) + `vite build` via the `defineExtConfig()`
   preset. Nothing here touches a node.
3. `lb-ext publish --node https://dev-node --key ~/.lb/dev-publisher.key` — packs wasm/manifest **and**
   `ui/dist` into one signed Artifact v2, POSTs `/extensions`. Gateway: cap gate → verify signature →
   cache → install → unpack UI bundle → loaded live. `204` ⇒ tool callable, page in the sidebar.
4. **Deny paths:** no `ext.publish` cap → 403, nothing installed. Key not in `LB_TRUSTED_PUBKEYS` →
   422. SDK pinned to an old major → the node refuses at load (`world` mismatch for wasm, `init`
   protocol mismatch for native) with a message naming both versions.
5. In-shell path still works: Studio on a local node with `LB_DEVKIT_ROOT` pointed at the
   `lazybones-extensions` checkout scaffolds/builds/publishes the same folders via `devkit.*`.

## Migration order (slices)

1. **SDK surfaces in-core:** `lb-ext-native` facade + native protocol version in the handshake;
   `lb-sdk` helper polish; `@nube/ext-ui-sdk` package with host importing its types. In-tree extensions
   switch to the facades while still path-dep'd — proves the surface is sufficient before any move.
2. **Artifact v2 + `lb-ext` CLI:** UI bundle in the artifact; `make publish-ext` reduced to a wrapper
   over `lb-ext publish`.
3. **The move:** create `lazybones-extensions`, migrate extensions one at a time (wasm tier first —
   already standalone — then natives, `control-engine` last), flipping each to `{ git, tag }` SDK deps;
   move fixtures to `rust/fixtures/ext/`; delete the emptied dirs; retarget `make build-wasm` and the
   devkit templates.
4. **Close-out:** extensions-repo CI (build + publish each ext against a spawned node), core CI
   conformance job (generate → build → publish for both tiers against the released SDK), docs promoted.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all against the real store/gateway/node:

- **Capability deny (mandatory):** publish without `ext.publish` → 403, nothing cached/installed;
  untrusted/tampered Artifact **v2** → 422 (signature covers the UI bundle — a bundle swapped after
  signing must fail verify). Existing registry deny tests re-asserted over v2.
- **Workspace isolation (mandatory):** reuse the registry isolation test — ws-B cannot see ws-A's
  published entry/`Install`; unchanged path, re-run over a v2 artifact.
- **Version-gate tests (the new load-bearing pair):** a wasm guest built against the wrong
  `WORLD_MAJOR` and a native child announcing the wrong protocol major are both **refused at load**
  with an actionable error; the matching versions load. This is the contract that makes two repos safe.
- **Artifact compat:** a v1 artifact (no `ui_bundle`) still verifies + installs; a v2 artifact's bundle
  lands in `LB_EXT_UI_DIR` and the page is served (real gateway GET).
- **Conformance (CI, both repos):** core CI runs generate → build → publish → `204` + tool call for
  both tiers against the **released** SDK versions (template drift breaks loudly, per `ext-sdk-scope`);
  extensions-repo CI builds every extension against its pinned SDK and publishes to a spawned node.
- **Fixture green:** `cargo test --workspace` and the runtime/supervision/registry suites pass from a
  bare core clone using only `rust/fixtures/ext/*` — no external checkout, no network.
- **Hot-reload:** the hello → hello-v2 swap test keeps running from fixtures (stateless-extension
  guarantee unaffected by the split).

## Risks & hard problems

- **Contract drift across repos is now possible.** Mitigations are structural, not advisory: semver'd
  SDK releases, refuse-at-load version gates on *both* tiers, and conformance CI in both repos. The
  native protocol version is the piece that doesn't exist yet — shipping the split without it means
  silent ABI drift.
- **The UI mirror only collapses if the host actually imports from `@nube/ext-ui-sdk`.** If the host
  keeps its own `RemoteMount`/`WidgetCtx` and the package is a copy, we've made a fourth mirror.
  The slice-1 exit gate is `federation.ts`/`federationWidget.ts` importing the package types.
- **Release friction.** Every WIT/protocol/widget-contract bump becomes: tag SDK → bump extensions
  repo. That's the price of the split; keep it cheap with a single `sdk-vX.Y.Z` tag covering all three
  surfaces and a one-command bump in the extensions repo.
- **Private deps in the extensions repo** (`rubix-ce`) — solved by the shipped container-build token
  mount, but the extensions repo's CI must wire the same secret; document it there, not just here.
- **Artifact size.** UI bundles push artifacts from ~100s of KB to MBs; the gateway body limit and the
  cache table need a stated bound (open question below).
- **`thecrew`'s `@nube/source-picker` import** — the one extension UI that reaches into `packages/*`.
  It must stop: either `source-picker` ships in/alongside `@nube/ext-ui-sdk`, or thecrew vendors it.

## Open questions

- **Publish naming:** destinations decided — crates.io as **`NubeDev`**, npm as **`aidanpick`**,
  published straight from the `lb` monorepo (no SDK repo). Remaining: the crate names (`lb-sdk` may be
  taken on crates.io — `lazybones-sdk`?) and the npm package scope (`@nube/` matches the existing
  packages — confirm the `aidanpick` account can claim/own that org scope, else publish under
  `@aidanpick/`).
- **Native protocol version mechanics:** a major constant in `lb-ext-native` checked at `init`, or
  carried in `extension.toml` `[runtime]` beside `world`? (Recommend the handshake — the manifest can
  lie; the running child can't.)
- **Artifact size bound:** what's the max `ui_bundle` size the gateway accepts, and does the artifact
  cache need eviction once artifacts are MBs?
- **`source-picker`:** publish it, absorb the needed subset into `@nube/ext-ui-sdk`, or vendor into
  thecrew? Decide when thecrew migrates (slice 3).
- **`control-engine` home:** in `lazybones-extensions` with the token mount (recommended) or its own
  repo because of `rubix-ce`'s cadence?
- **`app/sdk` alignment:** `app-sdk-scope.md` wants one shared panel/widget SDK direction — does
  `@nube/ext-ui-sdk` and `@nube/app-sdk` converge now or stay parallel until the app contract settles?

## Related

- `ext-sdk-scope.md` — the devkit/Studio this builds on; its deferred `lb-ext` CLI ships here.
- `devkit-container-build-scope.md` — the hermetic build + token mount the extensions repo's CI uses.
- `extensions-scope.md` — the manifest contract and rule-10 doctrine this is the proof of.
- `ui-federation-scope.md`, `ui/theme-inheritance-scope.md`, `ui/css-isolation-scope.md` — the UI
  contracts `@nube/ext-ui-sdk` freezes.
- `reference-extensions-scope.md`, `proof-panel-scope.md` — the shapes that become devkit-template-
  authoritative after the move.
- `../registry/` — the Artifact/verify/cache/install path (v2 is additive to it).
- `../app/app-sdk-scope.md` — the sibling SDK direction to converge with.
- `../node-roles/embed-node-scope.md` — the complementary seam: extensions consume lb from *outside*
  (published SDKs); embedders consume it from *inside* (git-dep `BootConfig`/`NodeBuilder` — no
  crates.io publish of `lb-host`, no extra repo).
- README `§6.3` (two tiers), `§6.4` (registry/trust), `§13` (manifest), `§3` rules 5/9/10.
- Skill: the build will write **`skills/lb-ext/SKILL.md`** (the CLI is a drivable surface: new → build
  → publish against a live node, grounded in a real run).
