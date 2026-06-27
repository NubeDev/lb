# Frontend dashboard scope — surface extension `[[widget]]` tiles in the builder palette

Status: **SHIPPED** (2026-06-28). Built in
[`sessions/frontend/widget-palette-session.md`](../../../sessions/frontend/widget-palette-session.md);
shipped truth in [`public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) → "Extension
widgets in the palette". A small, fully-unblocked slice on top of the **shipped** widget-builder v2
([`widget-builder-scope.md`](widget-builder-scope.md)).

When an extension ships a packaged dashboard widget (a `[[widget]]` tile, e.g. `proof-panel`'s **Proof
Ping**), a dashboard editor must get **a new option when adding a widget** — the tile appears in the
builder's source picker, gated to users who can edit the dashboard. Today the *renderer* for a packaged
tile is shipped (`ExtWidget` mounts `ext:<id>/<widget>` through the v2 bridge, tested against a real
gateway) but the *palette never offers it*: the only way to get such a cell is to hand-author the
`view: "ext:<id>/<widget>"` key. This slice closes that one discovery gap — the cheap last mile of the
extension-widget story.

---

## The gap, precisely

The widget-builder v2 surface is shipped end to end **except** the palette entry for a packaged tile:

- **Renderer — SHIPPED.** [`ui/src/features/dashboard/builder/ExtWidget.tsx`](../../../../ui/src/features/dashboard/builder/ExtWidget.tsx)
  parses `ext:<id>/<widget>`, picks the trust tier ([`trust.ts`](../../../../ui/src/features/dashboard/builder/trust.ts):
  allow-listed publisher key → in-process module federation via
  [`federationWidget.ts`](../../../../ui/src/features/dashboard/builder/federationWidget.ts); otherwise the
  sandboxed iframe [`WidgetIframe.tsx`](../../../../ui/src/features/dashboard/builder/WidgetIframe.tsx)), and
  calls the remote's `mountWidget(el, ctx, bridge, widgetId)` export. `WidgetView` routes any `ext:` cell
  here. Proven by the real-gateway test (`widgetBuilder.gateway.test.tsx`, an `ext:mqtt-bridge/cooler-switch`
  cell) and by `proof-panel` shipping `mountWidget` on its remote.
- **Backend — SHIPPED.** `ext.list` already returns each install's `widgets[]` (entry/label/icon/scope),
  narrowed to `requested ∩ admin_approved` (`crates/host/src/ext/list.rs`, `ui_decl.rs`). The v2 bridge
  re-checks `cell.tools ∩ install-grant` at the host per call (`tool_call.rs` `build_call_context`,
  `callback.rs`). Nothing new is needed server-side.
- **Palette entry — MISSING.** [`sourcePicker.ts`](../../../../ui/src/features/dashboard/builder/sourcePicker.ts)
  `extensionEntries()` reads `row.widgets[].scope` **only to harvest the widget's tools** as `extension`/
  `action` source entries — it never emits a `SourceEntry` for the **packaged tile itself**. The picker's
  groups are `series` / `live` / `sql` / `extension` / `action`; there is no `widget` group and no entry
  whose selection produces a `view: "ext:<id>/<widget>"` cell. So `proof-panel`'s **Proof Ping** can render
  in a cell but cannot be *added* from the builder UI.
- **Permission gate — ABSENT (UI side).** The builder is rendered unconditionally; whether a viewer may add
  a widget is enforced only when `dashboard.save` is called server-side. The "if they have the permissions"
  ask wants the **add** affordance hidden from a read-only viewer (the server check stays as the real gate).

## Goals

- **A new palette option per packaged tile.** For each installed extension's `[[widget]]` block, the
  builder's source picker offers one entry under a new **"Extension widgets"** group, labelled by the tile's
  `label`/`icon`. Selecting it produces a v2 cell with `view: "ext:<id>/<widget>"` (no view chooser — a
  packaged tile *is* its own view) and adds it on **Add to dashboard**.
- **Gate the affordance to editors.** The "Add widget" builder (and therefore the new entries) is shown only
  to a viewer with the dashboard **edit** capability (`mcp:dashboard.save:call`); a read-only viewer sees the
  dashboard without the add surface. The server re-check on `dashboard.save` remains the authoritative gate.
- **Live preview through the real bridge.** A selected tile previews in the builder exactly as it will render
  in the cell — `WidgetView` → `ExtWidget` → the real `mountWidget` over the v2 bridge, reaching only
  `tile.scope ∩ grant`. No fake preview.
- **Zero new backend, zero contract change.** `ext.list.widgets[]`, the v2 cell schema (`view`/`source`/
  `action`), the v2 bridge, and the trust tiers are all shipped and unchanged. This is a frontend
  discovery-and-gating slice only.

## Non-goals

- **No widget-builder v2 contract change.** The cell shape, the `mountWidget` export, the bridge, and the
  `[[widget]]` manifest block are frozen as shipped. This slice adds a picker entry and a gate, nothing more.
- **No new backend verb.** No change to `ext.list`, `dashboard.*`, or the bridge. (If `ext.list` does not
  already expose the widget's publisher-key/trust hint the renderer needs, that is an `ext.list` projection
  detail — confirm against the shipped row, do not invent a new verb.)
- **No building of the external-data reference extensions.** The user's "timescaledb extension" example
  needs the **extension side** (a native sidecar that owns a DB connection) which is **blocked** on separate
  platform fixes — see "What is ready vs blocked" and [`reference-extensions-scope.md`](../../extensions/reference-extensions-scope.md).
  This slice makes such an extension's tile/tool *addable from the dashboard the moment it exists*; it does
  not build the extension.
- **No new persistence, no `*.fake.ts`.** The cell still lives in `dashboard:{id}.cells[]`; tests use a real
  gateway and a real installed extension (`proof-panel`).

## Intent / approach

**Emit one packaged-tile `SourceEntry` per `[[widget]]`, render it as a non-configurable view, and gate the
builder on the edit cap.** The data is already in hand (`useSourcePicker` already passes `installed: ExtRow[]`
to the renderer); the work is three small, FILE-LAYOUT-respecting changes:

1. **`sourcePicker.ts` — add `extWidgetEntries(rows)`.** One `SourceEntry` per `row.widgets[]` tile:
   `group: "widget"`, `label` from the tile (`<ext> · <tile.label>`), `icon` carried for the option, and a
   resolved selection identifying `ext` + `widgetId` so the cell key becomes `ext:<id>/<widget>`. Add a
   `widget` value to the `SourceEntry["group"]` union and fold these into `buildSourceEntries`. The existing
   tool-harvesting (`extension`/`action` entries) stays — a tile and its tools are *both* useful sources.
2. **`WidgetBuilder.tsx` — render the group + force the view.** Add a `PickerGroup group="widget"
   label="Extension widgets"`. When a widget entry is selected, set the cell `view` to the packaged
   `ext:<id>/<widget>` key and **hide the view chooser** (a packaged tile carries its own renderer; `viewsFor`
   returns the single packaged view). The candidate cell sets `widget_type`/`view` to the `ext:` key and the
   preview routes through the shipped `WidgetView`.
3. **The edit gate.** Pass an `canEdit` (derived from the session's caps — does it hold `mcp:dashboard.save:call`
   for this workspace?) into `WidgetBuilder` (or its parent), and render the "Add widget" surface only when
   `canEdit`. Source the cap from wherever the shell already exposes the session grant (the same place the nav
   gates extension pages); do not re-derive it from a fresh call if the shell already has it.

**Why a `widget` group and not reuse `extension`.** The `extension` group offers an extension's *tools* (build
your own view over `mqtt.status`); the packaged tile is a *finished widget the developer shipped*. They are
different author intents and different cell shapes (`{source:{tool}}` vs `view:"ext:<id>/<widget>"`), so they
are distinct picker entries. Collapsing them would force the author to know which tools a tile uses — exactly
the MCP-literacy the source picker exists to hide.

**Rejected alternatives:**

- *A separate "widget gallery" palette distinct from the source picker.* Rejected — the source picker already
  is the one "add a tile" surface; a second palette splits the mental model. One picker, a new group.
- *Gate purely server-side (show Add to everyone, fail on save).* Rejected — showing an action a viewer can't
  complete is a poor affordance; gate the UI on the cap **and** keep the server re-check (defense in depth).
- *Let the builder configure a packaged tile's binding.* Rejected — a packaged tile owns its own data needs
  via its `scope`; the builder only places it. Configuration, if ever needed, is a later additive slice.

## How it fits the core

- **Tenancy / isolation (rule 6):** unchanged — `ext.list` is workspace-partitioned; the rendered tile's
  bridge derives the workspace from the session token, never the cell. A ws-B editor only ever sees and adds
  ws-B's installed tiles. Covered by the existing two-session test, extended to "the packaged tile added from
  the palette reaches only its own workspace."
- **Capabilities (rule 5/7):** two gates, both already-existing caps — **add affordance** gated on
  `mcp:dashboard.save:call` (UI) + re-checked on `dashboard.save` (host); the **tile's runtime calls** gated on
  `tile.scope ∩ install-grant`, re-checked per bridge call. This slice invents **no new capability**.
- **Placement (rule 1):** one builder, two transports (Tauri `invoke` / gateway SSE+HTTP). No role branch.
- **MCP surface (§6.1):** **consumed, not exposed.** The picker reads the shipped `ext.list` (get/list); the
  add persists via the shipped `dashboard.save` (one bounded UPSERT). No new verb, no live feed of its own
  (the tile's own `bridge.watch` rides the shipped series SSE), no batch.
- **Data (SurrealDB):** the cell is `dashboard:{id}.cells[]` with `view: "ext:<id>/<widget>"` — the shipped v2
  field. No new table.
- **Bus / Sync / Secrets:** unchanged — the tile's motion and any secret stay the tool's concern; the cell is
  a §6.8 upsert; no secret reaches the picker or the cell.
- **SDK/WIT impact:** **none.** The `[[widget]]` manifest block, the `mountWidget` export, and the v2 bridge
  are frozen as shipped. This slice touches only the shell's picker + gate.

## Example flow

1. **Install.** An admin installs `proof-panel` into `kfc` (it ships a `[[widget]]` tile **Proof Ping**,
   `scope = ["series.latest","series.find"]`). `ext.list` returns the install with `widgets:[{label:"Proof
   Ping", entry, icon, scope}]`, narrowed to the grant.
2. **Open the builder as an editor.** Alice (holds `mcp:dashboard.save:call`) opens a dashboard in edit mode.
   The source picker now shows an **Extension widgets** group containing **proof-panel · Proof Ping**.
3. **Add it.** She selects it; the view chooser is hidden (it's a packaged tile); the preview mounts the real
   `mountWidget` over the v2 bridge and shows the latest `proof.demo` value. She clicks **Add to dashboard** →
   a cell `{ i, …, v:2, view:"ext:proof-panel/proof-ping" }` is appended and persisted via `dashboard.save`.
4. **Reload.** `WidgetView` → `ExtWidget` re-mounts the tile from the persisted cell — identical to the
   preview.
5. **Read-only viewer.** Bob (no `mcp:dashboard.save:call`) opens the same dashboard: he sees the rendered
   Proof Ping cell but **no "Add widget" surface** — the affordance is gated. If a crafted client tried to
   `dashboard.save` anyway, the host denies it (opaque).
6. **Isolation.** Dave (a `mcdonalds` session) sees only `mcdonalds`'s installed tiles in his picker; a `kfc`
   tile is not listed and its data is denied/empty behind the wall.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real gateway, a real installed `proof-panel`, real
tools, no `*.fake.ts`:

- **Capability deny (the headline):** a viewer **without** `mcp:dashboard.save:call` does not get the add
  affordance, **and** a direct `dashboard.save` from such a principal is denied server-side (assert the host
  denies even if the UI gate were bypassed). The added tile's bridge call to a tool **outside** its
  `scope ∩ grant` is denied at the host (reuse the shipped ExtWidget deny test).
- **Workspace isolation:** two real sessions — a ws-B editor's picker lists only ws-B tiles; the added tile
  reads/writes only ws-B; `ext.list`/picker is workspace-partitioned.
- **Builder round-trip (frontend, real gateway):** install `proof-panel` → the **Extension widgets** group
  lists **Proof Ping** → select → preview renders the real `proof.demo` latest → **Add** persists a
  `view:"ext:proof-panel/proof-ping"` cell → reload re-renders it (extend `widgetBuilder.gateway.test.tsx`).
- **Trust-tier routing (unchanged, re-asserted from the palette path):** a non-allow-listed tile added from
  the palette renders **sandboxed** (iframe); an allow-listed-key one federates in-process.
- **Hot-reload / eviction (unchanged):** uninstalling the extension drops its entry from the picker and its
  `ext:` cells render the honest "extension not installed" state with `watch` streams torn down.

## Risks & hard problems

- **The gate must be a real cap check, not a role guess.** "If they have the permissions" means
  `mcp:dashboard.save:call` for *this* workspace, sourced from the session grant the shell already holds — not
  a hard-coded admin flag. Get it from the same place the nav gates editing surfaces; the server re-check is
  the backstop.
- **Don't double-offer confusingly.** The same extension now contributes both *tool* entries (build-your-own)
  and a *packaged tile* entry. Label them so an author isn't confused — the tile uses its `label`; tools use
  the `<ext> · <verb>` form. Keep the groups visually distinct.
- **Trust tier is the tile's, decided once.** The palette entry must not let an author pick "in-process"; the
  tier is decided by publisher key in `trust.ts` as today. The palette only places the tile.
- **Stale `ext.list` after uninstall.** A picker built from a stale `ext.list` could offer a tile that's gone;
  the renderer already degrades to "not installed," but the picker should re-read on workspace/extension
  change (it already keys on `ws`).

## Open questions

Decided so the build has no blocker; residuals are named follow-ups.

- **Picker group placement & label** — DECIDED: a new group **"Extension widgets"**, placed after "Installed
  extension"/"Action (control)" in the `<select>`. Additive.
- **Multiple `[[widget]]` tiles per extension** — DECIDED: one picker entry per tile (the cell key already
  carries `/<widget>`; `proof-panel` ships one, the model generalizes to N).
- **View chooser for a packaged tile** — DECIDED: hidden; a packaged tile is its own view. `viewsFor` returns
  the single `ext:<id>/<widget>` view when a widget entry is selected.
- **Where the edit cap comes from** — DECIDED: reuse the shell's existing session-grant source (the one the
  nav uses to gate editing); do not add a new read. If the shell does not yet expose `mcp:dashboard.save:call`
  to the dashboard view, surfacing it there is part of this slice (still no new backend verb).
- **Follow-up (not this slice):** a disabled/ghosted tile shown to read-only viewers as "ask an editor to add"
  — deferred; this slice simply hides the add surface.
- **`widget_type` vs `view` for a packaged tile** — RESOLVED in the build: the cell's *render* is driven by
  the v2 `view: "ext:<id>/<widget>"` key (`cellView` reads `view` first). `Cell.widget_type` is the v1
  fallback union (`chart|stat|gauge`) and the type system forbids putting the ext key there, so it stays the
  harmless `"chart"` every other v2 cell carries. The scope's intent (the cell points at the packaged key)
  is met via `view`. No contract change.

## What shipped

- `extWidgetEntries(rows)` + a `"widget"` group in `sourcePicker.ts` (folded into `buildSourceEntries`);
  `widgetIdOf` exported from `ExtWidget.tsx` so picker and renderer share one slug.
- `WidgetBuilder.tsx`: the "Extension widgets" `PickerGroup`, the view chooser hidden for a packaged tile
  (`viewsFor` returns the single `ext:<id>/<widget>` view), and a `canEdit` prop that gates the whole add
  surface.
- `DashboardView.tsx`: `canEdit = hasCap(useAppRoutingContext().caps, CAP.dashboardSave)` threaded into the
  builder — the shell's existing session-grant source, no new backend read.
- Tests (real gateway, real proof-panel, no fake): `widgetBuilder.test.ts` (unit),
  `widgetBuilder.gateway.test.tsx` (round-trip + cap-deny + isolation + trust-tier),
  `DashboardView.gateway.test.tsx` (wrapped in the real routing context), and a **live Playwright e2e**
  `ui/e2e/dashboard-widget.spec.ts` (built shell + real node: add from palette → in-process mount → real
  value).

**Trust-tier follow-up (resolved this slice).** Publishing `proof-panel` live revealed the iframe tier
could not render an installed widget (the remote externalizes React to the shell import map, which only
exists in-process) — it rendered blank with `Failed to resolve module specifier "react"`. Resolved by
making **installed extension widgets always render in-process** (the publish/install cap is the trust
gate); the sandboxed iframe tier is now reserved for **scripted author code** only. `trust.ts`
`extWidgetTier()` → `"in-process"`; `ExtWidget` dropped its iframe branch. See
[`../../../debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md`](../../../debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md).

## What is ready vs blocked (the user's two asks)

**Ask 1 — "a new option when adding a widget if they have the permissions": this slice, fully unblocked.**
Everything it needs (the `[[widget]]` projection in `ext.list`, the `ext:<id>/<widget>` renderer, the v2
bridge, the trust tiers, the `dashboard.save` cap) is shipped. `proof-panel` is the working reference tile.

**Ask 2 — "the widget can call the db, or call extensions (e.g. a timescaledb extension)": partly shipped,
partly blocked on the extension side, not the dashboard side.**

- **Call the DB — SHIPPED.** `store.query` / `store.schema` are live host verbs (parse-allowlisted to
  `SELECT`, workspace-walled, row/time-bounded); the builder's **"Direct SurrealDB"** source + Grafana-style
  Builder⇄Code SQL editor already make a DB-reading widget with no code. A widget calling the DB is done.
- **Call an extension's tools — SHIPPED (bridge side).** A widget reaches any granted extension tool through
  the v2 bridge, re-checked at the host. The dashboard is ready for a `timescale.query` / `mqtt.publish` the
  moment such a tool exists.
- **Building the external-data extension itself (the timescale/mqtt example) — BLOCKED, separately scoped.**
  A native sidecar that owns a Postgres/Timescale pool or an MQTT socket needs four platform fixes that are
  **not shipped**: the **native** host-callback transport (the wasm half shipped; the native stdio callback is
  not), the **`net:*`** capability family (the caps grammar has only `mcp:`/`store:`/`bus:`/`secret:`), the
  generic **`kv.*`** store, the binary-blob path — plus **`lb-secrets`** (a stub today) for the DB credential.
  These are owned by [`reference-extensions-scope.md`](../../extensions/reference-extensions-scope.md) (fixes
  1–4 + the secrets open question), not this slice. **The dashboard does not block them; they block the
  extension.** Once they land and a `timescale` extension ships its tool (and optionally a `[[widget]]` tile),
  it appears in the picker via the very mechanism this slice builds — no further dashboard work.

## Related

- [`widget-builder-scope.md`](widget-builder-scope.md) — the shipped v2 builder/bridge/cell this extends; the
  `ext:<id>/<widget>` renderer (follow-ups 1 & 2) is built there, the palette entry is the last mile here.
- [`widgets-scope.md`](widgets-scope.md) — resolves its open question "should widget palette entries appear
  only to dashboard editors with `mcp:dashboard.save:call`?" (yes — editors only) and its "render `ext:<id>`
  in a cell" item (rendered there; *added* here).
- [`README.md`](README.md) — the dashboard subtopic index; update its "what is not shipped yet" (federated
  widgets **do** render now; palette discovery is the remaining gap this closes).
- [`../../extensions/reference-extensions-scope.md`](../../extensions/reference-extensions-scope.md) — the
  blocked **extension side** of ask 2 (timescale/mqtt + the four platform fixes + secrets).
- [`../../extensions/ui-federation-scope.md`](../../extensions/ui-federation-scope.md) — the page bridge this
  narrows to a cell; `proof-panel` is the working model (ships the `[[widget]]` tile + `mountWidget`).
- `rust/extensions/proof-panel/extension.toml` (the `[[widget]]` block) +
  `ui/src/mount.tsx` (`mountWidget`) — the reference tile this slice surfaces.
- README **§6.1** (API shape), **§6.6** (the gates), **§6.13** (extension UIs), **§3** (rules 5/6/7).
</content>
</invoke>
