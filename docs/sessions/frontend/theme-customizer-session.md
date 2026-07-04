# frontend — theme customizer (session)

- Date: 2026-07-04
- Scope: ../../scope/frontend/theme-customizer-scope.md
- Stage: S9+ (frontend), STATUS.md
- Status: done (step 1 of 4 — theme-customizer, incl. Layout tab)

## Goal
Port the shadcn-store "Customizer" into the shell as the full theme control: light/dark, a
library of full theme presets (shadcn + tweakcn), radius, paste-to-import, per-token brand
overrides, **and the Layout tab (sidebar variant / collapsible / side)** — and make **every existing
token-driven surface (charts, panels, nav) re-theme live** by writing the project's **base** tokens
(not shadcn tokens). Persist per-member + per-workspace through the existing `prefs` verbs. Exit gate for this slice: the widened `ThemePreference` drives
base tokens end to end, presets/import/custom all round-trip, persistence rides prefs (member →
workspace-default → built-in), with the mandatory capability-deny + workspace-isolation tests green.

## What changed

### Rust — `ui_theme` prefs axis (persistence, no new verb/table/cap)
- `lb_prefs::Prefs` / `ResolvedPrefs` gain a nullable `ui_theme: Option<serde_json::Value>` axis —
  an opaque JSON blob holding the whole `ThemePreference`. Folds **whole** in `resolve()` (member's
  theme wins entirely over workspace default; not per-sub-field), matching the scope's
  member→ws-default→built-in intent. `builtin()` = `None`. Schema `option<object>` column added to
  both `user_prefs` + `workspace_prefs`; `PREFS_COLUMNS` projection updated. Dropped `Eq` from
  `Prefs`/`ResolvedPrefs` (`serde_json::Value` isn't `Eq`; `PartialEq` suffices — assert_eq! uses it).
- **Host + gateway need ZERO change**: `prefs.set`/`get`/`resolve`/`set_default` all parse into
  `lb_prefs::Prefs` via serde, and the axis is `#[serde(default)]` — so `ui_theme` flows through the
  MCP tool path (`prefs/tool.rs`), the gated verbs (`prefs/verbs.rs`), and the gateway HTTP routes
  (`routes/prefs.rs` deserializes the body straight into `Prefs`) untouched.

### TS — the theme layer (`ui/src/lib/theme/`), widened not forked
- `theme-options.ts`: `ThemePreference` = `{ mode, preset, radius, custom?, imported? }` — `accent`
  **hard-deleted** (three built-in presets amber/teal/blue replace it). `normalizeThemePreference`
  fails malformed/legacy records to `DEFAULT_THEME` (no compat upgrade).
- `theme-tokens.ts`: the base-token contract (`BasePalette`/`CustomTheme` + `BASE_TOKENS` metadata +
  structural validators).
- `color-to-hsl.ts`: any preset color (`#hex`/`oklch(…)`/`hsl(…)`) → the `"H S% L%"` triplet the base
  tokens use. Dependency-free (inline oklch→sRGB→hsl).
- `preset-adapter.ts`: **THE token bridge** — a shadcn-vocab preset → base tokens (`--primary`→
  `--accent`, `--background`→`--bg`, `--card`/`--popover`→`--panel`, `--border`/`--input`/`--ring`→
  `--border`, …). Load-bearing; the round-trip test is the regression guard.
- `theme-import.ts`: paste-to-import a tweakcn `:root{…}.dark{…}` block → `CustomTheme`, reusing the
  adapter; malformed → null (fail closed).
- `theme-presets.data.ts`: a curated 5-preset library (DATA, not branches).
- `theme-resolve.ts` + `theme-dom.ts`: resolve a preference to the active-mode palette and write it —
  **inline BASE tokens** for custom/imported/library presets (clearing `data-theme-accent`), or the
  `data-theme-accent` attribute for a built-in accent (clearing inline tokens). Plus `--radius`,
  `.dark`, `color-scheme`.
- `read-palette.ts`: read the live computed base palette off `<html>` (seeds Brand Colors; step-3
  `ctx.theme` will reuse it).
- `theme-prefs.ts` + `useThemePersist.ts`: read/persist the theme over `prefs.set`/`resolve`/
  `set_default` (`ui_theme` axis), with mount reconcile (authority = prefs, cache = localStorage) and
  debounced best-effort persist (denied → local-only, opaque).
- `ThemeProvider.tsx` / `theme-context.ts` / `index.ts`: new setter API (`setMode`/`setPreset`/
  `setRadius`/`setCustom`/`setImported`/`setTheme`/`reset`), `hydrated` flag.
- Client prefs types: `ui_theme?: unknown` added to `ResolvedPrefs` + `PrefsPatch` so the theme axis
  flows type-safely through the existing `prefs_*` invoke channel (no new client verb).

### TS — primitives + Customizer UI (Theme + Layout tabs)
- Hand-authored token-bound primitives (matching `switch.tsx` — no new Radix dep):
  `components/ui/{label,separator,accordion,color-picker}.tsx`.
- `features/theme/`: `Customizer.tsx` (Sheet + nav-footer trigger + **Theme|Layout tab switcher**),
  `ThemeTab.tsx` (presets/radius/mode/import/brand-colors + Reset + admin-gated "Set as workspace
  default"), `PresetPicker`/`RadiusPicker`/`ModeToggle`/`ImportField`/`BrandColors`. `ThemeSwitcher.tsx`
  rewritten to the preset API (kept as the compact quick-toggle). Mounted in `NavRail` footer.
- The admin "Set as workspace default" gates on `hasCap(session.caps, CAP.prefsSetDefault)` and calls
  `persistWorkspaceDefaultTheme` → `prefs.set_default`.

### TS — Layout tab (added after the scope's non-goal was found to be factually wrong)
- The scope originally deferred the Layout tab claiming "the shell does not use a shadcn Sidebar". It
  **does** — `NavRail` renders inside the shipped `components/ui/sidebar.tsx` (435 lines) via
  `SidebarProvider`, which already implements `variant`/`collapsible`/`side`; the `<Sidebar>` call just
  hardcoded them. Scope non-goal **reversed** and documented.
- `ThemePreference` gains `layout: {variant, collapsible, side}` (default `sidebar/icon/left`), riding
  the SAME opaque `ui_theme` prefs blob — zero backend change. `normalizeThemePreference` validates it
  (unknown axis → default, never partial). New `setLayout` setter on the provider/context.
- `NavRail` reads `useTheme().theme.layout` and spreads it onto `<Sidebar variant collapsible side>`,
  so the shell chrome re-lays-out live and the choice persists/roams.
- `features/theme/LayoutTab.tsx` + `layout/{OptionCard,SidebarMiniDiagram}.tsx`: the three picker
  groups (Sidebar Variant / Collapsible Mode / Position) with token-bound mini-diagrams, ported from
  the shadcn-store template UX.

## Decisions & alternatives
- **Chose: one new typed `ui_theme` axis on the closed `Prefs` record** over the scope's original
  "generic `ui.theme` key". Rejected the original because `lb_prefs::Prefs` is a **closed struct**
  with no key/value slot (`unit_overrides` is even a closed enum map — "never open free text"). One
  honestly-typed nullable axis riding the existing `set`/`set_default`/`resolve` verbs + caps honors
  the scope's real intent (no new verb/table/cap, per-member + workspace-default for free) without
  polluting the crate with an untyped grab-bag. Scope doc updated (Intent + two open questions).
- **Chose: hard-delete the legacy `{mode, accent}` shape**, no compat shim. Young project, no themes
  worth migrating; `accent` → three built-in `preset`s. Every caller updated in-slice; a legacy/
  malformed stored value normalizes to `DEFAULT_THEME`, nothing fancier. (Per user direction.)
- **Chose: presets write BASE tokens** (`--bg/--panel/--fg/--muted/--accent/--border`) as inline
  HSL-triplet overrides on `<html>`, letting `globals.css` derive shadcn tokens — the load-bearing
  fact. Rejected the literal template port (writes shadcn tokens inline) because it leaves every
  chart/panel/switcher on compiled defaults = half-themed app.

## Tests
Real infra, seeded via the real write path — no mocks, no fake backend.

**Rust `cargo test -p lb-prefs` — green (13 files):**
- `ui_theme_test` (6): blob round-trips through `option<object>`; a ui_theme-only patch leaves i18n
  axes untouched (MERGE); member theme wins WHOLE over ws default; ws default fills in for a member
  with none; nothing set → None; **workspace-isolation** (same user, different theme per ws, ws-B
  never reads ws-A's blob).
- `resolve_test` +1 (`ui_theme_folds_whole_first_link_wins`); `catalog_test` updated for the new field.

**Frontend `pnpm test` — 466 passed (74 files):**
- `preset-adapter.test` (4): shadcn preset → base tokens (light+dark, exact triplets); accepts
  oklch/hex/hsl; null on missing identity token; completes a sparse palette. **(the load-bearing
  regression guard — "existing UI re-themes")**
- `theme-import.test` (4): tweakcn `:root/.dark` → base tokens; `.dark`-absent reuse; hex/oklch;
  **fail-closed** on malformed/empty/non-string.
- `color-to-hsl.test` (4): hex/hsl/oklch → triplet; null on unparseable.
- `theme-dom.test` (5): built-in accent path (attr, no inline); custom writes inline base tokens +
  clears attr; **light↔dark re-applies the correct variant**; switching back to a built-in clears
  inline tokens; radius.
- `theme-storage.test` (5): new-shape load; unknown mode/radius→default (preset kept); **legacy
  {mode,accent}→default, no compat**; malformed fail; write-failure ignored.
- `ThemeProvider.test` (1): cache load → apply → change → cache persist (prefs unavailable in jsdom =
  the offline/denied path).
- `LayoutTab.test` (1): clicking Variant/Collapsible/Position cards drives `theme.layout`
  (sidebar→floating, icon→offcanvas, left→right) via a probe.
- `NavRail.test` +1: a seeded non-default layout reaches the DOM — the shadcn `<Sidebar>` reflects
  `data-variant="floating"` / `data-side="right"` (proves the LayoutTab→theme→NavRail→Sidebar loop).

**Frontend `pnpm test:gateway` — `theme-prefs.gateway.test` 5/5 green (REAL spawned node):**
- round-trip (persist → resolve/get, + fresh boot re-sign-in = theme roamed) — the round-tripped
  ThemePreference includes `layout`, proving the sidebar layout persists/roams through the real blob;
- workspace-default fold (admin `prefs.set_default` → member-with-none inherits; member-with-own wins);
- **capability-deny (mandatory)**: member without `mcp:prefs.set:call` denied on persist; non-admin
  without `mcp:prefs.set_default:call` denied on the workspace default (both `.rejects.toThrow()`);
- **workspace-isolation (mandatory)**: same identity's ws-A theme never resolves in ws-B;
- reset writes the explicit default.

`cargo fmt` clean; `tsc --noEmit` clean on all new files (2 pre-existing errors in untouched
`FlowsCanvas.gateway.test.ts` / `transformDebug.gateway.test.tsx`); `eslint src/lib/theme
src/features/theme` 0 errors.

**`cargo test -p lb-prefs -p lb-host`** — all lb-prefs + lb-host suites green EXCEPT one **unrelated
timing flake**: `control_engine_appliance_routing_test::appliance_record_routes_ce_patch_write_and_
offline_fails_loud` timed out (`ce.patch never became reachable: Elapsed`) under full-suite parallel
load, then **passed in isolation** (`cargo test -p lb-host --test control_engine_appliance_routing_test
<name>` → ok). Zero references to prefs/theme in that file; it's the known bus-timing flake class (memory
`flaky-bus-timing-tests`), not a regression from this change (which is a serde field on a struct + a
schema column + a resolve fold).

## Debugging
None yet.

## Public / scope updates
Scope `theme-customizer-scope.md` updated: persistence correction (structural `ui_theme` axis, not
key/value) + no-compat decision recorded; the "pref key + shape" and "new verb vs reuse prefs" open
questions RESOLVED in-doc. Promoted to `public/frontend/theme.md` (the shipped theme-customizer
truth). Remaining open questions (contrast policy, how much preset library, live cross-session sync,
locked-vs-suggested ws theme) left open — deferred as the scope intended.

## Skill docs
n/a: no new agent-/API-drivable surface — persistence reuses the existing `prefs` verbs (already
cataloged); the Customizer is a human-operated UI.

## Dead ends / surprises
- The scope's "just use `prefs.set` under key `ui.theme`" assumed a key/value prefs store that does
  not exist. Caught before coding by reading `rust/crates/prefs/src/prefs.rs`. Resolved with the
  structural `ui_theme` axis (scope updated).
- The shipped `select.tsx` is a NATIVE `<select>`, not the shadcn compound (`SelectTrigger/Content/
  Item`) — PresetPicker rewritten to native `<option>`s. The project hand-authors token-bound
  primitives instead of pulling Radix (per `switch.tsx`'s note), so accordion/label/separator/
  color-picker are hand-rolled, not `@radix-ui/*` installs.
- **`prefs.set_default` is NOT in the dev-login cap set** (`role/gateway/src/session/credentials.rs`
  `member_caps()`), and no `mcp:*.set_default:call` wildcard covers it — the code comments say the dev
  login "doubles as admin" but the cap was never added. So the positive workspace-default test grants
  it explicitly via `signInWithCaps`. **This will bite step-2 workspace-branding** (admin
  `prefs.set_default` for brand strings) — either add the cap to dev-login there, or seed an admin via
  `signInWithCaps`. Flagged for that session.

## Follow-ups
- Steps 2–4 (workspace-branding, theme-inheritance, css-isolation) are their own sessions. Step 3
  (`theme-inheritance`) will reuse `read-palette.ts` for `ctx.theme` and emit `lb:themechange` from
  `lib/theme` on apply (the emit seam is not yet added — `theme-dom` is the natural place).
- The `custom` theme's opposite-mode palette is seeded from the current computed palette until the
  user switches to that mode and edits — acceptable, but a future refinement could snapshot both modes
  at custom-start.
- Consider adding `mcp:prefs.set_default:call` to dev-login so the Customizer's admin control is
  exercisable end-to-end via `signInReal` (currently proven via explicit-cap grant).
- STATUS.md updated (slice row added).
