# Extensions scope — managed dashboards (an extension owns a board; humans can read it, admins can fix it)

Status: scope (the ask). Two asks, one small and one real, both surfaced by the first extension that
generates dashboards from its own domain data:

1. **Cut `lb-ext-ui-sdk` `ui-v0.15.0`.** The `dashboard`/`vars` fields on `ExtNavItem`/`ExtNavChild`
   and their clamp are written on the SDK repo's HEAD (`1bbe148`) but **untagged**. Every other half
   of ext-dashboard-nav has shipped — the Rust relay, the manifest parse, the shell render. Consumers
   pin tags, so the feature is unreachable until this exists.
2. **A `managed` dashboard.** A dashboard created by an extension (`owner = "ext:<id>"`) is today
   indistinguishable from a human's, un-editable by every human including admins, and gives a
   confusing bare `Denied` to anyone who tries.

First consumer:
`NubeIO/rubix-ai-extensions → extensions/modbus/docs/scope/device-dashboards-scope.md` (PRIMARY —
read it for the product motivation). Shell half:
`NubeIO/rubix-ai → docs/scope/frontend/managed-dashboard-badge-scope.md`. Ask (1) blocks the consumer
outright; ask (2) does not — the consumer ships correct without it and carries the sharp edge.

## The problem

### Ask 1 — the untagged SDK

`@nube/ext-ui-sdk@ui-v0.14.0`'s `dist/page.d.ts` declares `ExtNavChild` as
`{id, label, icon?, children?}` and its `clampNavChildren` copies exactly those four keys. An
extension that emits `dashboard`/`vars` has them stripped at the clamp and its child falls back to an
ext route — the "confusing half-failure" the ext-dashboard-nav DoD named. Meanwhile:

- `rust/crates/ext-loader/src/manifest.rs:41-110` — `NAV_MAX_DASHBOARD`/`NAV_MAX_VARS`/`NAV_MAX_VAR_KV`
  and the `[[ui.nav]]` `dashboard`/`vars` validation: **shipped**.
- `rust/crates/assets/src/install/model.rs:85-95` — `dashboard: Option<String>` + `vars` relayed
  verbatim: **shipped**.
- `rubix-ai → ui/src/features/shell/NavRail.tsx:163-199, 622-730` — `dashboardChildActive`, the
  `dashboardEntry` render, the active-highlight reverse-lookup: **shipped**.
- `lb-ext-ui-sdk → src/page.ts:96-104`, `src/nav.ts:18-98` — the fields + `clampVars`: **written,
  untagged**.

So the only missing artifact is a release. Cut it, and consumers (`rubix-ai/ui/package.json:32`,
`rubix-ai-extensions/extensions/modbus/ui/package.json:13`, both on `ui-v0.14.0`) bump.

### Ask 2 — an extension-owned dashboard has no honest lifecycle

A native sidecar's callback token is minted with `sub = "ext:<ext_id>"`
(`rust/crates/host/src/native/spec.rs:74`) — a stable `Member` principal. That is a *good* property:
an extension that upserts a dashboard on every config change gets a consistent owner, so
`dashboard.save`'s owner-only update check (`…/dashboard/save.rs:154`) passes no matter which human
triggered the change. Everything an extension needs to *create* a shared board already works today:
`dashboard.save` (create, `owner = ext:modbus`, `visibility = private`) followed by
`dashboard.share {visibility: "workspace"}` (owner-only — and it *is* the owner,
`…/dashboard/share.rs:57-60`). **This scope is not asking for new authority.**

What is missing is that the resulting record is a second-class citizen:

- **It is invisible as machine-generated.** In the roster, the switcher, and `dashboard.list` it looks
  exactly like a hand-made board. `Dashboard` (`…/dashboard/model.rs:460-505`) carries `owner` and
  nothing that says *what kind of thing* the owner is. A UI would have to string-match `"ext:"` on the
  owner to know — which is a per-shape branch on an id, the thing rule 10 exists to prevent.
- **No human can edit it, including an admin.** `dashboard.save` is owner-only with no override, while
  `dashboard.delete` HAS one (`dashboard.delete_any`, `…/dashboard/delete.rs:34` + the catalog row).
  So an admin may *delete* an extension's board but may not *fix* it. That asymmetry is the bug.
- **The denial is bare.** An operator who drags a cell and saves gets `DashboardError::Denied` with no
  hint that the board is managed or that duplicating is the way forward.
- **A converge silently reverts human intent.** If a human ever *could* edit it, the extension's next
  reconcile would overwrite them with no record that it happened. A managed board must be honest that
  its content is derived.

## Goals

1. **Release `ui-v0.15.0`** of `lb-ext-ui-sdk` from the written HEAD; verify the built `dist/`
   carries `dashboard`/`vars` on both nav types and that `clampNavChildren` copies + bounds them
   (`NAV_MAX_VARS = 32`, `NAV_MAX_VAR_KV = 128`, over-cap keys **dropped**, never truncated). Bump the
   pins in `rubix-ai/ui` and the consuming extension.
2. **A `managedBy` marker on `Dashboard`** — `#[serde(default, deserialize_with = "null_default")]
   pub managed_by: String` (rename `managedBy`), empty for every existing board. Set by
   `dashboard.save` **automatically** when the saving principal's `sub` is an extension identity —
   i.e. derived from the principal, never accepted as caller input (an extension cannot claim another
   extension's board; a human cannot mark a board managed). Relayed on `DashboardSummary` so the
   roster can paint it without a full `get`. Opaque to every other subsystem.
3. **The admin-override triad** — `dashboard.save_any` **and** `dashboard.share_any`, each exactly
   mirroring the shipped `dashboard.delete_any`: same catalog row shape, same "admin override"
   description, same check position (attempted only after the owner check fails), each its own
   capability. This closes the fix-vs-delete asymmetry *and* the re-scope-visibility gap in one pass
   rather than leaving the second half to be rediscovered later (D2). An admin save on a managed board
   is a legitimate, auditable act — and the extension's next reconcile will overwrite it, which is
   exactly why (4) exists.
4. **The reconcile is honest.** `managed_by` is what lets the shell say "this board is regenerated by
   `modbus`; your edits will be replaced" *before* someone spends an hour on it (rubix-ai sibling
   scope). No new host behaviour — the marker is the whole mechanism.
5. **A typed denial.** When a save is refused on a `managed_by`-marked board, the error carries that
   fact (a distinct `DashboardError` variant or a marked `Denied`) so the client can render "managed by
   `<id>` — duplicate to edit" instead of a bare denial. This must not leak existence to a caller who
   failed gates 1+2: the marker is only revealed to a caller who could already *read* the dashboard.

## Non-goals

- **No new authority for extensions.** An extension can already create + share + delete its own
  boards. Nothing here widens what `ext:<id>` may do.
- **No host interpretation of `managedBy`.** It is a string the host sets from the principal and
  relays. No lookup of the extension, no lifecycle coupling: if `modbus` is uninstalled, its boards
  remain (owned, marked, orphaned) until something deletes them. Cascading uninstall-delete is a
  separate ask, deliberately not bundled.
- **No pack coupling.** A pack-applied dashboard and an extension-managed dashboard are different
  things and stay different; this scope does not unify them.
- **No merge/rebase of human edits over a regenerated board.** Overwrite is the contract; the marker
  makes it visible. Anything smarter is a much larger design.
- **Not a breaking change.** `managedBy` is additive + defaulted; `save_any` is a new cap nobody holds
  by default. A pre-field dashboard round-trips byte-clean.

## How it fits the core

- **Capabilities:** `mcp:dashboard.save_any:call` and `mcp:dashboard.share_any:call` are the new
  grants, admin-only by convention exactly like `dashboard.delete_any`. Deny path: an admin without
  them gets the same owner-only denial as today.
- **Tenancy:** unchanged — everything is workspace-scoped through `authorize_dashboard`'s gate 1.
- **Data:** one additive field on the existing `dashboard` table. No new table, no migration (serde
  default).
- **MCP surface:** two new tools (`dashboard.save_any`, `dashboard.share_any`) + one new field on
  `dashboard.get`/`.list` output. No new read verb; no live feed; no batch (a save is one small
  record — explicitly not a job).
- **Rule 10:** the host never branches on *which* extension. `managedBy` is set from the principal and
  relayed; the shell branches on the field's presence, never on its value.
- **Placement:** either. **Secrets:** none. **Bus:** none.

## Example flow

1. `modbus` (as `ext:modbus`) calls `dashboard.save { id: "modbus-tmpl-sdm630", … }`. The host sees an
   extension principal, creates the record with `owner = "ext:modbus"`, `managedBy = "modbus"`,
   `visibility = private`.
2. `modbus` calls `dashboard.share { id, visibility: "workspace" }` — owner-only, and it is the owner.
3. Alice (member) opens the board from a modbus nav child; gate 3 passes on `workspace`. The roster row
   and the page header show a "Managed by modbus" badge (rubix-ai scope).
4. Alice drags a cell and saves → refused, typed: "managed by modbus — duplicate to edit". She clicks
   **Duplicate**; she gets `dashboard:modbus-tmpl-sdm630-copy`, `owner = alice`, `managedBy` empty —
   an ordinary board she owns.
5. An admin holding `dashboard.save_any` fixes a broken cell in place; the change sticks until the
   extension's next reconcile, and the header already warned that it would.
6. The template is edited in modbus → `dashboard.save` again, same owner, same id — an idempotent
   upsert. `visibility` is preserved across the update (already true today), so a deliberately
   narrowed board is not re-widened.

## Testing plan

Per `scope/testing/testing-scope.md` — real store, real caps, no fakes.

- **Capability deny-tests (mandatory):** save (and share) on a managed board as a non-owner admin
  *without* `save_any`/`share_any` → denied; *with* → allowed. Both mirror the existing `delete_any`
  test shape, including the check-order assertion (owner path first, override only on failure).
- **Workspace isolation (mandatory):** a managed board in ws A is invisible/unsavable from ws B.
- **Marker provenance:** a human caller cannot set `managedBy` (input field ignored/rejected); an
  extension principal always gets it set; `ext:a` cannot save over `ext:b`'s board.
- **Round-trip:** a pre-field stored dashboard reads with empty `managedBy` and re-saves byte-clean;
  `DashboardSummary` carries the field.
- **Denial shape:** the typed managed-denial is returned only to a caller who passed gates 1+2 (a
  caller who cannot read the board still gets the opaque `Denied` — no existence leak).
- **Hot-reload (mandatory for the ext path):** re-publish the consuming extension mid-run; its
  reconcile re-saves the same id under the same owner without error.
- **SDK release:** a built `ui-v0.15.0` round-trips `dashboard`/`vars` through `clampNavChildren`
  (kept, bounded, over-cap dropped) and through `ext.list` relay — the DoD item ext-dashboard-nav
  already specified but could not verify without a tag.

## Risks & hard problems

- **The asymmetry is load-bearing, so get the overrides right.** Admins can already `delete_any`;
  adding `save_any`/`share_any` is smaller than it looks, but each is a write override on someone
  else's asset — the check must sit strictly *after* the owner check and be its own cap, never folded
  into an ambient "admin" role test.
- **Marker inference from the principal.** Deriving `managedBy` from `sub`'s `ext:` prefix is a shape
  match on an identity, not a branch on an extension id — acceptable — but it must live in ONE helper
  so a future identity scheme (a differently-prefixed sub) changes one place, not five.
- **Reconcile-overwrites-admin.** An admin fix vanishing on the next converge will feel like a bug the
  first time it happens. The warning in the UI is the mitigation; if it proves insufficient, the next
  step is a `managedRevision`/"extension last wrote at T" line, not merge logic.
- **Uninstall orphans.** Boards outlive the extension by design (D5) and are removable with
  `delete_any`. Someone will eventually ask for cascade-delete; it is a destructive default and gets
  its own scope.
- **Release order.** (1) has to land before any consumer builds; (2) is independent and can follow.

## Decisions (RESOLVED — there are no open questions)

- **D1 — `managedBy` holds the bare extension id** (`"modbus"`), not the full principal. The full
  principal is already on `owner` (`"ext:modbus"`), so storing it twice adds nothing; the bare id is
  what a badge renders and what a roster filter keys on. Derived host-side from the principal's `sub`
  in **one** helper (`managed_by_of(principal) -> Option<String>`), so a future identity scheme
  changes one function.
- **D2 — Ship the complete admin-override triad: `dashboard.save_any` AND `dashboard.share_any`,
  alongside the shipped `delete_any`.** Not "defer share_any until someone asks": the three verbs that
  mutate an asset someone else owns must have the same override story, or the gap resurfaces the first
  time an admin needs to re-scope a board's visibility and finds they can delete it but not share it.
  Each is its own capability, checked strictly *after* the owner check fails, never folded into an
  ambient admin-role test. Same catalog-row shape and same test shape as `delete_any`.
- **D3 — Managed boards are listed, badged, and filterable.** They appear in `dashboard.list` like
  anything else; `managedBy` rides `DashboardSummary` so the roster paints the badge without a full
  `get` **and** so a client can filter/group on it without a second read. Hiding machine-made boards
  is how they become invisible orphans; a filter the user opts into is the right control.
- **D4 — No interpolation work is required here.** The consuming extension's boards use variable
  substitution in **both** target `args` and SQL query text; both paths are already implemented and
  wired in the shell (`rubix-ai → ui/src/lib/vars/interpolate.ts`, `interpolateValue.ts`, applied at
  every `viz` dispatch site). This scope neither changes nor duplicates them — it only pins them with
  a regression test in the rubix-ai sibling scope.
- **D5 — Uninstall does not cascade-delete an extension's boards.** They remain (owned, marked,
  orphaned) and are removable with `delete_any`. Cascading deletion of a user-visible asset on
  uninstall is a destructive default and belongs in its own scope with its own confirmation story.

## Related

- `NubeIO/rubix-ai-extensions → extensions/modbus/docs/scope/device-dashboards-scope.md` (PRIMARY consumer)
- `NubeIO/rubix-ai → docs/scope/frontend/managed-dashboard-badge-scope.md` (shell half)
- `scope/extensions/ext-dashboard-nav-scope.md` (the nav fields — shipped; ask 1 releases them)
- `scope/extensions/native-caller-identity-scope.md` (why the sidecar's own `sub` is `ext:<id>`)
- `rust/crates/host/src/dashboard/` — `save.rs`, `share.rs`, `delete.rs`, `visibility.rs`, `model.rs`
