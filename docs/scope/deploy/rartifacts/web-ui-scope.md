# rartifacts scope — web UI (federated extension pages on the minimal shell)

Status: scope (the ask). Slice 5 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

The operator console is the **`rartifacts` extension's own federated UI** — React +
TailwindCSS + shadcn/ui pages (the standard lb extension-UI stack, `@nube/ext-ui-sdk`
contract) mounted by the **lb minimal shell**
(`frontend/minimal-shell-scope.md`: auth + workspace pick + full-screen scoped mount
of a configured ext page). No second SPA host, no hand-rolled auth screens — the
node serves everything from one origin, and every button is a gateway call
(`pkg.*` MCP tools or the mounted routes) a curl could make with the same key.

## Goals

- `rartifacts/extensions/rartifacts/ui/` — Vite + React + TS + Tailwind v4 +
  shadcn/ui building `remoteEntry.js` (module federation), declared via the
  manifest's `[ui]` block, staged into the node's ext-UI dir (the shipped
  `RUBIX_EXT_UI_DIR`-style flow); fully offline bundle, no CDN.
- Pages (the extension's page + the shell's mount):
  - **Claim** (host-served, pre-auth — it *cannot* live behind the shell's login):
    6-digit boot-code field → the admin api-key revealed **once** (copy button,
    "never shown again"), then a pointer into the shell login. Post-claim visits
    render a paste-your-key card.
  - **Packages** — searchable table (name, kinds, latest, channels, owner,
    **visibility badge**); detail: version × arch matrix (digests, sizes), channel
    pointers, yank buttons, the **visibility toggle** (owner/admin; confirm dialog
    spelling out "public = anyone can download, no token"), config-schema + health
    spec read-only, `pkg_event` history. An anonymous visitor sees the public
    catalog read-only — the UI is honest about the anonymous tier.
  - **Publish** — metadata TOML + blob upload with progress (the manual door; CI
    uses curl), surfacing the slice-3 deny reasons verbatim.
  - **Channels** — promote/demote with version picker (non-yanked only), reason
    field, confirm dialog.
  - **Agents** (admin) — the live roster from `pkg.agent.list`: name, hostname,
    arch, rubixd version, `last_seen` (stale badged), **Revoke** (confirm; the
    remote kill switch). Register-agent flow → api-key shown once → drop into the
    box's `[[remote]] token_path`.
  - **Access** (admin) — publisher api-keys + registered pubkeys, mint/revoke via
    the shipped lb api-key verbs (reuse the platform's admin components where they
    exist rather than re-building).
- Auth plumbing: the shell's session (api-key pasted at login) rides every gateway
  call; 401 → login; 403 → honest "your key can't do this" (an agent key sees
  read-only truthfully).
- CI: `pnpm build` for the ui precedes the artifact pack; a `ui-dev` mode proxies
  Vite against a running node.

## Non-goals

- No second SPA host / rust-embed frontend (that was the pre-lb design — superseded).
- No fleet/agent-machine view (rubixd's own UI is per-machine; a fleet console is a
  future product). No human user management (api-keys are identity v1). No charts.

## Intent / approach

This is the payoff of the lb posture: login, session, theming, mounting, and the
capability-filtered surface come from the shell + gateway; the extension ships only
its pages. Dependency honesty: the **minimal shell is a scope, not shipped** — the
session must verify its status first; the recorded fallback is vendoring the ems
thin-shell (auth + ext mount) as a temporary host, swapped when minimal-shell lands
(tracked as debt in the session doc). Alternative rejected: standalone rust-embed
SPA (two auth systems, a second origin, re-built login — everything the platform
already owns).

## How it fits the core

The menu is never the permission model: pages render from `ext.list` discovery and
every action re-checks at the wall (403s surfaced, not hidden). Rule 10 intact: the
shell mounts an *opaque* configured page; nothing in lb names `rartifacts`.
FILE-LAYOUT applies to the `.tsx` tree (one page/component per file).

## Example flow

1. Fresh server behind TLS → `/claim`, boot code, admin api-key copied once → shell
   login with it.
2. Mint publisher `ci` + register its pubkey; register agent `site-alpha`, drop its
   key on the box.
3. Weeks later: bad 0.4.6 — packages page → demote `stable` (reason), yank 0.4.6,
   watch `pkg_event` confirm; agents page shows site-alpha's `last_seen` ticking;
   decommission day → Revoke.

## Testing plan

- No-new-verbs guard: the UI calls only existing tools/routes (route+tool table
  snapshot).
- Bundle guards: offline (`dist/` greps clean of external URLs), staged and served
  from the node, CSP intact.
- Browser smoke (headless, real node): claim happy path (+ wrong code error); login
  with pasted key; publish a small real artifact; visibility toggle; promote + yank;
  register agent → revoke → its key 401s live.
- Deny honesty: an agent-key session renders read-only *and* the forced action 403s
  (no client-side-only security).

## Risks & hard problems

- The minimal-shell dependency (above) — verify-first, vendored fallback recorded.
- Upload progress for multi-GB blobs in a browser: cap the form at a configured
  size and say so (curl exists); resumable browser upload is explicitly not v1.
- Claim page lives host-side, pre-auth — keep it a static page + one open route,
  zero shell coupling.

## Open questions

- Global activity page (`pkg_event` feed) vs per-package only — recommendation:
  global, it's one query.

## Related

[`token-auth-scope.md`](token-auth-scope.md) (claim/mint contracts) · lb
`docs/scope/frontend/minimal-shell-scope.md` (the mount host) · lb
`docs/scope/extensions/ui-federation-scope.md` + `extensions/ui/` contracts (theme
inheritance, CSS isolation — both apply to these pages) ·
[`../rubixd/embedded-ui-scope.md`](../rubixd/embedded-ui-scope.md) (the deliberately
minimal sibling).
