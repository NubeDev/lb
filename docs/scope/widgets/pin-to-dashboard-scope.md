# Widgets scope — Slice B: pin a tool result-render to a dashboard (`dashboard.pin`)

Status: scope (the ask). Promotes to `public/frontend/dashboard.md` (a "Pin to dashboard" section beside
Slice A's) + `public/channels/` once shipped. Topic: `widgets` (the umbrella program).

**Slice B of the system-wide widget program**
([`widget-platform-scope.md`](widget-platform-scope.md) § "Slice B — Pin-to-dashboard"). The umbrella is a
sketch (7 lines); this doc is the build-ready scope for the slice. It closes **G2** (a tool/channel widget
cannot be pinned to a dashboard) and is the keystone for "widgets are system-wide": a GENERIC path that
takes any `x-lb-render` envelope — a tool's `ToolDescriptor.result`, or a live channel `rich_result` body —
and mints a persisted `dashboard:{id}` cell. The reminder widget (`reminder.list`, which already declares a
`result = table` render) becomes dashboard-addable with **zero reminder-specific code** in the pin path: the
envelope is already a valid `{view:"table", source:{tool:"reminder.list"}, options, tools, fieldConfig}`
cell-shape, and Slice A's `check_view_cells` validator already accepts it. **No branch on a tool id (rule
10) — the envelope is opaque data.**

## The open question, resolved: `dashboard.pin` verb, NOT client-compose

The umbrella leaned client-compose ("the client/AI builds the cell from the envelope + `dashboard.save`")
**unless a server-side mint proves necessary**. This slice picks the **server-side mint verb**, and the
proof of necessity is the same argument Slice A used to put save-validation server-side:

- **The host is the only boundary every writer crosses.** A pin produces *persisted state* (`dashboard:{id}`
  cell), not ephemeral render. A headless external agent over `POST /mcp/call` — no shell, no
  `ResponseView.buildCell` — must be able to pin a tool's `result` envelope. With client-compose, every
  client (web shell, RN app, AI agent, external agent) re-implements the envelope→cell mapping; the host
  cannot enforce fidelity, and an unbuilt mapping in one client silently breaks the pin. Slice A rejected
  this exact shape for save-validation: *"a headless `POST /mcp/call` or routed-Zenoh writer bypasses the
  shell entirely; the host is the only boundary every writer crosses."* The same holds for the mint.
- **Rule 7 (MCP is the universal contract).** `dashboard.pin` is one MCP verb the AI, the web UI, and the RN
  app call identically. The central agent reads `reminder.list`'s `result` from `tools.catalog` and pins it
  with one call — no client-side cell construction, no tool-specific knowledge in the client.
- **Envelope↔cell fidelity is a host concern (the umbrella's named risk).** The mapping is non-trivial: a
  `Cell` carries layout (`i/x/y/w/h`), `schema_version`, the `tools`→`sources[]` fold (the row-control
  write verbs become hidden extra targets so `cellTools(cell)` covers `render.tools`), `fieldConfig`, and
  the v3 panel shape. One mapping in the host, not N in clients. The shipped channel render path
  (`ResponseView.buildCell`) already does this mapping client-side for EPHEMERAL render; `dashboard.pin`
  reuses the SAME field-for-field mapping for the PERSIST path, but host-side, so the pinned cell renders
  identically to the channel response (the umbrella's cross-surface fidelity invariant).
- **Slice D reuses it.** Channel-origin authoring (Slice D: response → widget → preview → dashboard) pins a
  previewed envelope; `dashboard.pin` is the verb it calls. Building the mint client-side now would force
  Slice D to re-derive it.

**Rejected alternative — client-compose (`buildCell` + `dashboard.save`).** Rejected because (a) it makes
the host unable to enforce the envelope→cell mapping for a headless writer, (b) it forces every client to
replicate the mapping (the leak the no-fakes rule catches: an AI reading TS `buildCell` and an AI reading
the RN port can't tell whether either is the truth), and (c) the envelope↔cell fidelity risk is best owned
by ONE host function, not mirrored across surfaces. The channel render path (`ResponseView.buildCell`) is
UNTOUCHED — it keeps doing ephemeral envelope→cell for render; `dashboard.pin` hosts the persist-time twin.

## Goals

- **A `dashboard.pin` MCP verb.** One write verb that takes an `x-lb-render` envelope + a target dashboard
  id, mints one v3 `Cell` host-side, and persists via the existing `dashboard.save` validation + write path
  (so `check_view_cells` / `check_genui_cells` / `check_cells_bounds` / `validate_and_strip_refs` all run —
  the same authority Slice A established for every writer). Append-or-replace by a stable cell id derived
  from the envelope's `source.tool` (opaque string ops — idempotent: re-pinning the same envelope updates
  the cell in place, not duplicates). Returns the hydrated dashboard (mirrors `dashboard.save`'s return).
- **The host owns the envelope→cell mapping.** One `mint_cell_from_envelope(envelope) -> Cell` function in
  `rust/crates/host/src/dashboard/pin.rs` (sibling of `save.rs`), reusing the SAME field mapping
  `ResponseView.buildCell` uses client-side (view/source/action/options/fieldConfig/tools-fold), but
  emitting a v3 cell (Slice A's validator accepts v3). The tool id in `source.tool` is OPAQUE DATA — no
  `match`/`if` on it (rule 10); the cell id `i` is `pin-{slug(source.tool || view)}` by pure string ops.
- **A "Pin to dashboard" affordance in the channel.** Near `ResponseView` (where a `rich_result` renders):
  a control that picks a target dashboard (from `dashboard.list`, or "new dashboard") and calls
  `dashboard.pin` over the real gateway. The client constructs the ENVELOPE (it already has it — it's the
  rendered `rich_result` body, or the descriptor's `result` interpolated with the collected args); the host
  constructs the CELL. No cell construction in the client.
- **The headline: `reminder.list` is dashboard-addable with ZERO reminder-specific code.** Pinning
  `reminder.list`'s declared `result` envelope produces a persisted cell that reloads and renders through
  the real `WidgetView` (the same renderer the channel uses), with the pin path treating `reminder.list` as
  opaque data. This is the proof that "a tool + a `result` envelope IS a widget" (source #2) is now
  placeable on a dashboard, not just renderable in a channel.

## Non-goals

- **No new cell/view contract.** The v3 `Cell` shape, the `View` vocabulary, the `options`/`fieldConfig`
  roots are frozen as shipped (Slice A's Non-goal, carried forward). `dashboard.pin` mints a v3 cell that
  Slice A's validator already accepts — it does not widen the contract.
- **No change to how a widget renders or gets data.** `WidgetView`, the federation bridge, `ctx.data`
  frames, trust tiers, and `cell.tools ∩ grant` re-check are untouched. A pinned cell renders exactly as
  the same envelope renders in a channel.
- **No per-tool / per-extension special-casing.** The pin path treats `source.tool` and any `ext:<id>`
  view key as opaque data (rule 10). A GitLab-for-GitHub swap forces no change to `dashboard.pin`. The
  reminder widget is the PROOF, not a special case.
- **No envelope-schema strictness beyond Slice A.** `dashboard.pin` validates the minted cell through the
  EXISTING `check_view_cells` (view name) + `check_genui_cells` (genui IR) + bounds — it does NOT add
  envelope-level option-key validation (Slice A's named follow-up, still deferred). A malformed envelope
  (missing `view`, non-object `source`) is a loud `BadInput`; an envelope with an invented option key still
  persists (same stance as `dashboard.save`).
- **No result-render coverage (Slice C).** B only needs the envelope→cell path to work for a tool that
  ALREADY declares a `result` (`reminder.list` is the proof). Do not add `result` envelopes to other tools.
- **No channel-origin AI authoring (Slice D).** B is the pin mechanism; D later builds the
  response→widget→preview→pin flow on top of it. B does not build the AI/preview authoring surface.
- **No option-key validation, version stamping, ext-key install-resolve** — Slice A follow-ups, deferred.
- **No batch pin.** One envelope → one cell per call (a small, bounded, always-fast write — not a job).

## How it fits the core

- **Tenancy / isolation (rule 6):** the pin reuses `dashboard.save`'s owner-only-update + the workspace
  namespace (the dashboard id is workspace-scoped, the cell's `source.tool` re-checks under the viewer's
  grant at render). A pin in ws-A is invisible to ws-B (the cell lives on a `dashboard:{id}` record in
  ws-A's namespace). Tested with the mandatory two-session isolation case.
- **Capabilities (rule 5/7):** new member-level write cap `mcp:dashboard.pin:call`. The pin verb gates on
  it FIRST (workspace-first, then the cap), opaque deny. The owner-only-update inside the persist path
  (mirroring `dashboard.save`) is the second gate — a non-owner with the pin cap still cannot overwrite
  someone else's dashboard. **Wiring (required, or the verb is dead-on-arrival):** the member credential map
  (`rust/role/gateway/src/session/credentials.rs::member_caps()`) enumerates the dashboard caps explicitly
  and its wildcards (`mcp:*.{get,list,write,create,update,delete,post}:call`) do NOT match `.pin` — same
  trap as `dashboard.catalog` (Slice A). This slice adds `mcp:dashboard.pin:call` to `member_caps()`; the
  happy-path test runs with a PLAIN member token (only `mcp:dashboard.pin:call` +
  `mcp:dashboard.get:call` + `mcp:dashboard.list:call`) so it proves the grant, not an admin bypass. No
  pre-approval (rule 10): `granted = requested ∩ admin_approved` like every verb.
- **Placement:** either — pure node-local read+write, no cloud authority, no `if cloud`. Symmetric.
- **One datastore:** no new persistence. The pinned cell lives on the existing `dashboard:{id}` record
  (SurrealDB); the envelope is NOT stored as a separate record (the cell IS the persisted form). No new
  table, no new blob.
- **MCP surface (API shape):** **one write verb + its descriptor.** `dashboard.pin { dashboard, title?,
  envelope, now } -> Dashboard` (returns the hydrated record, mirroring `dashboard.save`). No CRUD on pins
  (a pin is a cell on a dashboard; CRUD on the dashboard is the existing `dashboard.save`/`.delete`). No
  get/list (a pin's result is read via `dashboard.get`). No live feed. No batch. The verb self-describes via
  a `ToolDescriptor` so it appears in `tools.catalog` (the AI discovers it can pin).
- **Bus (Zenoh):** N/A — state (a dashboard cell), not motion. No new subject.
- **Sync / authority:** node-local; the pin is deterministic from the envelope + the dashboard record. No
  offline divergence.
- **SDK/WIT impact:** none — no plugin boundary change. An extension's `[[widget]]` tile is already
  pinnable as an `ext:<id>/<widget>` envelope view (structural, like `dashboard.save` accepts it).
- **Skill doc:** **Yes** — extends `skills/dashboard-widgets/SKILL.md` with "how an agent/user pins a tool
  result to a dashboard" (the `dashboard.pin` call, the envelope shape, the idempotent re-pin), grounded in
  a live gateway run. The slice's implementing session writes/updates it.

## The envelope → cell mapping (the core work)

`mint_cell_from_envelope(envelope: &Value, existing: Option<&Cell>) -> Result<Cell, DashboardError>` lives
in `rust/crates/host/src/dashboard/pin.rs`. It mirrors `ResponseView.buildCell` field-for-field so a pinned
cell renders identically to the channel response (the cross-surface fidelity invariant):

| Envelope field | Cell field | Notes |
|---|---|---|
| `view` | `view`, `widget_type` | `widget_type = view` for v1-render compat. Validated by `check_view_cells`. |
| `source {tool, args}` | `source: Source { tool, args }` | The re-runnable read. Empty `tool` → no source (an inline-data-only envelope). |
| `action {tool, argsTemplate}` | `action: Action { tool, args_template }` | A control's write tool. |
| `options` | `options` | Opaque `Value` (row controls, viz options). |
| `fieldConfig` | `field_config` | Opaque `Value` (per-field presentation — copied so the shared table column-model resolves headers/hide). |
| `tools[]` | `sources[]` (hidden extra targets) | The row-control write verbs (minus `source.tool`/`action.tool`) become hidden `sources[]` so `cellTools(cell)` = `render.tools` (the bridge leash). Mirrors `buildCell`'s fold. |
| — | `i` | `pin-{slug(envelope.source.tool || envelope.view || "cell")}` — stable, idempotent. Pure string ops; no branch on the tool id (rule 10). |
| — | `x, y, w, h, v` | If updating an existing cell with the same `i`, KEEP its layout (a re-pin preserves position). Else default `0, next-free-y, 6, 4`, `v: 3`. |
| — | `schema_version` | Set by the persist path (the `Dashboard` record's `schema_version`, pinned at save). |

The minted cell + the dashboard's existing cells are then run through the SAME validation chain
`dashboard.save` uses (`check_cells_bounds` → `check_genui_cells` → `check_view_cells` →
`validate_and_strip_refs`) before `write_dashboard`. The pin reuses the validation primitives (they're
`pub`); it does NOT call `dashboard_save` (it has its own cap gate `mcp:dashboard.pin:call`, distinct from
`mcp:dashboard.save:call`).

## Example flow

1. In a channel, a member runs `reminder.list` from the command palette. The palette posts the descriptor's
   `result` envelope (interpolating the collected args into `source.args`); the channel renders it via
   `ResponseView` (the shipped path, unchanged).
2. The member clicks "Pin to dashboard" on the rendered response. The affordance lists their dashboards
   (`dashboard.list`) + a "New dashboard" option. They pick "Ops".
3. The affordance calls `dashboard.pin { dashboard: "ops", envelope: <the rich_result body minus
   kind/v>, now }` over `POST /mcp/call`. The host: gates `mcp:dashboard.pin:call`; mints the cell
   (`i: "pin-reminder-list"`); reads dashboard "ops" (owner-only — the member must own it); appends the
   cell (or replaces the existing `pin-reminder-list`); validates via the Slice A chain; writes; returns
   the hydrated dashboard.
4. The member opens the Ops dashboard. The pinned cell renders through `WidgetView` → `TablePanel` (with
   row controls via the shipped `ResponseTable`-equivalent path on the dashboard side, OR the shipped
   `TablePanel` if row controls are channel-only — see Open questions), re-running `reminder.list` under
   the viewer's grant. It is indistinguishable from a hand-authored reminder table cell.
5. A headless AI agent, given `reminder.list`'s `result` from `tools.catalog`, pins it the same way:
   `POST /mcp/call dashboard.pin { dashboard, envelope: <descriptor.result> }`. No shell in the loop; the
   host is the boundary. The resulting cell is byte-identical to the channel-origin pin (same mint fn).

## Testing plan

Real gateway + real store, no fakes (rule 9). Mirror Slice A's test file
(`rust/crates/host/tests/widget_catalog_test.rs`) for structure. Mandatory categories:

- **Capability deny (required) + plain-member happy path.** A principal WITHOUT `mcp:dashboard.pin:call` is
  denied (opaque `ToolError::Denied`). The happy path runs as a PLAIN member holding ONLY
  `mcp:dashboard.pin:call` + `mcp:dashboard.get:call` + `mcp:dashboard.list:call` — proves the grant, not an
  admin bypass. A non-owner with the pin cap is denied on an existing dashboard they don't own (the
  owner-only-update gate).
- **Workspace isolation (required).** A pin in ws-A produces a cell on a ws-A dashboard; a ws-B principal
  cannot see it (`dashboard.get` in ws-B returns none of ws-A's dashboards). The envelope's `source.tool`
  re-checks under the viewer's grant at render — a ws-B viewer with `reminder.list` granted still can't see
  ws-A's reminders (the source gate is workspace-walled).
- **The HEADLINE (integration + UI).** Pin `reminder.list`'s declared `result` envelope → a persisted cell
  that reloads and renders through the real `WidgetView`, with ZERO reminder-specific code in the pin path.
  Assert the pin path is generic: the Rust mint function accepts an envelope with an ARBITRARY tool id
  (e.g. `federation.query`, `__test__.x`) and produces a valid cell — no `match`/`if` on the id. The UI
  gateway test pins a real `reminder.list` rich_result to a real dashboard, reloads, and asserts the
  reminder rows render through `WidgetView` (the channel→dashboard parity).
- **Envelope↔cell fidelity (the umbrella's ~line 197 risk).** Assert the minted cell round-trips: pin an
  envelope → `dashboard.get` → the cell's `view`/`source`/`action`/`options`/`fieldConfig`/`sources[]`
  match the envelope (the `tools` fold, the `fieldConfig` copy, the v3 shape). Re-pin the SAME envelope →
  the cell is REPLACED (idempotent, same `i`), not duplicated. Pin a DIFFERENT envelope → a new cell
  appends.
- **Shell path AND headless `POST /mcp/call` parity.** The same envelope pinned via the direct
  `dashboard_pin` function AND via `call_tool` → `dashboard.pin` produces the SAME cell (Slice A proved this
  pattern for the validator; B proves it for the mint). The Slice A view-rejection still fires through the
  pin path (an envelope with `view:"heatmap"` → `BadInput`, nothing persisted).
- **Catalog completeness (unit, Rust).** `dashboard.pin`'s `ToolDescriptor` is in `host_descriptors()` so
  `tools.catalog` lists it (the AI discovers it can pin). The descriptor's `input_schema` names
  `dashboard`/`envelope`/`now`.

A UI unit test covers the "Pin to dashboard" affordance (renders near `ResponseView`, calls `dashboard.pin`
over the real gateway, shows success). The UI gateway test is the headline parity proof.

## Risks & hard problems

- **Envelope↔cell fidelity (the named risk).** A `result` envelope and a persisted `Cell` are close but not
  identical (the cell has layout, `schema_version`, the `tools`→`sources[]` fold). The mint must produce a
  v3 cell that Slice A's validator accepts AND `WidgetView` renders unchanged. Mitigation: ONE host function
  (`mint_cell_from_envelope`) reused by both the shell and headless paths; the fidelity test asserts the
  round-trip; the UI parity test asserts the pinned cell renders identically to the channel response.
- **Row controls on a dashboard.** The channel renders a `table` with `options.rowControls` via
  `ResponseTable` (the shipped `TablePanel` has no per-row control column). A pinned reminder cell on a
  dashboard needs the same row controls (enable switch, run-now, delete) to be useful — does the dashboard
  `TablePanel` render them, or does the pin path need to route through a dashboard-side `ResponseTable`?
  Resolved in implementation (see Open questions) — the long-term-right call is one row-control-aware table
  renderer shared by both surfaces, but this slice may ship the pinned cell rendering read-only on the
  dashboard IF the row-control path is channel-coupled (a named follow-up, not a silent gap).
- **Idempotency by tool id.** Deriving `i = pin-{slug(source.tool)}` means pinning two DIFFERENT envelopes
  from the same tool (e.g. `reminder.list` with different filters) collide on the same cell. Acceptable for
  v1 (the "pin the reminder widget" use case is one cell per tool); a future envelope-hash `i` is the
  follow-up if a second filter matters. Documented as a known limit, not a silent bug.

## Open questions

- **Row controls on the dashboard.** Does the shipped `TablePanel` honor `options.rowControls`, or is that
  channel-only (`ResponseTable`)? Resolve during implementation: (a) if `TablePanel` already renders row
  controls, the pinned cell works as-is; (b) if not, this slice ships the pinned reminder cell as a
  read-only table on the dashboard (row controls channel-only) AND names the dashboard-side row-control
  path as a follow-up — NOT a silent gap. The headline test asserts the cell renders (rows visible); row
  controls on the dashboard are a stated, named limit if (b).
- **Cell id idempotency.** `i = pin-{slug(source.tool || view)}` (one cell per tool per dashboard) vs a
  full envelope hash (one cell per unique envelope). Picking the tool-id slug for v1 (simpler mental model:
  "the reminder widget is on the dashboard"); the hash is the follow-up if a second filter matters. Decide
  and document in the session.
- **"New dashboard" from the pin affordance.** If the user picks "New dashboard" in the channel affordance,
  does `dashboard.pin` create it (idempotent UPSERT like `dashboard.save`, owner = principal, visibility =
  private), or does the affordance call `dashboard.save` first with empty cells then `dashboard.pin`? Lean:
  `dashboard.pin` creates-if-absent (one call, mirrors `dashboard.save`'s UPSERT) — decide in the build.

## Related

- Umbrella: [`widget-platform-scope.md`](widget-platform-scope.md) (the program; Slice B § ~line 105).
- Slice A (shipped): [`../frontend/dashboard/widget-catalog-scope.md`](../frontend/dashboard/widget-catalog-scope.md)
  — `dashboard.catalog` + `check_view_cells` (the validator the pin reuses). Skill:
  [`../../skills/dashboard-widgets/SKILL.md`](../../skills/dashboard-widgets/SKILL.md).
- Precedents reused: [`ResponseView.tsx`](../../../ui/src/features/channel/ResponseView.tsx) `buildCell`
  (the client-side envelope→cell mapping the mint mirrors), [`dashboard/save.rs`](../../../rust/crates/host/src/dashboard/save.rs)
  (the validation chain the pin reruns), [`dashboard/catalog.rs`](../../../rust/crates/host/src/dashboard/catalog.rs)
  (the descriptor pattern), [`reminder/descriptor.rs`](../../../rust/crates/host/src/reminder/descriptor.rs)
  `list_render()` (the headline envelope).
- Core rules: README §3 (rules 5/6/7/10), `docs/scope/extensions/extensions-scope.md` (opaque ext ids).
- Skill (build updates it): `skills/dashboard-widgets/SKILL.md`.
