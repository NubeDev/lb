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

- Package (`@nube/minimal-shell`, runtime dep, updates by tag bump) vs devkit **template**
  (scaffold, product owns the copy)? Recommend: **package with a thin config entry** —
  vendoring is the disease this scope treats.
- Does the invite-accept surface live here or gateway-served? (Coordinate with
  `invites-scope.md`; recommend here — it's a themed client screen over one public verb.)
- Home = one ext page v1 (recommended) — is a bottom-tab multi-page mode (N pages of the
  *same* ext) v1.5 or full-shell territory?

## Related

`../extensions/ext-out-of-tree-scope.md` · rubix-ai `docs/scope/ui/rubix-ui-scope.md` (the
vendoring decision this retires) · `../auth-caps/invites-scope.md` · workspace-branding /
login-branding work (prefs blob + pre-auth cache) · first consumer: `cc-app`
`docs/scope/ui/mobile-shell-scope.md`.
