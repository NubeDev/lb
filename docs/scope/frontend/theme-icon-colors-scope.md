# Frontend scope — sidebar icon colors

Status: shipped (2026-07-05). Promoted to `public/frontend/frontend.md`.

Add a per-sidebar-icon color preference to the shared UI shell: a member opens Settings → Theme,
hits **Auto-assign**, and every rail icon gets a distinct color drawn from a **prefilled 100-color
palette**. Each icon is then individually editable (swatch grid + custom hex). The choice persists
and roams through the same `ui_theme` prefs blob the rest of the customizer rides — **zero backend
change**, no new verb/cap/table.

## Goals

- Ship a **Settings → Theme → Icon colors** control that colorizes the sidebar icons.
- Provide a **prefilled 100-color palette** as the source pool — deterministic, frozen, data-not-branches.
- **Auto-assign** one palette color per rail surface, evenly hue-spread so any subset of the rail
  looks rainbow-distributed rather than clustered.
- Allow **per-icon override** (palette swatch or custom hex) and **Clear all** (fully reverts to
  default-fg icons).
- Persist through the existing `ui_theme` blob; ride the existing prefs verbs + cap gates.

## Non-goals

- No per-*extension* (`ext:<id>`) icon color in the picker UI this slice — extension slots are
  dynamic and not enumerated by `RAIL_SURFACES`. The map is keyed by string, so a future per-ext
  surface is additive (set `iconColors["ext:foo"]`); the UI simply doesn't list it yet.
- No contrast guard/AA enforcement on icon colors — they're decorative accents on the rail surface,
  not body text. (A future contrast nudge could reuse `lib/theme/contrast.ts`.)
- No workspace-level *lock* of icon colors — same posture as the rest of the customizer (workspace
  default + member override through the prefs resolve chain).
- No animation/transition on color change beyond the shadcn primitives' default.

## Intent / approach

**Everything is still one `ui_theme` blob — zero backend change.** A new optional
`iconColors?: Record<string, string>` axis joins `ThemePreference`. **Presence === ON, absence ===
OFF**: the first set on a fresh theme both enables colorization and assigns the one color; "Clear
all" deletes the field entirely. Normalization validates each entry as a canonical `#rrggbb` hex
and drops malformed entries **per key** (fail-closed per key, never whole-blob), keeping the
"presence === ON" contract honest. (Rejected: a sibling boolean `iconColorsEnabled` flag — redundant
with presence and another field for normalization to wrangle.)

**A prefilled 100-color palette as DATA, not a literal list.** `lib/theme/icon-colors.data.ts`
builds the palette once at module load via a **golden-angle hue spread** (≈137.5°) with two
alternating saturation/lightness profiles, converted to hex and frozen. Consequence: any two
adjacent palette indices are perceptually far apart, which is what makes a small even sample of the
palette look "rainbow-distributed" rather than clustered. The palette is the single source for
"what colors exist to pick from" — both the auto-assigner and the swatch grid consume it. Update
the palette by editing `buildPalette`, never by mutating the frozen export. (Rejected: a hand-typed
100-row literal — harder to keep coherent, no perceptual structure, and "prefilled" is satisfied by
a deterministic generator whose output is frozen data.)

**Auto-assign evenly samples the palette across whatever keys the rail currently shows.**
`autoAssignIconColors(keys)` picks indices `round(i * 100 / n)` for `i` in `0..n-1`, so 5 icons get
5 colors evenly spread across the full hue range, not 5 clustered neighbors. Pure and deterministic:
the same keys always map to the same colors, so a reload doesn't reshuffle the rail. The picker
calls it with `RAIL_SURFACES.map(s => s.key)` — the flat list of rail entries exported from
`NavRail`, the single source of truth for "what surfaces exist." (Rejected: hash-the-key-to-index —
order-dependent shuffling on every surface-list change; even sampling reads as "designed.")

**Application is one inline `style`.** `NavRail.item()` reads `theme.iconColors?.[key]` and, when
present, renders `<Icon style={{ color }} />`. Inline `color` wins over the button's `text-*`
classes without specificity fights, and lucide `<svg>` uses `currentColor` so the icon picks it up
for free. No-op when colorization is OFF (no inline style, today's default-fg behavior). This works
in both the expanded and collapsed (icon-only) rail because the same `<Icon>` renders in both
modes.

**In-DOM popover, not a native color input.** `IconColorSwatch` mirrors the shipped
`color-picker.tsx` discipline: the whole row is the trigger, the editor is a hand-authored popover
(a 10×10 swatch grid + a hex field), outside-click/Escape dismissal owned in-component. Why: the
native `<input type="color">` is a silent no-op on WebKitGTK (the Tauri Linux webview) — the same
shipped bug the appearance scope flags for the brand-color picker. No new dependency; works
identically on every engine.

## How it fits the core

- **Tenancy / isolation:** unchanged from the customizer — the whole preference is the member's own
  `ui_theme` prefs axis; workspace isolation is inherited from the `prefs` crate and re-proven by
  the existing mandatory tests.
- **Capabilities:** unchanged — `mcp:prefs.set:call` to persist own, admin-gated
  `mcp:prefs.set_default:call` for the workspace default. No new grant.
- **Symmetric nodes / placement:** UI-only; browser and Tauri run the same code.
- **One datastore:** the durable preference stays in the member's SurrealDB prefs record.
- **No mocks:** pure palette/assignment/normalization in `pnpm test`; persistence rides the existing
  gateway-tested prefs path (no new gateway test needed — the axis is opaque data through the same
  verbs). No `*.fake.ts`.
- **State vs motion:** N/A — theme is state, applied locally.
- **Stateless extensions:** extensions consume the rail like any user; they own no icon-color state.
- **MCP surface / API shape:** no new verbs. Reads via `prefs.resolve`, the single write via
  `prefs.set`.
- **Core knows no extension:** the `iconColors` map is keyed by **opaque surface id**. `NavRail`
  applies whatever is in the map for the key it is rendering — no `if surface === "channels"` branch
  and no extension-id special case. `RAIL_SURFACES` enumerates core surfaces only because those are
  the ones the *picker UI* lists; the application path treats every key as opaque data.
- **One responsibility per file:** `lib/theme/icon-colors.data.ts` (palette + assignment + hex
  validation), `features/theme/IconColorSwatch.tsx` (one surface's picker),
  `features/theme/IconColors.tsx` (the accordion section). No file nears the 400-line cap.
- **SDK/WIT impact:** none. The UI federation widget contract is untouched.

## Example flow

1. A member opens Settings → Theme and scrolls to **Icon colors** (an accordion, beside Brand
   colors). Colorization is OFF; the section offers one button: **Auto-assign colors**.
2. They click it. `autoAssignIconColors(RAIL_SURFACES)` runs, writing one palette color per rail
   surface into `theme.iconColors`. The provider applies the change live: every sidebar icon now
   renders in its assigned color, in both the expanded and collapsed rail.
3. They expand a surface's row → a popover opens with the 100-swatch grid + a hex field. The
   currently-assigned swatch is ringed. They click a different swatch (or paste a hex); the icon
   recolors instantly.
4. They hit **Re-run auto-assign** to start over, or **Clear all** to fully revert (the field is
   deleted; icons return to default fg).
5. The debounced `prefs.set` persists the whole `ui_theme` blob; the desktop shell resolves the same
   icon colors on next boot — the preference roamed.

## Testing plan

Per `scope/testing/testing-scope.md` — the customizer's suite is the floor; no new platform-data
category opens (the axis is opaque client-side data through the already-gateway-tested prefs verbs).

- **Palette + assignment (unit, `pnpm test`):** the palette ships exactly 100 unique, frozen,
  canonical lowercase `#rrggbb` values; `autoAssignIconColors` is deterministic, one-color-per-key,
  every value a palette member, and a small key set samples colors far apart (not the first N
  neighbors); hex validation accepts `#rrggbb`, expands `#rgb`, fails closed to null on garbage.
- **Normalization (unit, existing `theme-options` coverage):** a stored blob with a malformed
  `iconColors` drops bad entries per-key and keeps valid ones; an all-garbage blob drops the field
  entirely (presence === OFF).
- **Component (unit):** the accordion starts disabled; Auto-assign populates every rail surface;
   each row's swatch opens the 100-swatch dialog and a pick writes the chosen hex; Clear all
   reverts to the disabled state.
- **Persistence round-trip:** inherited from the customizer's existing gateway test — `iconColors`
   is opaque data in the same blob, so the round-trip needs no new gateway test.
- **Build/lint:** `pnpm exec eslint` clean on every new/changed file; `pnpm exec tsc --noEmit`
   introduces no new errors.

## Risks & hard problems

- **Contrast on the active item.** An icon color that clashes with the accent highlight on the
  active rail item can read poorly. Mitigated by: mid-lightness + strong-saturation palette
  defaults (readable on both light and dark sidebar surfaces), and per-icon editability so a member
  can fix any one clash. A future contrast nudge is a non-goal here, not a blocker.
- **"Presence === ON" must stay honest.** If `setIconColor`'s last-entry-removed branch ever fails
  to delete the field, colorization would stick on with an empty map. The provider's
  `Object.keys(next).length > 0 ? next : undefined` guard and the normalization's all-garbage-drop
  are both load-bearing — covered by the IconColors "Clear all" test.
- **Palette determinism vs. rail growth.** Adding a core surface shifts every index after it under
  even sampling (the existing surfaces keep their colors only if they stay earlier in the list).
  Acceptable: a new surface is rare, and **Re-run auto-assign** is one click. Members' explicit
  per-icon overrides are keyed by surface id, so they survive a rail reordering untouched.

## Open questions

- **Per-extension icon colors in the picker?** Deferred — the map already supports `ext:<id>` keys,
  but `RAIL_SURFACES` is core-only. Add when an extension wants a self-color UI through the generic
  seam (never a core branch on the id).
- **Contrast policy?** Warn-only on low-contrast picks, or accept-as-member-choice? Leaning
  accept-as-choice (decorative, not body text); revisit if members report legibility issues.
- **Palette size beyond 100?** 100 comfortably covers the ~20 rail surfaces + per-workspace
  variation + hand-pick headroom. Revisit only if a real need surfaces.

## Related

- `theme-customizer-scope.md` — the shipped predecessor: base-token bridge, prefs `ui_theme` axis,
  member/workspace-default fold. This scope adds one optional axis to the same blob.
- `theme-appearance-scope.md` — flags the native `<input type="color">` WebKitGTK bug this slice's
  in-DOM swatch popover avoids.
- `nav-rail-scope.md` — owns the rail surface list (`RAIL_SURFACES`) the picker iterates.
- `../../FILE-LAYOUT.md` — data-not-branches decomposition (palette as frozen data, one picker per
  surface).
