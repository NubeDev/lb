# Session — nav hide + pins (sidebar curation)

Status: **in-progress → built, tests green** (2026-07-08). Scope:
[`scope/nav/nav-hide-and-pins-scope.md`](../../scope/nav/nav-hide-and-pins-scope.md). Branch:
`ext-devkit-updates` (owner commits).

## What was built

Two deltas on the shipped nav plane — an **admin hidden-set** and **per-user pins** — end to end
(store → verbs → MCP bridge → gateway routes → UI), exactly per the scope.

### Rust (`rust/crates/host/src/nav/`)

- `model.rs` — `NavHidden { hidden[], updated_ts }` (+ `HIDDEN_TABLE`, `MAX_HIDDEN = 200`,
  `MAX_PINNED = 50`); `NavPref` gains additive `pinned: Vec<String>` (old records deserialize with
  no pins); `ResolvedNav` gains `hidden` (the echo the UI subtracts from its client-side fallback)
  and `pinned` (resolved items), both present on every tier including `Fallback`.
- `store.rs` — `read_hidden`/`write_hidden` (one `nav_hidden:[ws]` record, constant id, LWW).
- `hidden.rs` (new) — `nav_hidden_get` (rides `mcp:nav.resolve:call`) / `nav_hidden_set` (rides
  `mcp:nav.save:call`, bounded, blank refs rejected).
- `pref.rs` — `nav_pref_set` is now a **partial write**: `nav_id: Option` (None = keep the pick,
  `""` clears) and `pinned: Option` (None = keep pins) — a pin toggle can never clobber the active
  pick. Bounded by `MAX_PINNED`.
- `resolve.rs` — the strip pipeline gains its third filter in ONE place: `strip_hidden()` (post
  cap-strip + ext-strip, recursing one group level) applied to menu items, and `resolve_pins()`
  resolves the member's pin refs through the SAME `resolve_item` pipeline (cap-, ext-, and
  hidden-stripped — **hide beats pin**), never mutating the stored record (un-hide restores free).
  `pin_to_item()` maps the shared ref grammar (bare surface key | `ext:<id>` | `dashboard:<id>`).
- `tool.rs` / `tool_call.rs` / `system/catalog.rs` — `nav.hidden.get`/`nav.hidden.set` on the MCP
  bridge; gate aliases: `hidden.get` → `nav.resolve`, `hidden.set` → `nav.save` (the same pattern as
  `nav.set_default` — **no new caps**, a deliberate deviation from the scope's named
  `mcp:nav.hidden_*:call`; the set_default precedent is stronger and avoids widening the grant
  matrix for one pointer-shaped record).

### Gateway (`rust/role/gateway/`)

- `GET|POST /nav/hidden` (routes/nav.rs, server.rs); `POST /nav/pref` body gains optional `pinned`
  and `id` became optional (partial write).

### UI (`ui/src/`)

- `lib/nav/` — `NavHidden` type, `ResolvedNav.hidden/pinned`, `NavPref.pinned`;
  `getNavHidden`/`setNavHidden`/`setNavPins`; `http.ts` bridges `nav_hidden_get/set` + the partial
  `nav_pref_set`.
- `features/shell/useResolvedNav.ts` — returns `{ items, hidden, pinned, togglePin }`; `togglePin`
  reads the RAW `nav_pref` (the resolved pins are stripped, so they can't be the write source),
  flips the ref, re-resolves.
- `features/shell/NavRail.tsx` — a **Pinned** section above whichever menu applies; the fallback
  rail subtracts the `hidden` echo (surfaces AND `ext:<id>` slots); a hover pin/unpin toggle on
  rail entries (rail-only affordance, per the scope's open-question lean); `SURFACE_GROUPS`
  exported as the settings tab's honest source.
- `features/settings/SidebarTab.tsx` (new) + a URL-routable `sidebar` tab in `SettingsView` —
  hide switches per rail source (shared `SURFACE_GROUPS` + live `ext.list` pages, never a copy),
  editable only with `CAP.navSave` (member read-only; server re-checks regardless).

## Open questions → resolved during build

- **Hiding the settings surface**: moot — `settings` is not a rail entry (it lives in the
  page-header gear), so it isn't in the hideable list and no lock-out exists.
- **Pin affordance**: rail-only hover toggle in v1 (header stars deferred), as the scope leaned.
- **`debug: true` strip-reasons on resolve**: not built — the settings tab needed no preview in
  v1; re-open if the tab grows one.
- **New caps vs riding existing grants**: rode `nav.save`/`nav.resolve` (see above). The scope's
  "Related"/testing language still holds — deny paths are per-verb and tested.

## Tests (all green)

- **Rust** `crates/host/tests/nav_test.rs` — 22 pass (15 shipped + 7 new): admin-cap deny +
  capless-read deny; workspace isolation (set + echo); **headline** hide-strips-menu-but-direct-
  read-still-succeeds; hide inside groups + at default/team tiers; bounds (201 refs, blank ref,
  LWW replace/clear); pins ordered + cap-stripped + record-never-mutated + partial-write semantics;
  **hide beats pin** + un-hide restores; pins member-owned + bounded (51st pin rejected).
- **UI unit** `NavRail.test.tsx` — +3: fallback subtracts `hidden`; Pinned section renders above
  the menu; `onTogglePin` fires with the entry ref (pressed state = Unpin).
- **UI real gateway** `SidebarTab.gateway.test.tsx` — 5: the tab round-trips a hide toggle
  (Save → `/nav/hidden` → echo on a fallback resolve); member write-deny (read still works);
  hidden-strip on a resolved (pick-tier) menu; pins partial-write (active pick survives) + ordered
  resolve; hide-beats-pin + restore with no `nav_pref` rewrite.

## Incidental fixes (pre-existing on this branch)

~120 test files (and `role/cli/src/header.rs`, `crates/secrets/src/lib.rs` test mods) still built
`lb_auth::Claims` without the branch's new `constraint`/`run_id` fields and did not compile;
mechanically added `constraint: None, run_id: None`. No behavior change.

## Follow-ups (named, not built)

- Header pin stars (beyond the rail hover toggle).
- Per-team hide / pin sharing — explicitly out of scope; the nav builder covers team menus.
- A `debug` strip-reason field on `nav.resolve` if the Sidebar tab grows a preview.
