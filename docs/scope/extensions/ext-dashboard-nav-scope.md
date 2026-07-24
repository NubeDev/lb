# Extensions scope — a `dashboard` target on a contributed nav entry (SDK + relay)

Status: scope (the ask). The **SDK + manifest/relay half** of letting an extension's nav entry open a
HOST dashboard instead of its own page. Additive over the shipped ext-nav-contribution work
(`ext-nav-contribution-scope.md`). This is a **short consumer stub** — the PRIMARY design + the shell
render live in `NubeIO/rubix-ai → docs/scope/frontend/ext-dashboard-nav-scope.md`; the first consumer is
`NubeIO/ems → ems-ext/docs/scope/dashboard-nav-scope.md`. All three ship together.

## The problem

The shipped ext-nav types carry no way to point at a host dashboard. `ExtNavItem`
(`{id,label,icon?,admin?,dynamic?}`) and `ExtNavChild` (`{id,label,icon?,children?}`) — the SDK
`@nube/ext-ui-sdk` `page.ts` types, mirrored by `ExtUi.nav` / `ExtNavItem` in
`rust/crates/assets/src/install/model.rs` and relayed by `ext.list`'s `ExtRow` — encode only an opaque
`ext:<ext>/<id>` route into the extension's mount. The host's OWN dashboard-nav grammar
(`dashboard:{id}` ref + a `vars` binding) exists and works, but an extension cannot emit it. See the
rubix-ai primary doc for the full mechanism trade-off (option (a) chosen: reuse the host grammar).

## Goals

- **Two optional fields on both nav types**, mirroring the host `NavItem`'s `dashboard`/`vars`:
  - `dashboard?: string` — a `dashboard:{id}` ref (opaque; the host resolves it against its dashboard
    plane). Absent ⇒ the entry routes into the mount exactly as today.
  - `vars?: Record<string, string>` — a pinned variable binding the host folds into the viewer URL as
    `?var-<name>=<value>` (the SAME shape the host `NavItem.vars` uses).
  - Both land on `ExtNavItem` (a static, manifest-declared dashboard nav item) AND `ExtNavChild` (a
    dynamic, per-entity `setNav` dashboard child — the crux case: `site-1 → site-overview` bound to
    that site, `meter-1 → meter-detail` bound to that meter).
- **Relay verbatim, interpret never.** `ExtUi.nav`'s Rust `ExtNavItem` gains
  `#[serde(default)] dashboard: Option<String>` + `#[serde(default)] vars: BTreeMap<String,String>`;
  `ExtRow` relays them the way it relays `nav` today. The host stores and forwards; it does not resolve
  the dashboard id or the vars (rule 10). Installs written before the fields deserialize to `None`/empty.
- **Extend the clamp, don't fork it.** `clampNavChildren` (`@nube/ext-ui-sdk` `nav.ts`) already copies
  `{id,label,icon,children}` per node; extend it to also copy `dashboard` + `vars`, and bound `vars`
  (≤ 32 keys, key + value ≤ 128 chars each — same posture as the label clamp: truncate with a console
  warning, never throw). A dynamic child that smuggled extra fields is still stripped to the known set.
- **Manifest parse validation** (mirroring the `[[ui.nav]]` `id`/`label` checks): if a `[[ui.nav]]`
  item declares `dashboard`, it must be a non-empty string ≤ 128 chars; `vars` (if present) a string→
  string map within the clamp bounds. A violation is a parse ERROR, not a silent drop.

## The shape

Manifest (a STATIC dashboard nav item — the degenerate case):

```toml
[[ui.nav]]
id        = "fleet-overview"
label     = "nav.fleet"
icon      = "layout-dashboard"
dashboard = "dashboard:ems-fleet-overview"   # opens the host viewer, not the mount
# vars omitted ⇒ the dashboard opens unbound
```

Dynamic child (the CRUX — per-entity binding via `bridge.setNav`):

```ts
bridge.setNav([
  { id: "site-1", label: "Acme HQ",
    dashboard: "dashboard:ems-site-overview", vars: { site: "site-1" },
    children: [
      { id: "m/meter-1",          label: "Meter 1" },                    // ext route (no dashboard)
      { id: "m/meter-1/settings", label: "Meter 1 Settings" },          // ext route
      { id: "m/meter-1/board",    label: "Meter 1 Board",
        dashboard: "dashboard:ems-meter-detail", vars: { meter: "meter-1" } }, // host dashboard
    ] },
]);
```

A child with `dashboard` opens the host viewer var-bound; a child without it routes into the mount.
Both coexist under one parent — that interleaving is the whole point.

## Non-goals

- **No host interpretation of the ref or the vars.** The host does not validate the dashboard id, does
  not check the vars against the dashboard's declared variables, and does not reach-scope the
  dashboard's data. All of that is the viewer's / the extension's / the data plane's job (see the ems
  consumer scope's reach verdict).
- **No new authority.** The fields are presentation lenses. `admin` still gates presentation only; the
  verbs and the viewer's cap re-check remain the wall.
- **No change to the `ext:<ext>/<id>` route grammar.** A child without `dashboard` is unchanged.
- **Not a breaking major.** Both fields are optional + serde-defaulted; the `mount`/`setNav`/`ext.list`
  signatures are unchanged. Minor bump only.

## Risks

- **Field-strip on the wire.** If the relay or the clamp is not extended, an extension emitting
  `dashboard`/`vars` has them silently dropped and the child falls back to an ext route — a confusing
  half-failure. The DoD asserts round-trip through relay AND clamp.
- **`vars` size.** Unbounded vars in a `setNav` tree is a chrome-bloat + payload risk; the clamp bound
  is the single place it is enforced (never the host, never the shell).

## Definition of done

1. `ExtNavItem`/`ExtNavChild` (SDK TS + the Rust `ExtNavItem`) carry optional `dashboard` + `vars`;
   `ext.list`/`ExtRow` relay them verbatim; a pre-field install reads as `None`/empty.
2. `clampNavChildren` copies + bounds `dashboard`/`vars`; a child smuggling extra fields is stripped;
   an over-cap vars map truncates with a warning.
3. `[[ui.nav]]` parse validates a declared `dashboard`/`vars` (error, not silent-drop); tests cover a
   valid static dashboard item, a bad ref, and an over-cap vars map.
4. SDK exports the widened types; the reference extension emits one static dashboard nav item and one
   dynamic dashboard child, proving the wire path.
5. Released as a `node-v*` + `ui-v*` tag pair; downstream (rubix-ai shell, ems) consumes tags.

## Cross-links

- `NubeIO/rubix-ai → docs/scope/frontend/ext-dashboard-nav-scope.md` (PRIMARY: shell render + route)
- `NubeIO/ems → ems-ext/docs/scope/dashboard-nav-scope.md` (consumer + pack dashboards + reach verdict)
- `NubeIO/lb → docs/scope/extensions/ext-nav-contribution-scope.md` (the shipped nav-contribution base)
