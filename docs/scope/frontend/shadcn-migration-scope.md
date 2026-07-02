# Frontend scope — shadcn migration plan (flows as the template)

Status: scope (the ask). Execution companion to `ui-standards-scope.md`; promotes to
`public/frontend/frontend.md` as views land. This doc is the **ordered plan** for converting
every remaining legacy view onto shadcn/ui, using the **Flows page as the canonical shape**.

`ui-standards-scope.md` sets the *standard* (which primitives, the checklist, the ESLint
freeze). It nominates Members as the reference. This plan **re-bases the reference on
`features/flows/FlowsView.tsx`** — the request was "make every page look like Flows" — and turns
the standard into a concrete, view-by-view work order with waves, so a session can pick up the
next item and know exactly what "done" means.

## Goals

- Every full-screen surface in `ui/` matches the **Flows** shape: an `<AppPageHeader>`-led
  `<section className="flex h-full min-w-0 flex-col bg-bg text-fg">`, an optional
  roster/aside, a `min-h-0 flex-1` scrollable body, controls only from `components/ui/*`.
- All **28 views** currently in ESLint `LEGACY_VIEWS` are migrated and removed from that
  allowlist, one at a time, each keeping its `*.gateway.test.tsx` green.
- The three missing primitives (`dialog`, `switch`, `alert`) are generated and token-bound
  before their first caller needs them.
- End state: `LEGACY_VIEWS` is empty → flip `pnpm lint` to `eslint src --max-warnings 0` so
  the standard is build-breaking.

## Non-goals

- A visual redesign. Same tokens, density, amber accent (`ui-design-scope.md`). This changes
  *which primitives/layout* a page uses, not the palette.
- A big-bang PR. Migration is per-view; no new screen may add to the legacy pattern.
- Net-new product surface. No new pages.

## The canonical shape (copy Flows, not Members, not Dashboard)

`features/flows/FlowsView.tsx` is the reference because it is the only large surface already
100% on the standard: `AppPageHeader`, an inline `role="alert"` error strip, a roster aside
(`FlowRail`) + a `min-h-0 flex-1` body (`FlowCanvas`), and an empty-state card — all shadcn.

```tsx
<section aria-label="… view" className="flex h-full min-w-0 flex-col bg-bg text-fg">
  <AppPageHeader icon={Icon} title={…} description="…" workspace={ws} actions={…} />
  {error ? <div role="alert" className="border-b border-destructive/20 bg-destructive/10 …">{error}</div> : null}
  <div className="flex min-h-0 flex-1">
    <Rail … />                {/* list / open / new / delete */}
    {open ? <Body … /> : <EmptyState />}
  </div>
</section>
```

> ⚠️ **Do not copy `DashboardView.tsx` as a template.** Despite its comment, it still uses
> `.page-header` and raw `<button>`/`<select>` — it is a *migration target*, not a model.
> `AppPageHeader` signature: `{ icon, title, description?, workspace?, actions?, className? }`.

## The standard (per-view checklist — run before removing from `LEGACY_VIEWS`)

From `ui-standards-scope.md` §"The standard", applied per view:

1. **Controls are shadcn.** No raw `<button>/<input>/<select>/<textarea>` and no legacy
   `globals.css` class (`control-field*`, `soft-button*`, `danger-button-sm`, `icon-button`,
   `scope-pill`, `page-header*`, `page-title`, `page-subtitle`, `state-alert`). Use the
   `components/ui/*` primitive; generate it if missing.
2. **Header is `AppPageHeader`** inside the `flex h-full min-w-0 flex-col` section.
3. **It resizes** at ≤640px: no horizontal scroll, fixed side columns collapse or move to the
   mobile sheet, `w-1/2` splits become `flex-col md:flex-row`, no un-overridden `w-64`/`w-96`.
4. **Tokens, not raw colors** — `bg/fg/muted/accent/border/destructive`, no `text-red-600` etc.
5. **Tests stay green** through the real gateway harness; `aria-label`/`aria-current`/roles kept.
6. **Remove the view's path from `LEGACY_VIEWS`** in `ui/eslint.config.js`, then `pnpm lint`.

## Component backlog (generate first, bind to our tokens)

Present today: `badge button card input select sheet sidebar table tabs textarea tooltip`.
**Still missing — generate before their wave:**

- **`dialog`** — replaces hand-rolled `role="dialog"` in `ConfirmDestructive`,
  `CreateSeriesWizard` (Wave 1 blocker for `confirm/` and `ingest/`).
- **`switch`** — replaces `SchemaForm`'s hand-rolled `role="switch"` (Wave 3, ingest).
- **`alert`** — replaces the `.state-alert` error banner (used across admin/data/ingest).

Generate with `shadcn add`, then repoint `--background`/etc. to our `bg`/`fg`/`accent`/`border`
tokens the way `components/ui/sidebar.tsx` already is (see the token-drift risk below).

## The work order (28 views, waves by traffic + the request)

The user named **channels, rules, dashboard** first; those anchor Waves 1–2. Each row =
one PR-sized migration + its gateway test + one `LEGACY_VIEWS` deletion.

### Wave 0 — unblock (primitives + close the lint gap)
- Generate `alert`, `dialog`, `switch` and token-bind them.
- **Add `src/features/rules/RulesView.tsx` to the standard's coverage.** It uses raw controls
  but is *absent* from `LEGACY_VIEWS` — either it's already erroring lint or slipped in
  unguarded. Migrate it (below) and confirm lint is clean, or explicitly allowlist-then-migrate.

### Wave 1 — channels + rules (the loudest surfaces the user called out)
- `channel/ChannelView.tsx`
- `channel/ChannelList.tsx`
- `channel/MessageComposer.tsx`
- `channel/palette/CommandPalette.tsx`
- `channel/palette/argWidgets/SqlArg.tsx`
- `channel/query/QueryCard.tsx`
- `rules/RulesView.tsx`  *(coordinate with `rules-workbench-scope.md` / `rules-editor-ux-scope.md`)*
- `confirm/ConfirmDestructive.tsx`  *(needs `dialog`; shared by many surfaces — do early)*

### Wave 2 — dashboard
- `dashboard/DashboardView.tsx`  *(rebuild header on `AppPageHeader`; it's the worst offender)*
- `dashboard/Grid.tsx`
- `dashboard/AddWidget.tsx`
- `dashboard/DashboardRoster.tsx`
- `dashboard/vars/VariableEditor.tsx`

### Wave 3 — admin + ingest tables (hardest responsive work; needs `table`/`switch`/`dialog`)
- `admin/PeopleAdmin.tsx`, `admin/RolesAdmin.tsx`, `admin/TeamsAdmin.tsx`,
  `admin/WorkspacesAdmin.tsx`  *(the `w-1/2` splits → `md:` stacks; see `admin-console-scope.md`)*
- `ingest/IngestView.tsx`, `ingest/SchemaBuilder.tsx`, `ingest/SchemaForm.tsx` (`switch`),
  `ingest/CreateSeriesWizard.tsx` (`dialog`)
- `data/DataView.tsx`

### Wave 4 — the remaining single-view surfaces
- `inbox/InboxView.tsx`, `outbox/OutboxView.tsx`, `workflow/WorkflowView.tsx`,
  `agent/AgentView.tsx`, `studio/StudioView.tsx`, `session/LoginView.tsx`,
  `workspace/WorkspaceSwitcher.tsx`

### Wave 5 — close it out
- Confirm `LEGACY_VIEWS` is empty; flip `lint` script to `eslint src --max-warnings 0`.
- Delete every `globals.css` control class that now has zero callers (the deletion is the
  proof the fork closed). Update `ui-standards-scope.md` open questions.

## How it fits the core

Presentation layer only. **MCP/data/bus/tenancy/capabilities: N/A** — no tools, records,
subjects, or grants change. Cap-gating stays in `NavRail`'s `allowed`/`extSlots` + the gateway
re-check. **FILE-LAYOUT:** each primitive and each view/sub-view stays one component per file —
this reinforces it. **No mocks (CLAUDE §9):** restyles keep the existing `*.gateway.test.tsx`
running against a real seeded node — never swap in a fake to make a snapshot pass.

## Testing plan

Per `scope/testing/testing-scope.md`, the categories that apply to a presentation change:

- **Gateway component tests (real node).** Every migrated view keeps/extends its
  `*.gateway.test.tsx`; behavior (post a message, add a member, approve, run a flow) passes
  unchanged after the restyle. This is the "no mocks" guarantee for UI.
- **Responsive assertion.** Add a narrow-container render (assert the mobile branch) for at
  least the shell + one representative migrated view per wave, so "resizes" is regression-guarded.
- **Capability-deny / workspace-isolation.** Inherited, not re-proven — a restyle must not
  drop `NavRail`'s cap-gating; the gateway re-checks regardless.
- **a11y smoke.** `aria-label`/`aria-current`/roles are preserved as Flows/NavRail set them.

## Risks & hard problems

- **Two systems during migration.** Legacy + shadcn coexist until the classes are deleted; an
  agent can copy the wrong one. Mitigation: this plan + the ESLint freeze + "Flows is the model."
- **Token drift in generated primitives.** shadcn CLI ships its own `--background` vars; each
  new primitive (`dialog`/`switch`/`alert`) must be repointed at our tokens or it looks off.
- **Dense tables on mobile (Wave 3).** admin/data tables are the hardest; a `Table` inside a
  horizontal-scroll card is likely more pragmatic than full reflow.
- **`ConfirmDestructive`/`Dialog` focus management** needs real narrow-viewport testing, not CSS.
- **`RulesView` is unguarded.** Confirm whether it currently errors lint before assuming the
  freeze is intact.

## Open questions

- **`card` for dashboard widgets or bespoke chrome?** Decide when migrating `dashboard/widgets/`.
- **Mobile nav entry point.** Where does the off-canvas `SidebarTrigger` live in each header —
  a slot in `AppPageHeader`?
- **Run `shadcn add` vs hand-author** the missing three? (Lean: CLI, then repoint tokens.)
- **Wire `pnpm lint` into CI / the gateway-test job** so the freeze runs on every change.

## Related

- `scope/frontend/ui-standards-scope.md` — the standard this plan executes.
- `scope/frontend/ui-design-scope.md`, `frontend-scope.md` — look/tokens + shell plan.
- `scope/frontend/admin-console-scope.md`, `dashboard-scope.md`, `data-console-scope.md`,
  `rules-workbench-scope.md`, `rules-editor-ux-scope.md` — surfaces with the most legacy work.
- Canonical code: `ui/src/features/flows/FlowsView.tsx`,
  `ui/src/components/app/page-header.tsx`, `ui/src/components/ui/sidebar.tsx`,
  `ui/eslint.config.js` (`LEGACY_VIEWS`).
- README `§3`, FILE-LAYOUT §4.
