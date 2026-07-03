# Frontend scope — theme customizer

Status: scope (the ask). Successor to the shipped `theme-switcher-scope.md`. Promotes to
`public/frontend/frontend.md` once shipped.

> **Placement note.** The ask arrived pointed at `docs/scope/extensions/`, but theming is a
> property of the **shared UI shell** (README §6.13, the `frontend` topic), not an extension.
> Rule 10 forbids the core branching on an extension id, and rule 4 forbids durable state in an
> extension instance — a surface that restyles *core* design tokens for the whole app is core-shell
> work by definition. Extensions stay themed for free because they already inherit the host CSS
> variables (see `ui-federation-scope.md`); they are consumers of this layer, never owners of it. So
> this scope lives under `frontend/`. If we later let an extension *contribute* a preset, that preset
> is opaque **data** served through the registry — a separate, additive scope, not a fork of this one.

Port the "Customizer" theme surface from the `shadcn-dashboard-landing-template` into the Lazybones
shell: a slide-out panel with light/dark mode, a library of full **theme presets** (shadcn + tweakcn
packs), a **radius** control, **paste-to-import** of a tweakcn theme, and per-token **custom color**
overrides — replacing the current three-accent `ThemeSwitcher` as the *full* control while keeping a
compact quick-toggle. The hard requirement: **every existing UI element must re-theme live** — charts,
panels, dashboards, the nav rail, editor chrome — because they render from the design tokens, and a
naïve port would set the wrong tokens and leave them unchanged. Persist the richer choice **per member**
through the existing `prefs` surface (the deferral the switcher scope named), with local storage as the
first-paint cache.

## Goals

- Bring the template's Customizer UX into the shell: **Theme tab** (preset library + Random, radius,
  mode, import, custom "brand" colors) behind a slide-out `Sheet`, reachable from the nav/footer.
- Make preset selection drive the project's **base design tokens** so **all existing token-driven UI
  re-themes instantly** (the user's core requirement), with **zero** per-component color branches.
- Promote theme persistence from browser-only `localStorage` to **prefs** via the existing
  `prefs.get/set/resolve` MCP verbs — no new backend verb — keeping localStorage as an offline/first-paint
  cache so there is no theme flash on boot.
- Support a **per-workspace default theme**: an admin sets the workspace's default via the shipped
  admin-gated `prefs.set_default`, and each member may still override it for themselves — the two compose
  through the prefs resolve chain (member → workspace default → built-in), no new machinery.
- Preserve the shipped amber-default identity and the compact quick mode/accent toggle; the Customizer
  is the *superset* surface, not a rip-and-replace of the small control.
- Add the shadcn primitives the port needs (`accordion`, `label`, `separator`, a `color-picker`) under
  the project's token discipline, one responsibility per file.

## Non-goals

- **No Layout tab** in this slice. The template's Layout tab configures a shadcn `Sidebar`
  (variant / collapsible / side) the project does not use — the shell is `NavRail` + `StudioShell`.
  Sidebar-layout controls are deferred to `nav-rail-scope.md`, not smuggled in here.
- No new MCP tool, table, or capability *invented* for theming — persistence rides the existing `prefs`
  verbs and pref key. If that proves too coarse, a `theme`-specific verb is a follow-up, flagged below.
- No public/anonymous theme sharing, no cross-workspace theme marketplace, no extension-contributed
  presets (all additive later scopes).
- No redesign of the token *set*; we keep `--bg/--panel/--fg/--accent/--border/--muted/...` as the
  contract and only change how their **values** are chosen.
- No live font/shadow/spacing editor beyond what a pasted tweakcn theme already carries.

## Intent / approach

**The whole game is which tokens a preset writes.** The template applies a preset by setting the
*shadcn* tokens (`--primary`, `--background`, `--card`, …) as inline styles on `<html>`. Lazybones
inverts that dependency: in `styles/globals.css` the shadcn tokens are **derived** from a small base
palette —

```
--primary: var(--accent);   --background: var(--bg);   --card: var(--panel);   …
```

— and the app's own surfaces (charts via `features/charts/chartTheme.ts`, panels, `ThemeSwitcher`
swatches, editor chrome) read the **base** tokens `--bg/--fg/--accent/--panel/--border/--muted`
directly. So a port that writes `--primary`/`--background` inline would restyle shadcn buttons but leave
**every chart, panel, and base-token reader untouched** — exactly the bug the user is asking us to avoid.

The approach is a **token-bridge adapter**: a preset is normalized into **base-token values**, written to
`<html>` (inline overrides for a custom/imported theme; the static `:root`/`.dark` blocks remain the
default), and the existing CSS derivation cascades those into the shadcn tokens for free. One direction of
truth — base → shadcn — for both the built-in accents and any imported theme. Incoming tweakcn/shadcn
presets (which speak the shadcn vocabulary) are mapped **back** onto base tokens by the adapter
(`--primary` → `--accent`, `--background` → `--bg`, `--card`/`--popover` → `--panel`, `--muted` → `--muted`,
`--border`/`--input`/`--ring` → `--border`/`--accent`), with light/dark variants kept distinct.

The theme layer (`lib/theme`) already centralizes options, validation, DOM application, storage, provider,
and hook — we **extend** it, not fork it. `ThemePreference` grows from `{ mode, accent }` to
`{ mode, preset, radius, custom?, imported? }` (accent becomes one built-in preset among many);
`theme-dom.ts` gains base-token application from a resolved palette; `theme-storage.ts` keeps its
localStorage fallback but the provider now **reads/writes the member's `prefs` record** as the authority.

**Rejected alternative — apply shadcn tokens directly (a literal port).** Simplest to copy, but it
severs the base-token cascade: charts/panels/switcher read `--accent`/`--bg`/`--panel`, which the preset
never sets, so they stay on the compiled defaults while buttons change — a visibly half-themed app. The
adapter is a little more code once, versus a permanent per-component divergence. Rejected.

**Per-workspace default + per-member override — for free from the prefs chain.** The `prefs` crate
already resolves `request override → user pref → workspace default → built-in fallback`, each axis
independently. So theming needs **no new precedence logic**: an admin writes the workspace's default
`ui.theme` with the admin-gated `prefs.set_default` (it targets the `workspace_prefs:[ws]` record), a
member writes their own `ui.theme` with `prefs.set`, and `prefs.resolve` folds them so the member's
choice wins where set and the workspace default fills the rest. A workspace can thus ship a branded
house theme that every member sees on first load, while still letting individuals opt into light/dark or
their own accent — unless we later choose to *lock* the workspace theme (see Open questions). The
Customizer surfaces this as two levels: a normal member view, plus an **admin-only "set as workspace
default"** action gated on `mcp:prefs.set_default:call`.

**Rejected alternative — keep persistence in `localStorage` only (as the switcher shipped).** Fails the
platform's one-datastore / workspace-first posture and won't roam between a user's browser and desktop
shell. The switcher scope explicitly deferred synced prefs to the `prefs` topic; this scope collects that
debt. localStorage stays only as a first-paint cache to avoid a boot flash.

## How it fits the core

- **Tenancy / isolation:** the theme pref is a **per-member** record, and `prefs.get/set/resolve` are
  already workspace-scoped and read/write **own** prefs only — so a member's theme cannot read or write
  across the workspace wall. Isolation is inherited from the `prefs` crate, and its test proves it here.
- **Capabilities:** writing the member's own pref requires `mcp:prefs.set:call`; reading requires
  `prefs.get`/`prefs.resolve`; setting the **workspace default** theme requires the admin-gated
  `mcp:prefs.set_default:call`. A member without `prefs.set` cannot persist a personal theme (the UI
  degrades to local-only, opaque deny); a non-admin cannot set the workspace default (the "set as
  workspace default" action is hidden/denied). No new grant is minted — theming reuses the shipped prefs
  grants and their existing deny paths.
- **Symmetric nodes:** none of this branches on cloud vs edge. The same React shell runs in the browser
  (SSE/HTTP to the gateway) and the Tauri webview (local host); storage prefers `prefs`, then
  `localStorage`, then compiled defaults — config/role, never `if cloud`.
- **One datastore:** the durable theme lives in the member's **SurrealDB** prefs record via the existing
  verbs. No new table, no separate settings store. localStorage is a cache, not a source of truth.
- **No mocks / no fake backend:** the persistence path is tested against a **real** spawned gateway
  (`pnpm test:gateway`) hitting the real `prefs` verbs on the real store — no `*.fake.ts`, seeded with a
  real member. Pure-frontend token/adapter tests stay in `pnpm test`.
- **State vs motion:** theme is **state** (a member preference in SurrealDB), not motion — no Zenoh
  subject. A theme change is a local render plus one `prefs.set`; other sessions pick it up on next
  `prefs.resolve`, not via a live push (see Open questions for whether that's worth a watch later).
- **Stateless extensions:** unchanged. Extensions own no theme state; they inherit host CSS variables at
  render, so they re-theme with the shell automatically. No extension-id branch is introduced.
- **MCP is the contract:** persistence is expressed as the existing `prefs` MCP tools under a reserved
  pref key (e.g. `ui.theme`) — the same verbs the UI, agents, and other callers already use.
- **API shape (§6.1):** **no new verbs.** *Get/list* — read the member's theme via `prefs.get`/
  `prefs.resolve` (single key; no list needed). *CRUD* — the only write is `prefs.set` on the theme key
  (update-in-place; delete = reset to default, done client-side). *Live feed* — **N/A** for this slice;
  theme is not motion and a single member rarely themes two live sessions at once (deferred, see below).
  *Batch* — **N/A**; one member, one key, always a fast single write.
- **Durability:** N/A. The theme write has no cross-node must-deliver side effect, so it does **not** go
  through the outbox — it is a plain, own-scoped `prefs.set`.
- **One responsibility per file:** the port is decomposed under FILE-LAYOUT — `lib/theme/` keeps
  options / validation / dom-apply / preset-adapter / storage / prefs-sync / provider / hook as separate
  files; `features/theme/` holds `Customizer` (sheet shell), `ThemeTab`, preset picker, radius picker,
  import parser, and color pickers as separate components; preset packs are **data** files, not code
  branches. No file exceeds the 400-line hard cap.
- **SDK/WIT impact:** none. The plugin boundary and host-callback ABI are untouched; extensions keep
  inheriting host CSS variables exactly as today.

## Example flow

1. A member signs in. `ThemeProvider` mounts and paints from the **localStorage cache** immediately
   (no flash), then calls `prefs.resolve` for `ui.theme`; if the stored record differs, it reconciles to
   the member's authoritative preference and updates the cache.
2. The member clicks the Customizer trigger in the nav footer; the `Sheet` slides in with the **Theme
   tab** active.
3. They pick a tweakcn preset from the library. The **preset adapter** maps the preset's shadcn-vocabulary
   light/dark styles onto the project's **base tokens** and writes them as inline overrides on `<html>`.
4. The CSS derivation cascades base → shadcn tokens; **charts, panels, dashboards, nav rail, and editor
   chrome all re-render in the new palette at once** because they read the base tokens.
5. They nudge **radius** to `0.75rem` and open **Brand Colors** to hand-tweak `--accent`; each change is a
   live inline write, previewed instantly.
6. On change (debounced), the provider persists the resolved `ThemePreference` via `prefs.set` under
   `ui.theme` **and** updates the localStorage cache.
7. The member opens the desktop Tauri shell later; `prefs.resolve` returns the same theme and the app
   boots into it — the preference roamed.
8. They hit **Reset**; the provider clears inline overrides and the custom/imported fields, falling back
   to the default preset (amber), and persists the reset.

## Testing plan

Both the pure-frontend categories **and** — because persistence now touches the real store — the
mandatory platform categories from `scope/testing/testing-scope.md` apply.

- **Preset adapter (unit, `pnpm test`):** a tweakcn/shadcn preset maps to the correct **base** tokens for
  both light and dark; round-tripping a known preset yields the expected `--bg/--fg/--accent/--panel/...`
  values. This is the regression guard for "existing UI re-themes."
- **DOM application (unit):** applying a resolved palette sets the base-token inline styles + `.dark`
  class + radius on `<html>`; Reset removes them and restores defaults.
- **Import parser (unit):** pasting a tweakcn CSS block parses `:root`/`.dark` into `{ light, dark }` base
  tokens; malformed input fails closed to the current theme (no partial apply).
- **Preference validation/persistence (unit):** the widened `ThemePreference` normalizes unknown
  preset/radius/mode to defaults; localStorage fallback still works when storage is unavailable.
- **Prefs round-trip (real gateway, `pnpm test:gateway`):** against a **real** spawned node, a seeded
  member sets `ui.theme` via `prefs.set` and reads it back via `prefs.resolve`; a second boot restores it.
  **No fake backend.**
- **Capability deny (mandatory):** a member token **without** `mcp:prefs.set:call` is denied on persist
  (opaque deny) and the UI stays local-only; a **non-admin** token is denied on `prefs.set_default`
  (workspace-default theme) — assert both honest denies, not a silent success.
- **Workspace-default resolve (real gateway):** an admin sets `ui.theme` via `prefs.set_default`; a
  member with no personal theme resolves to that default, and a member *with* a personal theme resolves
  to their own — proving the member → workspace-default → built-in fold on the real store.
- **Workspace isolation (mandatory):** member A's `ui.theme` is invisible/unwritable to member B / another
  workspace via the prefs path; seed two real members and assert no cross-read.
- **Component/interaction (`pnpm test`):** the Customizer sheet opens, tabs switch, preset select + radius
  + color pickers fire the right handlers, Random and Reset behave; controls expose labels/`aria` and
  focus (extend the existing `ThemeSwitcher.test.tsx` discipline).
- **Build/lint:** `pnpm build`, `pnpm lint`, `cd rust && cargo test --workspace` stays green
  (no Rust change expected; the prefs verbs already exist).

## Risks & hard problems

- **The token bridge is the feature.** If a preset writes shadcn tokens instead of base tokens, the app
  looks half-themed and the user's core requirement silently fails. The adapter + its round-trip test are
  the load-bearing piece; treat a missing/weak adapter test as a blocker, not a nicety.
- **Contrast drift across a preset library.** The shipped three accents are hand-verified AA. An imported
  or random tweakcn preset can produce accent-on-bg combinations below AA. Decide the policy: warn,
  auto-adjust, or accept-as-user-choice (see Open questions) — and keep body text bound to `--fg`, never
  the accent.
- **First-paint flash vs. authority.** localStorage must paint first for speed, but `prefs` is the
  authority; the reconcile must be flicker-free and must not clobber a just-made local change with a
  slower `prefs.resolve` (last-writer/debounce discipline).
- **Missing primitives under token discipline.** `accordion/label/separator/color-picker` aren't in
  `components/ui`. They must be added shadcn-style, wired to the project tokens (per
  `ui-library-css-rules` — scope utilities, alias host tokens, no preflight) or they'll render invisibly
  in jsdom/tests and drift from the design system.
- **Custom/imported overrides vs. mode switch.** When a member is on a custom theme and flips light↔dark,
  the correct light/dark variant of *that* custom theme must re-apply (the template re-applies on
  `isDarkMode` change) — regression-test the interaction.
- **Radius as a global.** `--radius` is one inline global; confirm no surface hard-codes a corner radius
  outside the token, or the radius control will look inconsistent.

## Open questions

- **Pref key + shape.** Reserve `ui.theme` (recommended) vs a namespaced `frontend.theme`? Store the whole
  `ThemePreference` JSON as one pref value, or split mode/preset/radius into sibling keys? Recommendation:
  one key, one JSON value — atomic, one write, easy reset. Confirm against the `prefs` value-shape rules.
- **How much of the preset library ships.** The template bundles large shadcn + tweakcn preset packs.
  Ship the full packs as data, or a curated subset that we've contrast-checked? Recommendation: curate a
  vetted subset for the built-in library, keep **Import** for the long tail.
- **Contrast policy** for imported/random presets: warn-only, auto-nudge accent to AA, or accept as the
  member's explicit choice? (Leaning warn-only — it's their workspace.)
- **New verb vs. reuse `prefs`.** Is a single JSON blob under `prefs` enough, or do we want a first-class
  `theme.get/set` for validation/versioning? Recommendation: reuse `prefs` now; promote to a dedicated
  verb only if migration/versioning pain shows up.
- **Live cross-session sync** (a `watch`/live feed so two open sessions of the same member converge in
  real time) — worth it, or is next-load reconcile enough? Deferred; revisit if users run split shells.
- **Does the compact `ThemeSwitcher` stay?** Recommendation: keep it as the quick mode/accent control in
  the collapsed rail; the Customizer is the full surface. Both write the same theme layer.
- **Locked vs. suggested workspace theme.** Can an admin *force* the workspace theme (members cannot
  override), or is the workspace theme only a **default** members may override? Recommendation: default
  only (member choice wins) — a lock is a follow-up flag, not this slice. This is distinct from the
  workspace **branding** (logo/favicon/site name), which is admin-owned and *not* member-overridable —
  scoped separately in `workspace-branding-scope.md`.

## Related

- `theme-switcher-scope.md` — the shipped predecessor this supersedes; it deferred synced prefs to
  `prefs/` and named the base-vs-shadcn token compatibility risk this scope resolves.
- `workspace-branding-scope.md` — the sibling that owns admin workspace **branding** (logo, favicon,
  site/login heading). Theme = per-member preference with a workspace default; branding = admin-only
  workspace identity. They share the workspace-default prefs seam but differ in ownership and in the
  pre-auth login-rendering problem, so they are separate scopes.
- `../prefs/user-prefs-scope.md` — the resolve chain (`user → workspace default → built-in`) and
  `prefs.set_default` (admin) that the per-workspace theme rides.
- `../workspace/workspace-scope.md`, `../tenancy/tenancy-scope.md` — the workspace-as-tenant wall the
  workspace-default theme is scoped by.
- `../prefs/*` and `../../public/prefs/prefs.md` — the `prefs.get/set/resolve` surface persistence rides;
  README §6.6 (identity/caps) for the grant that gates `prefs.set`.
- `ui-standards-scope.md`, `ui-design-scope.md` — shadcn-first control layer, token discipline, and the
  dark-first amber operator-console direction the default must preserve.
- `ui-federation-scope.md` — why extensions re-theme for free (host CSS-variable inheritance) and stay
  out of this layer.
- `nav-rail-scope.md` — where the deferred Layout/sidebar controls belong if we want them.
- `../../FILE-LAYOUT.md` §4 — the one-component / one-hook / data-not-branches decomposition the port
  must follow.
- Source template: `shadcnstore/shadcn-dashboard-landing-template` (`vite-version`,
  `src/components/theme-customizer/*`, `src/hooks/use-theme-manager.ts`) — the UX to port; the token
  application is deliberately **not** copied verbatim (see Intent).
- **Skill doc:** N/A. This adds no new agent-/API-drivable surface — persistence reuses the existing
  `prefs` verbs, which already carry their catalog/skill entry. The Customizer itself is a
  human-operated UI, not an automatable task.
</content>
</invoke>
