# app — scope setup: React Native mobile app (session)

- Date: 2026-07-04
- Scope: ../../scope/app/README.md (three asks: app-shell, app-extensions, app-sdk)
- Stage: post-S8; scoped for a future stage slot (see STATUS.md)
- Status: done (scope setup only — no implementation)

## Goal

Turn the raw idea — "a React Native app with module federation and extensions, same
doctrine as the core framework: many workspaces, gateway access, AI agent, channels,
and long-term a shared panel/widget SDK with the dashboard" — into a complete scope
setup per `docs/SCOPE-WRITTING.md`: the scope docs, public stub, `/app` workshop
scaffolds, and index updates.

## What was produced

- **New topic `docs/scope/app/`** (README read-order index + three scope docs):
  - `app-shell-scope.md` — the RN host. Key decision: **transport = REST + SSE via
    the gateway; zenoh-ts rejected** (it is WS to the `zenoh-plugin-remote-api` — a
    second server surface *beside* the gateway that would bypass capability/workspace
    mediation, breaking rules 5–7; mobile network realities independently favor
    stateless HTTP + resumable SSE). A gateway-verbs-over-one-WS multiplex is noted as
    a future optimization behind the invoke seam, not an architecture change.
  - `app-extensions-scope.md` — additive `[app]` manifest block beside `[ui]`,
    **Re.Pack 5 + Module Federation 2** remotes, the **JS-only rule** (no per-ext
    native modules; build-time gate + trusted-publisher line), component-based mount
    (`Page`/`Widget` — RN has no DOM), unchanged signed-`Artifact` publish, served
    under `/extensions/{ext}/app/`. Rejected: WebView-hosted web remotes (kept only
    as a fallback posture). Untrusted app remotes deferred (no cheap RN sandbox).
  - `app-sdk-scope.md` — `@nube/app-sdk` as the **authored contract source** (ends
    the hand-synced-mirrors drift: host web copy, ext copies, devkit template get
    CI-checked against it), plus the shared verb map/invoke/SSE clients extracted
    from `ui/src/lib/ipc/http.ts`, aligning with the pending `panel-kit` →
    `@nube/panel-kit` promotion for the one shared panel/widget brain.
- **`docs/public/app/app.md`** — TODO stub.
- **`docs/scope/README.md`** — `app/` topic entry added.
- **`/app` workshop scaffolds** (READMEs + contract types only — honest stubs, no
  fake runnable code): `app/README.md`, `app/docs/README.md`, `app/shell/README.md`,
  `app/sdk/` (`package.json`, `src/index.ts`, `src/contract/{mount,widget,remote}.ts`
  — `WidgetCtx` mirrors the shipped web v3 frames-in contract byte-for-byte),
  `app/extensions/proof-panel-app/README.md` (manifest stays with
  `rust/extensions/proof-panel`), `app/extensions/channel-chat/{README.md,
  extension.toml}` (pure-app ext — the manifest lives with it). Empty `ext-a`/`ext-b`
  placeholder dirs removed.
- **`docs/STATUS.md`** — "scoped, next up" pointer added.

## Tests / debugging

No code changed (docs + type-only scaffolds); the testing standards are *named in the
scopes* instead: every implementation slice runs against the real spawned
`test_gateway` node with the mandatory capability-deny + workspace-isolation cases
(rule 9 — no `*.fake.ts`; the sdk ships no mock transport). No debugging entries —
nothing broke.

## Open questions carried into implementation

Per scope doc §Open questions — headline ones: RN/Re.Pack version pins, RN SSE client
choice (spike), `[app].sdk` compat field, whether `app/sdk` joins the root pnpm
workspace (lean: yes), devkit `--app` template in the same slice (lean: yes).

## Next

First implementation slice = **app-shell** (login → workspaces → channels over the
real gateway), then **app-extensions** (the two reference exts), then the **app-sdk
extraction** — each its own session per `docs/HOW-TO-CODE.md`.
