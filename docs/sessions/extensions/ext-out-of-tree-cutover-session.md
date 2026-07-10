# Extensions — out-of-tree SDK cutover (session)

- Date: 2026-07-10
- Scope: ../../scope/extensions/ext-out-of-tree-scope.md (slices 1–2)
- Stage: S10+ — extension developer experience / out-of-tree split
- Status: in-progress (slices 1–2 shipped + green; slices 3–5 — Artifact v2, extensions move, CI — deferred)

## Goal

Make the standalone `NubeDev/lb-ext-sdk` (Rust) and `NubeDev/lb-ext-ui-sdk` (TypeScript) the
authoritative owners of the extension contract, and turn `lb` into a plain **consumer** of them:

- Slice 1: fill the SDK's real bodies (the native child-side serve loop; a guest WIT helper).
- Slice 2 (the cutover, load-bearing): move the WIT world out of `lb/rust/sdk`, repoint lb's
  `ext-loader`/`runtime`/workspace from the `rust/sdk` path dep to the `lb-ext-sdk` git-tag dep,
  delete `lb/rust/sdk`, and switch the UI shell to import the page/widget contract from
  `@nube/ext-ui-sdk`. Prove behavior-neutral: `cargo build/test --workspace` + `pnpm test` green.

## What changed

### In `lb-ext-sdk` (the standalone SDK repo) — slice 1

- **`lb-ext-native` now speaks lb's REAL supervisor wire.** It previously defined a divergent
  `Request`/`Response`/`Init` shape that no lb host could read. Rewrote it to mirror
  `lb-supervisor` byte-for-byte:
  - `frame.rs` — `Content-Length` framing (child mirror of `lb-supervisor::frame`).
  - `wire.rs` — `Request { id, method, params }` / `Reply { id, result, error }` / `Method`
    (`init`/`health`/`call`/`shutdown`) / `CallParams`, serde-identical to `lb-supervisor::rpc`.
  - `handshake.rs` — `InitReply { protocol_major, tools }` returned as the `init` reply result;
    `PROTOCOL_MAJOR` unchanged (the native analogue of `WORLD_MAJOR`).
  - `serve.rs` — a real **`serve(reader, writer, tools)`** loop: the four control methods dispatched
    to a caller-supplied `Tools` trait (opaque-JSON `call`), ending on `shutdown` or EOF.
  - `stdio.rs` — `serve_stdio(tools)`, the one call a native extension's `main` makes.
- **`lb-sdk`** documents the guest WIT-consumption pattern + exposes `WORLD_NAME`; a `links` build
  script exports the WIT dirs as `DEP_LB_SDK_WIT` / `DEP_LB_SDK_WIT_COMPAT` so any out-of-tree host
  or guest can `bindgen!`/`generate!` against the one authoritative WIT (see cutover below).
- Tagged **`sdk-v0.2.0`** (serve loop) then **`sdk-v0.2.1`** (WIT `links` export). 26 tests green,
  `fmt` + `clippy -D warnings` clean.

### In `lb` — slice 2 (the cutover)

- Workspace `lb-sdk` dep: `{ path = "sdk" }` → `{ git = "…/lb-ext-sdk", tag = "sdk-v0.2.1" }`;
  removed `sdk` from `members`; **deleted `lb/rust/sdk/`**.
- **Host bindgen sourced from the SDK's WIT, no in-repo copy.** `bindgen!`/`generate!` resolve their
  `path:` against the *consuming* crate, so once the SDK is a git dep the old `../../sdk/wit` literal
  is dead. Added `rust/crates/runtime/build.rs`: it reads `DEP_LB_SDK_WIT*` and emits the two
  `bindgen!` invocations (with the absolute WIT path baked in as a literal) into `$OUT_DIR`;
  `bindings.rs` / `compat_v0_1.rs` `include!` them. `ext-loader` needed no build change — it only
  calls `lb_sdk::world_major_matches` (trivially fine over the git dep).
- **Guests use the same seam.** `hello` + `hello-v2` (the fixtures `make build-wasm` builds) gained a
  `build.rs` (reads `DEP_LB_SDK_WIT`, emits `generate!` into `$OUT_DIR`) and a normal `lb-sdk` git
  dep. The five product exts (proof-panel, echarts-panel, github-bridge, thecrew, energy-dashboard),
  the `core-thing` test fixture, and the **devkit wasm template** (`build.rs.tmpl` + scaffold
  registration) were converted identically — so a freshly-scaffolded ext gets the pattern too.
- **UI shell imports the contract from `@nube/ext-ui-sdk`.** `ext-host/federation.ts` re-exports
  `RemoteMount` from the package; `dashboard/builder/federationWidget.ts` re-exports
  `WidgetField/WidgetFrame/WidgetTheme/WidgetCtx/WidgetHandle/RemoteWidgetMount` (and aliases the
  package's `WidgetBridge` to the local `WidgetBridgeContract`) — the "three mirrors" collapse to one
  source. Added `@nube/ext-ui-sdk` as a `link:../../lb-ext-ui-sdk` dep (interim until npm publish).
  Updated `contract-mirrors.guard.test.ts` to assert the NEW reality: the authoritative source is the
  package (v4 theme), and the host file imports rather than redefines it.

## Decisions & alternatives

- **`links` build-script WIT export, not a vendored copy in lb.** `bindgen!` can't read an env var in
  its `path:` literal, and `include_str!` can't reach a dep. The cargo-native answer is a `links`
  crate exporting the WIT path as `DEP_*` metadata + a per-consumer `build.rs` that generates the
  macro call into `OUT_DIR`. Rejected keeping a `wit/` copy in lb: that resurrects the mirror the
  split exists to kill (and for the *host* specifically — the leak the scope calls out).
- **`lb-sdk` is a NORMAL dep of the guests, not a build-dep.** Cargo only hands `DEP_*` metadata to a
  dependent whose build script's package links the `links` crate via `[dependencies]`; a build-dep
  does not propagate it (verified: `NotPresent` until moved). `lb-sdk` is pure Rust with no deps, so
  compiling it for `wasm32-wasip2` is free.
- **Native wire matched to lb, not kept as the SDK's prior invented shape.** A published child wire
  that no real host speaks is worse than useless — it *looks* done. The serve loop now drives lb's
  actual `init/health/call/shutdown`, proven by a full-lifecycle test driven from the host side.
- **Zero-boilerplate `lb_sdk::export!` macro deferred, not faked.** Re-exporting wit-bindgen's
  generated `export!` across a published-crate boundary is version-fragile; documented the honest
  `generate!` pattern instead (open question below).

## Tests / green output

- `lb-ext-sdk`: `cargo test --workspace` → 26 passed (9 CLI + 17 native incl. full
  init→call→shutdown over an in-memory duplex); `clippy -D warnings` clean. (pasted in scope/status.)
- `lb`: `make build-wasm` → hello + hello-v2 build **green** against the SDK WIT (`BUILD_WASM_EXIT: 0`);
  `cargo build --workspace` → **green** (`BUILD_EXIT: 0`, `Finished ... in 2m 17s`).
- `lb`: `cargo test --workspace` → **21 test binaries ok; ONE pre-existing failure**
  (`lb-cli reminder_test::create_ls_..._real_gateway` → `Denied { tool: "reminder.create" }`), which
  is a cap-resolution gap from the tree's in-flight **builtin-role-freshness authz WIP** (modified
  `authz/*` + untracked `builtin_caps.rs`/`resolve_live.rs`, wired via `host/src/authz/mod.rs`) — NOT
  this extraction, which touches zero authz code. Every runtime/ext-loader/host/registry suite that
  exercises the WIT boundary is green (the extraction is behavior-neutral).
- `lb`: `pnpm test` → **166 test files pass, 2 fail** — both pre-existing on this tree:
  `radius-scale.guard.test.ts` (bare `rounded` in the in-progress MarkdownBody/SetupWizard refactor) and
  `flows/debug/DebugValueView.test.tsx`. The extraction's OWN tests are green:
  `contract-mirrors.guard.test.ts` (6), `ExtWidget.test.tsx` (6), `ExtWidgetTheme.test.tsx` (1), and the
  ext-host federation tests.

  (Pre-existing red set recorded in the agent-memory note `preexisting-fails-2026-07-10-tree`.)

## Deferred (explicit, not silent gaps)

- Slice 3 (Artifact v2 UI bundle + `lb-ext publish` wired), slice 4 (extensions move to
  `lb-extensions`, fixtures to `rust/fixtures/ext/`), slice 5 (CI conformance) — untouched here.
- The five product exts still live in `rust/extensions/`; they now build against the SDK dep (proving
  the surface suffices) but move out in slice 4.
- npm publish of `@nube/ext-ui-sdk`; until then lb links the sibling repo (`link:`).

## Related

- scope: ../../scope/extensions/ext-out-of-tree-scope.md
- sibling seam: ../../scope/node-roles/embed-node-scope.md (Phase 2 — embed lb as a lib)
- SDK repos: NubeDev/lb-ext-sdk (`sdk-v0.2.1`), NubeDev/lb-ext-ui-sdk (`ui-v0.4.0`)
