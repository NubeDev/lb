# Frontend scope — the minimal shell (a publishable host for 100%-extension UIs)

Status: scope (the ask). Promotes to `public/frontend/` once shipped.

> Read with: `../extensions/ext-out-of-tree-scope.md` (the SDK split this completes on the
> host side), lb `MIGRATION.md` (lb is a library now), rubix-ai's
> `docs/scope/ui/rubix-ui-scope.md` (the decision this obsoletes: **vendor the whole lb
> shell** was chosen *because nothing smaller existed*), `@nube/ext-ui-sdk` (defineRemote —
> the extension half of the contract; this is the missing host half).

An lb extension UI needs a host page to federate into — login, workspace pick, the
`ext.list` discovery, the scoped mount, SSE wiring, theme tokens. Today the only host is
lb's **full shell**: desktop-shaped, admin-heavy (sidebar, dock, admin console, dashboards),
and consumable only by **vendoring the whole `ui/` tree** (rubix-ai's documented
compromise). A product whose UI is *100% its own extension* — mobile-first, no lb chrome —
has nowhere to stand. We want the **minimal shell**: a small, published package (or
template) that does *only* the host-side contract, so an embedder ships
`minimal-shell + their extension` and nothing else.

## Goals

- **Only the contract:** auth screens (login; the invite-accept surface when
  `auth-caps/invites-scope.md` lands; workspace pick for multi-ws identities), boot config
  fetch, `ext.list` discovery, **full-screen scoped mount** of a designated extension page
  via the same federation seam the big shell uses, SSE/event-stream wiring, theme-token
  provider (the host side of the SDK's CSS-isolation contract), PWA manifest + installable
  defaults, mobile-first viewport behavior.
- **Configuration, not code:** which extension page is "home", branding (logo/name/colors —
  riding the shipped `ui_branding` prefs blob + its pre-auth cache), gateway URL. The
  extension id is **opaque config data** (rule 10) — the shell never branches on it.
- **Published like the SDKs:** consumable with zero lb checkout (`ui-v*`-tagged package or
  a devkit template — decide, see open questions), version-locked to `@nube/ext-ui-sdk`'s
  mount contract.
- **Small enough to stay small:** a hard budget (~15 files) — the moment someone wants a
  sidebar, they've outgrown it and should take the full shell.

## Non-goals

- **Not a replacement for lb's full shell** — the workbench (channels, dashboards, admin,
  dock) stays what it is; this is the *other* end of the spectrum.
- **No native (RN/Tauri) shell** — this is the web/PWA host; the `app/` scopes own native.
- **No nav framework** — one extension's page owns the whole viewport; in-ext routing
  belongs to the extension. (Multi-ext nav = you want the full shell.)
- **Not a UI kit** — components come from the extension + SDK presets.

## Intent / approach

Extract, don't rewrite: the full shell already contains the contract pieces (auth flow,
federation loader, event stream, theme provider); the work is carving them out of the
shell's chrome into a package the shell itself could consume back (one implementation of
the host contract, two skins over it — the proof it's really generic).

**Rejected — "every embedder vendors the full shell"** (the rubix-ai status quo): drags an
admin desktop app into products that don't want it, N diverging copies of the host
contract, and mobile-first is unreachable by subtraction. **Rejected — each product
hand-rolls a thin host:** the mount/CSS/auth contract would fork per product — exactly what
`defineRemote` was created to prevent on the extension side.

## How it fits the core

- **Tenancy / capabilities:** unchanged — the shell is a client; it logs in through the
  normal gateway, folds `reach:` caps like any client, renders only what the token allows.
  Deny path: an unreachable home extension renders the same "not available" state as the
  big shell (no cap probing).
- **Rule 10:** the shell reaches the extension only via `ext.list` + the federation mount;
  the configured id is data. A swap to a different extension is a config change, provably
  (test: mount the `hello` fixture ext by config only).
- **Placement / data / motion:** client-side only; SSE via the unified event stream; no new
  server surface except consuming the public branding route when it lands.
- **No mocks:** e2e = real node + a real published fixture extension (rule 9); no fake
  gateway.
- **SDK/WIT impact:** none on the Rust ABI. It **completes** the `@nube/ext-ui-sdk`
  contract host-side; version it in lockstep (`ui-v*`).

## Example flow (cc-app, the first consumer)

1. `ui/` = `create-minimal-shell` output + config: gateway URL, home = the care extension's
   page id, branding blob.
2. A guardian opens the PWA → branded login (pre-auth cache) → single workspace →
   full-screen care UI mounts via `defineRemote`'s host counterpart. No sidebar, no dock —
   the guardian never knows lb exists.
3. The product later swaps its UI extension id in config; the shell is untouched.

## Testing plan

Mandatory: **capability-deny** (token without the home ext's `reach:`/page caps → the
denied state, no mount), **workspace isolation** (two-ws identity: pick, correct scoping;
single-ws: skip). Plus: e2e login → mount the real `hello` fixture ext (Playwright, real
node), theme cascade reaches the ext (SDK isolation contract holds — host-byte-identical
check), PWA installability/manifest, SSE reconnect/resume, branding pre-auth cache paints
before login. No `*.fake.ts`.

## Risks & hard problems

- **Scope creep is the failure mode** — every feature request is "the full shell exists";
  the file budget + non-goals are the fence.
- **Two shells drifting on one contract** — mitigated by the full shell consuming the
  extracted core (or at minimum a shared contract-test suite both must pass).
- **Auth screens fork** (login/invite/branding × two shells) — extract those screens as the
  shared piece first; they're the most security-sensitive thing to fork.

## Open questions

- ✅ **Package** (`@nube/minimal-shell`, runtime dep, updates by tag bump) — vendoring is the
  disease this scope treats. (Rejected: devkit template — product owns the copy, drifts.)
- ✅ Invite-accept surface lives here — a themed client screen over `POST /public/invite/accept`
  (the `acceptInvite` function in `session.ts`). The shell provides the API; the product host
  adds the screen. Pre-auth locale: `GET /public/invite/verify` returns the invite's
  `{email, locale, redeemable}` so that screen renders in the invitee's language.
- ✅ **i18n (2026-07-11, release scope gap d):** every shell string flows through en+es catalogs
  (`src/i18n.tsx`) via the `@nube/ext-ui-sdk` seam (`resolveLocale`/`makeTranslator`/
  `catalogParity` — user pref → `navigator.language` → `en`); `src/i18n.test.tsx` is the CI
  key-parity gate (the TS twin of the `.mf` parity test). Extensions ship their own catalogs
  through the same SDK seam.
- ✅ Home = one ext page v1 (recommended). A bottom-tab multi-page mode is v1.5 or full-shell
  territory.

## Shipped (v1) + review-fix amendments (2026-07-11)

Shipped: `packages/minimal-shell` (~15 files) — login screen, invite-accept API
(`acceptInvite` in `session.ts`), `ext.list` discovery with an opaque `VITE_HOME_EXT`
config override (rule 10 holds: a swap is a config change), full-screen scoped mount via
`@nube/ext-ui-sdk`, refcounted SSE hub, theme-token provider, PWA manifest.

Review fixes applied in-place:
- **SSE subscribe was unauthenticated** — `POST /events/{sid}/subscribe` is header-authed
  on the gateway; `events.ts` sent no `Authorization`, so every subscription 401'd. Fixed
  (both the reconnect re-declare and `subscribe()`), see
  `docs/debugging/frontend/minimal-shell-sse-subscribe-401.md`.
- **401 left a stale UI** — `ipc.ts` cleared `lb.session` without notifying the session
  store; the app stayed "logged in" until reload. Fixed via a `lb.session.cleared` window
  event re-emitted by `session.ts` (regression test in `App.test.tsx`).
- **`getSession` snapshot loop** — a fresh `JSON.parse` object per call breaks
  `useSyncExternalStore` (`Object.is`) once a session exists; now cached by raw string
  (regression test).

Deferred honestly (named in Goals, not built — each needs a driver before it earns code):
- **Workspace pick for multi-ws identities** — v1 login asks for the workspace by name.
  (Rejected building it blind: the pick list needs a pre-auth "my workspaces" surface that
  doesn't exist yet.)
- **Branding (`ui_branding` blob + pre-auth cache) and boot-config fetch** — login is
  unbranded; the pre-auth cache pattern exists in the full shell and should be extracted,
  not re-written here.
- **Publishing** — the package is still `"private": true` with a `link:` dep on the SDK;
  the ✅ "published like the SDKs" decision stands but the `ui-v*` publish pipeline hasn't
  been wired.
- **Testing plan** — unit tests only (4, real jsdom render, no fakes). The mandatory
  capability-deny e2e (unreachable home ext → denied state) and the Playwright
  login→mount-the-`hello`-fixture run, PWA installability, SSE reconnect/resume, and
  branding pre-auth paint are open items; the deny path today renders the generic error
  state, not a distinct "not available" screen.

## Related

`../extensions/ext-out-of-tree-scope.md` · rubix-ai `docs/scope/ui/rubix-ui-scope.md` (the
vendoring decision this retires) · `../auth-caps/invites-scope.md` · workspace-branding /
login-branding work (prefs blob + pre-auth cache) · first consumer: `cc-app`
`docs/scope/ui/mobile-shell-scope.md`.
