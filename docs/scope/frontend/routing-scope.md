# Frontend scope — URL routing (shareable, deep-linkable pages with typed args)

Status: scope (the ask). Promotes to `public/frontend/frontend.md` once the shell is on the router.

The shell currently navigates by `useState` (`surface`, `channel`, …) in `App.tsx`, so there is
**no URL to share, no back/forward, no deep-link**. We want real routes that work identically in
the **browser** and the **Tauri desktop webview**, so a user can copy the address bar to share an
exact page *with its arguments* — e.g. a dashboard scoped to a date range
(`#/dashboards?from=2026-01-01&to=2026-03-31`). The router is **@tanstack/router in hash mode**,
chosen for **type-safe, schema-validated search params** (the date-range/filter args are the whole
point), with the hash strategy making the same URLs valid in both shells.

## Goals

- Every navigable surface has a **URL**: the ~11 core surfaces, the channel selection, and
  extension pages (`ext:<id>`) all become addressable paths.
- **Shareable args as typed search params.** Page arguments (dashboard `from`/`to`, channel name,
  future filters/paging) live in the URL as validated query params, not React state.
- **Both shells, one grammar.** The same `#/...` URL resolves in the browser address bar and the
  Tauri window. "Share this page" = copy the current URL.
- **Back/forward + deep-link/reload** land on the exact page+args.
- Replace `App.tsx`'s `useState` surface/channel switching with the router as the single source of
  navigation truth — without changing the look (shadcn shell stays as-is) or weakening cap-gating.

## Non-goals

- Server-side routing / SSR / a dev-server rewrite rule. Hash mode means none is needed.
- Pretty path URLs (`/dashboards` without the `#`). Deferred — the Tauri custom-protocol webview
  makes history/path mode fragile; revisit only if a real need appears (see Open questions).
- New product surfaces. This is the navigation substrate for the pages that already exist.
- Auth/session changes. Logged-out still short-circuits to `LoginView` before any route renders.
- Persisting per-user "last route" server-side (the URL itself is the state).

## Intent / approach

Adopt **@tanstack/router** with `createHashHistory()` so all routes live under the URL fragment.
The fragment is the one URL part that is stable across both runtimes: the browser handles `#`
natively, and the Tauri webview (served from a custom `tauri://`/`asset://`-style protocol where
path-based history is unreliable) treats the fragment as in-page navigation, so no protocol/route
config is required. TanStack's `validateSearch` (a Zod-or-plain schema per route) gives us typed,
defaulted, validated search params — exactly what the dashboard date-range needs so a malformed
shared link degrades to sane defaults instead of crashing.

**Rejected alternatives:**
- *react-router (hash mode)* — works, but search params are stringly-typed; we'd hand-roll the
  parse/validate the date-range needs. TanStack bakes that in.
- *A tiny hand-rolled `useHashRoute` hook (no dep)* — fine for flat surface switching, but we'd
  reinvent typed/validated search params, nested loaders, and history ergonomics the moment args
  appear. The args are the requirement, so the dep earns its place.
- *Path/history mode* — blocked by the Tauri custom-protocol webview; hash sidesteps it entirely.

### URL grammar (the contract)

```
#/                              → redirect to the token's workspace + default surface
#/t/<ws>/                       → redirect to the default surface (channels) in <ws>
#/t/<ws>/channels?c=<name>      → ChannelView; `c` selects the channel (default "general")
#/t/<ws>/members
#/t/<ws>/dashboards?from=<iso>&to=<iso>   → DashboardView scoped to a date range (both optional)
#/t/<ws>/ingest
#/t/<ws>/data
#/t/<ws>/system
#/t/<ws>/inbox
#/t/<ws>/outbox
#/t/<ws>/admin
#/t/<ws>/extensions
#/t/<ws>/ext/<id>               → a federated extension page (the `ext:<id>` surface today)
```

- **Every surface lives under a `/t/<ws>` tenant prefix.** Surface ↔ tenant-relative path is 1:1
  with today's `Surface` type (`CoreSurface | ext:${string}`); `ext:<id>` becomes `/ext/<id>`.
- **Args are search params**, per-route schema-validated with defaults. Unknown/invalid params are
  dropped to defaults, never thrown — a shared link must always render.
- **The `<ws>` segment is a deep-link hint AND a guard — never an authorization input.** It makes a
  shared URL self-describing (you can see which workspace a link points at), but it does **not**
  select the workspace. The `/t/$ws` layout route's `beforeLoad` compares `<ws>` to the verified
  session's workspace: if they differ, it **rewrites the URL** to the recipient's own workspace
  (same surface + args) and renders that — it never fetches the URL's `<ws>`. The gateway re-derives
  the real workspace from the verified token regardless (§7), so even a hand-forged `/t/<ws>` that
  bypasses the client guard reads only the token's workspace server-side. Switching workspace is
  still a re-login (`switchWorkspace`), which mints a new token and lands on `/t/<new-ws>/…`.
- **Tenant-less links degrade gracefully.** A bare `#/dashboards?from=…` (e.g. an old link minted
  before this grammar) is caught by the root not-found/index fallback, which grafts the token's
  workspace on (`→ #/t/<token-ws>/dashboards?from=…`) — preserving the surface and args.

## How it fits the core

- **Tenancy / isolation (§7):** the workspace appears in the path as `/t/<ws>` but is **not an
  authorization input** — it is a display/deep-link hint plus a client-side guard. The real
  workspace always comes from the verified session token: the `/t/$ws` route guard rewrites any
  mismatched `<ws>` to the recipient's own, and the gateway re-derives the workspace from the token
  on every verb regardless. A pasted (or hand-forged) URL therefore never selects another tenant's
  data — client-side it's redirected, server-side it's ignored. This is the security-relevant
  decision of this scope. (Earlier draft kept the workspace fully out of the URL; we moved it into a
  *guarded* path segment so links are self-describing without weakening the §7 wall.)
- **Capabilities:** routing is convenience-only, same as today's `allowed[]` gating. A route to a
  surface the session can't see (e.g. `#/admin` for a non-admin) **redirects to the default surface**
  rather than rendering — but this is cosmetic; the gateway already denies the underlying verbs
  (proven in `role/gateway/tests/admin_routes_test.rs`). Routes must reuse the existing `allowed`/cap
  checks, not invent a parallel gate.
- **Placement / symmetric nodes (§3 rule 1):** the router is shell-side and shell-agnostic — one
  hash-history config runs in both Tauri and browser with no `if (isTauri)` branch. This is the
  symmetry rule applied to the frontend: edge vs cloud, desktop vs browser = config, not a code fork.
- **MCP surface:** N/A — routing is pure client navigation; it calls no new tools and changes none.
  Pages keep calling their existing verbs through the one IPC seam (`lib/ipc`).
- **Data / bus / durability / secrets:** N/A — no records, no subjects, no secret material. The URL
  is ephemeral client state.
- **UI standard (`ui-standards-scope.md`):** the router slots *under* the shadcn shell —
  `SidebarProvider`/`NavRail`/`SidebarInset` stay; route components render where the
  `active === "…"` switch is today. `NavRail`'s `active`/`onSelect` become router link state. No
  conflict with the shadcn-first or responsive rules.
- **SDK/WIT impact:** none — routing is not the plugin boundary. Extension pages are addressed by
  `/ext/<id>` but the federation/bridge contract is untouched.

## Example flow

Share a dashboard date-range, recipient on a different machine/shell:

1. User A opens Dashboards, picks Jan–Mar 2026. The view writes the range to search params →
   address bar reads `#/t/acme/dashboards?from=2026-01-01&to=2026-03-31` (A is in workspace `acme`).
2. A clicks the page's **Share** affordance (the `Share2` icon already in `DashboardView`) → it
   copies `window.location.href` (tenant prefix included — the link names workspace `acme`).
3. User B (in the **browser**; A was in **Tauri**), whose token is for workspace `beta`, pastes the
   URL. The `/t/$ws` guard sees `acme ≠ beta` and **rewrites** to `#/t/beta/dashboards?from=…&to=…`
   — same surface + args, B's own workspace. `validateSearch` parses `from`/`to`; `DashboardView`
   opens scoped to that range.
4. B's session token sets B's workspace (`beta`); the gateway re-checks B's dashboard-read caps and
   reads only `beta`'s data. The link shared *the view+args*, never A's workspace or authority —
   the `acme` segment was a hint the guard corrected, not a key.
5. B hits back → returns to B's prior route. Reloads → same dashboard+range (URL is the state).

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply (real node, no fakes —
CLAUDE §9; the `*.gateway.test.tsx` harness already mounts the shell against a real seeded gateway):

- **Route → render (real gateway).** Mount the app at a given hash (`#/members`, `#/dashboards?from=…&to=…`)
  and assert the right surface renders with args applied — through the real gateway, not a mock.
- **Search-param validation.** A malformed/garbage `?from=` resolves to the default range, never
  throws — the "a shared link always renders" guarantee, unit-tested on the route schema.
- **Capability-deny (mandatory).** A session lacking a surface's cap that deep-links to it
  (`#/admin`) is redirected to the default surface AND the underlying verb is gateway-denied — reuse
  the existing admin-route deny test as the server half.
- **Workspace-isolation (mandatory).** A pasted URL carries no workspace; assert the rendered data
  is the *recipient's* workspace, proving the URL can't cross the §7 wall.
- **Both-shell parity.** The hash route resolves under the Tauri webview origin and the browser
  origin — at least a unit check that the history config is hash-based (no path assumptions), since
  the E2E desktop window isn't in CI.
- **Back/forward + reload** land on the correct route+args.

## Risks & hard problems

- **Tauri webview URL handling** is the crux — confirm hash navigation works under the desktop
  protocol early (a spike before converting all surfaces). The whole router choice rests on it.
- **State→URL migration in `App.tsx`.** `surface`/`channel`/extension-page selection all move into
  routes at once; the cap-gating (`allowed[]`), the logged-out short-circuit, and the stable
  hook-order (`useExtensionPages` called unconditionally) must be preserved exactly.
- **Extension page routes.** `/ext/<id>` must resolve only to *installed, cap-visible* pages
  (today's `extPages.find`), and unknown ids redirect — not render a broken `ExtHost`.
- **Search-param schema drift.** Each page owns its arg schema; keep them colocated (FILE-LAYOUT)
  so a page and its URL contract don't diverge.
- **Bundle/setup cost** of TanStack vs the no-dep hook — accepted for the typed-args payoff; keep the
  route tree small and flat (no premature nesting).

## Open questions

- ~~**`?ws=` workspace hint.**~~ **Resolved:** the workspace is now an explicit, *guarded* path
  segment (`/t/<ws>`) rather than a query hint — self-describing links, but the segment is never
  trusted (the `/t/$ws` guard rewrites a mismatch to the recipient's workspace; the gateway re-checks
  from the token). Still open: should a mismatched `<ws>` *offer* a switch-workspace (re-login) flow
  ("this link is for `acme` — switch?") instead of silently rewriting to the current workspace?
  (Lean: silent rewrite for v1 — safest; an explicit switch prompt is a follow-up if cross-workspace
  sharing becomes common.)
- **Path mode later?** Is a prettier `/dashboards` URL ever worth solving the Tauri custom-protocol
  history problem (a protocol handler / asset rewrite), or is hash permanent? (Lean: hash permanent.)
- **Which args graduate to the URL now** beyond dashboard `from`/`to` and channel `c`? (e.g. data
  console table selection, inbox filter.) Start with dashboard + channel; add per page as needed.
- **Share affordance scope.** Just the dashboard's existing `Share2`, or a shell-level "copy link"
  in `AppPageHeader` available on every surface? (Lean: shell-level — every page is shareable.)
- **`validateSearch` impl:** plain hand-written validators or pull in Zod? (Lean: start plain; Zod
  only if schemas grow.)

## Related

- `scope/frontend/ui-standards-scope.md` — the shadcn shell the router slots under (no conflict).
- `scope/frontend/frontend-scope.md` — the one-IPC-seam shell (Tauri | HTTP) this must stay
  symmetric across; `scope/frontend/dashboard-scope.md` — the date-range args' first home.
- `scope/extensions/ui-federation-scope.md` — the `ext:<id>` → `/ext/<id>` pages.
- Canonical code today: `ui/src/App.tsx` (the `useState` switch this replaces),
  `ui/src/features/shell/NavRail.tsx` (`Surface`/`active`/`onSelect`),
  `ui/src/features/dashboard/DashboardView.tsx` (the `Share2` affordance + `ws` prop).
- README `§3` rule 1 (symmetric nodes — applied to desktop vs browser), `§7` (workspace hard wall).
