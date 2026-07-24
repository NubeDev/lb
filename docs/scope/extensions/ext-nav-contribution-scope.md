# Extensions scope — extension nav contribution (an extension extends the host sidebar)

Status: scope (the ask). Additive over the shipped UI-federation contract (`ui-federation-scope.md`)
and the widget identity/options work (`ext-widget-panel-options-scope.md`). Owns the **manifest +
host-relay + SDK** half; the shell rendering half is
`NubeIO/rubix-ai → docs/scope/frontend/ext-nav-rail-scope.md`, and the first consumer is
`NubeIO/ems → ems-ext/docs/scope/nav-unification-scope.md`. All three ship together or none do.

## The problem

An extension today gets **exactly one** sidebar slot. `[ui]` carries a single `entry`/`label`/`icon`
(`rust/crates/assets/src/install/model.rs` `ExtUi`), `ext.list` relays one row, and the shell renders
it as one flat entry routed at a flat `path: "/ext/$id"`. An extension with more than one
top-level destination has nowhere to put the others.

So it builds its own. EMS ships a second sidebar **inside its mount** — `ems-ext/ui/src/components/
layout/nav-rail.tsx` + `AppShell.tsx`, a four-item rail (Sites · Explore · Studio · Access) rendered
beside the host's own sidebar. The result is two navigation columns stacked side by side, one of
which the host cannot see, search, pin, hide, or deep-link into. That is not an EMS bug — it is the
only move available under a one-slot contract, and any extension with two pages will re-derive it.

The consequences of a private in-mount rail are structural, not cosmetic:

- **Not addressable.** The URL stays `/t/<ws>/ext/ems` no matter which of the four views is open. No
  deep link, no shareable link to Explore, no back/forward. EMS wrote this down as "Decision 4: no
  URL router — the host owns the address bar" (`top-view.tsx`) — a correct reading of the contract,
  and precisely the constraint this scope lifts.
- **Not host-integrated.** The nav planes the platform already ships — `nav.resolve`'s authored
  menus, the hide-and-pins set, the workspace default — all key on refs like `ext:<id>`. They can
  address the extension but nothing **inside** it, so an admin can neither pin EMS→Explore nor hide
  EMS→Access.
- **Not consistent.** The in-mount rail is the extension's own components, so it drifts from host
  nav behavior (collapse, active styling, icon rail mode, mobile treatment) by construction.

## Goals

- **A declared nav tree in the manifest.** `[ui]` gains an optional ordered `[[ui.nav]]` list — the
  extension's top-level destinations. Each item is `{ id, label, icon?, admin?, dynamic? }`. Absent
  ⇒ today's exact behavior (one slot named `label`), so every shipped manifest keeps working.
- **Sub-path addressing.** The host route becomes `/ext/$id/$*`, and the trailing segment is handed
  to the mount as `ctx.route`. The extension renders that route; when it navigates internally it
  calls `ctx.onNavigate(path)` and the host updates the address bar. Deep links, back/forward, and
  shareable URLs work for extension destinations exactly as they do for core surfaces.
- **Dynamic children, host-rendered.** An item marked `dynamic = true` may have its children supplied
  **at runtime by the extension** through a new `bridge.setNav(items)` — so EMS's `Sites` can list the
  caller's reachable sites as real sidebar children. The host renders whatever it is handed and
  **branches on none of it** (rule 10: ext ids and item ids stay opaque).
- **Labels stay i18n keys the extension owns.** `label` is relayed verbatim; the extension's own
  catalog resolves it. The host never translates an extension's string (EMS rule 8 — en + es).
- **One vocabulary.** A contributed nav item is addressed `ext:<ext>/<item-id>` — the same shape the
  widget work already uses for view keys, so `nav.resolve`, hide-and-pins, and the router share one
  ref grammar with no new special case.

## Non-goals

- **No new authority.** A nav item is a **lens**, exactly as the core nav is: it grants nothing. The
  bridge still gates every call against the install's approved scope, and the host re-checks
  server-side. `admin = true` on an item is a **presentation** gate (mirrors `visibleNavItems`); the
  verbs remain the real wall.
- **No host interpretation of item ids.** The host routes and renders them; it never branches on a
  known id. An `if ext == "ems"` anywhere in the shell is the leak this scope exists to avoid.
- **No unbounded dynamic nav.** `setNav` is capped (count + depth + label length, below) and is
  per-mount ephemeral — it is not persisted, not shared between members, and never outlives the page.
- **Not a replacement for `nav.resolve`.** Authored menus stay the admin's tool; this scope makes
  extension internals *addressable* so authored menus and pins can finally reference them.

## The manifest shape

```toml
[ui]
entry = "remoteEntry.js"
label = "EMS"
icon  = "activity"
scope = [ "ems.site.list", ... ]

[[ui.nav]]
id    = "sites"
label = "nav.sites"      # an i18n KEY in the extension's own catalog
icon  = "layout-grid"
dynamic = true           # children supplied at runtime via bridge.setNav

[[ui.nav]]
id    = "explore"
label = "nav.explore"
icon  = "line-chart"

[[ui.nav]]
id    = "studio"
label = "nav.studio"
icon  = "wrench"
admin = true             # presentation gate only
```

Parse-time validation (mirroring the `[[widget]]` `id`/`options` precedent): `id` is a non-empty
slug (`[a-z0-9-]{1,32}`), **unique** within the block; `label` non-empty, ≤64 chars; `icon` ≤64
chars; at most **16** items. A violation is a manifest parse ERROR, not a silent drop — the same
posture `pack.validate` takes on a reserved table shadow.

## The relay

`ExtUi` gains `#[serde(default)] pub nav: Vec<ExtNavItem>` and a new
`ExtNavItem { id, label, icon, admin, dynamic }` (all serde-defaulted). `ext.list`'s `ExtRow`
relays it verbatim, the way `options` is relayed today — **the host stores and forwards, it never
interprets**. Installs written before this field deserialize to an empty vec ⇒ one flat slot.

## The SDK / mount contract

Additive on `@nube/ext-ui-sdk` — the `mount(el, ctx, bridge)` signature is unchanged, so this stays a
minor, not a breaking major:

- `ctx.route: string` — the sub-path below `/ext/<id>/` (`""` at the root).
- `ctx.onNavigate(path: string): void` — ask the host to change the address bar. The host navigates;
  the ext re-renders from the resulting `ctx.route`. **One direction of truth: the URL.**
- `bridge.setNav(items: ExtNavChild[]): void` — publish dynamic children for `dynamic` items. Capped
  at 200 items total, depth ≤ 3, label ≤ 64 chars; over-cap is truncated with a console warning
  rather than throwing (a nav is chrome — it must never break the page).

`ctx.route` changes must re-render the page **without remounting it**. Today `ExtHost` re-keys the
mount effect on theme/caps changes; routing through that path would unmount on every nav click and
lose all page state. This needs the live `update(ctx)` re-supply that `ext-widget-panel-options-scope`
already names as the widget-parity follow-up — **that follow-up is a prerequisite here, not optional.**

## Risks

- **The remount trap** (above) is the one that will bite. If `ctx.route` is threaded through the
  existing re-key, every sidebar click remounts the extension: full data refetch, lost scroll, lost
  form state. Build `update(ctx)` first and verify a route change does **not** call `mount` twice.
- **Reach leakage through nav labels.** Dynamic children are the first case where **extension data**
  (site names) renders in **host chrome**. Whatever the extension hands `setNav` is displayed as-is,
  so the extension must derive those rows through its own reach chokepoint — for EMS,
  `site_reach/`'s `reachable_sites`, covered by its CI fence. The host cannot check this for the
  extension; the scope's job is to state plainly that `setNav` is a **reach-scoped output** and any
  consumer owes it a cross-client deny-test.
- **Nav-plane ref explosion.** `ext:<ext>/<item>` refs will appear in `nav_pref.pinned` and
  `nav_hidden.hidden` and can outlive an uninstall or a manifest edit that drops an item. Resolution
  must tolerate a dangling ref by dropping it silently — the existing cap-strip path already does
  this for `ext:` refs, so extend that, don't add a second rule.

## Definition of done

1. `[[ui.nav]]` parses + validates (unique slug ids, caps enforced, error not silent-drop); manifest
   tests cover a valid block, a duplicate id, an over-cap list, and an absent block.
2. `ExtUi.nav` / `ExtRow` relay it verbatim; an install written before the field reads as empty.
3. SDK exports the `ExtNavItem`/`ExtNavChild` types, `ctx.route`, `ctx.onNavigate`, `bridge.setNav`.
4. `update(ctx)` re-supply lands **first**; a route change re-renders without a remount (test asserts
   `mount` called once across several navigations).
5. A reference extension declares two nav items and one dynamic parent, proving the whole path.
6. Released as a `node-v*` + `ui-v*` tag pair — downstream (rubix-ai shell, ems) consumes tags, per
   `ems/docs/WORKFLOW-LB.md`.
