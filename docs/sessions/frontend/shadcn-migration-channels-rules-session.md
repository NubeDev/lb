# Frontend — shadcn migration Wave 0/1 + reusable page-shell (channels · rules · dashboard) (session)

- Date: 2026-07-01
- Scope: ../../scope/frontend/shadcn-migration-scope.md (executes ../../scope/frontend/ui-standards-scope.md)
- Stage: S9+ (frontend standard) — no stage exit gate; presentation-only migration
- Status: done

## Phase 2 addendum — reusable page-shell + Wave 2 dashboard + channels/rules alignment

After Wave 1, the user flagged that **Dashboard and Rules didn't line up with Flows**: the fix was
structural, not spacing. Flows puts `AppPageHeader` at the **full width** with the rail *below* it;
Dashboard/Rules/Channels had the rail as a **full-height sibling** with a body-only header, so the
"New" control and the page title never shared a baseline. Extracted the Flows shape into three
reusable layout components and pointed every large surface at them:

- **`components/app/page.tsx` (`AppPage`)** — the canonical `section` + full-width `AppPageHeader` +
  optional destructive-token error strip + `min-h-0 flex-1` body row. Adopting it *guarantees* the
  alignment (header spans the width; rail drops below).
- **`components/app/rail.tsx` (`AppRail`)** — the shared roster aside (identical width/border/tone +
  a `min-h-[3rem]` header control-row slot).
- **`components/app/empty-state.tsx` (`AppEmptyState`)** — the shared accent-icon "select or create…"
  card.

Adopted in: **Flows** (`FlowsView`+`FlowRail`, no visual change — the reference), **Dashboard**
(`DashboardView`+`DashboardRoster` — Wave 2: restructured + header controls migrated to
`Button`/`Input`/`Select`/`Badge`; **removed from `LEGACY_VIEWS`**), **Rules** (`RulesView`+`RuleRail`
— restructured; already shadcn), and **Channels** (`ChannelView` — the channel rail (workspace
switcher + `ChannelList`) moved *into* the surface via `AppPage`, so the `#channel` header spans full
width; `RoutedShell` dropped its channels-only aside special-case; `ChannelsRoute` now passes
`onSelectChannel`/`onSwitchWorkspace`).

Per the user's ask, **deleted the "add new workspace" control** on the channel rail: `WorkspaceSwitcher`
lost its create-workspace `<form>` (input + "+") and was migrated to shadcn (`Select` + `Alert`,
legacy classes gone) — **removed from `LEGACY_VIEWS`**. The workspace *switch* (select) stays.

Also restored the workspace tooltip: `AppPageHeader`'s `WorkspaceBadge` now carries
`title="Workspace {ws}"` (Wave 1 had dropped the old `scope-pill` title that `App.gateway.test`
asserts via `getByTitle`).

**Verified live** (real gateway `node` on `:8080` + vite): Dashboard, Flows, Channels, Rules all
render the identical shape — full-width header, rail below with an aligned header row, accent
empty-state; the channel "new workspace" control is gone (screenshots taken this session).

**Tests/lint this phase:** `pnpm test` 268/268; `pnpm lint` 0 errors (150→**91** warnings);
`LEGACY_VIEWS` 28 → **17**. Migrated-view gateway tests pass in isolation
(`ChannelView` 5/5 incl. the responsive smoke, `RulesView` 7/7, `DashboardView` 6/6, all `flows`).
Fixed two Wave-1 regressions surfaced by `App.gateway.test` (my failing-file grep had excluded
`src/App.*` — lesson logged): the dropped workspace `title` (restored on the badge) and
`getAllByRole("listitem")` in the message-ordering test now scoping `within` the `messages` list
(the surface legitimately renders a second list — the channel roster). The remaining `App.gateway`
red is the **pre-existing `signInReal` "not a member of any workspace" seeding flake** (varies
run-to-run, present at HEAD, unrelated to layout).

---

## (Phase 1) Wave 0 + Wave 1

## Goal

Migrate the loudest legacy views onto shadcn/ui so every full-screen surface matches the
canonical **Flows** shape (`AppPageHeader`-led `section`, `min-h-0 flex-1` body, controls only
from `components/ui/*`). Execute **Wave 0** (unblock: generate the three missing primitives +
close the RulesView lint gap) and **Wave 1** (channels + rules + the shared confirm dialog).
Stop after Wave 1. Presentation only — no MCP tool / record / bus subject / capability change.

## What changed

**Wave 0 — primitives (generated, token-bound like `components/ui/sidebar.tsx`):**
- `ui/src/components/ui/alert.tsx` — `Alert`/`AlertTitle`/`AlertDescription`, `default` +
  `destructive` variants on our `border`/`bg-card`/`fg`/`destructive` tokens. Replaces `.state-alert`.
- `ui/src/components/ui/dialog.tsx` — on `@radix-ui/react-dialog` (already a dep — the same
  primitive `sheet.tsx` wraps), centered modal on `bg-panel`/`fg`/`border`. Focus-trap +
  Escape/overlay dismissal for free. `showClose` prop (default true). Replaces the hand-rolled
  `role="dialog"`.
- `ui/src/components/ui/switch.tsx` — hand-authored `role="switch"` button (we don't carry
  `@radix-ui/react-switch`; a button gives the same a11y contract), controlled/uncontrolled,
  `bg-accent`/`bg-border` tokens. Replaces hand-rolled `role="switch"` toggles.

**Wave 0 — RulesView lint gap:** `features/rules/RulesView.tsx` was *already* on the standard
(`AppPageHeader` + `Button`/`Input`) and *absent* from `LEGACY_VIEWS` — i.e. already guarded and
lint-clean. The only gap was one raw color on the dirty indicator: `text-amber-600` → `text-accent`
(amber *is* our accent). No `LEGACY_VIEWS` change needed (it was never in the list).

**Wave 1 — migrations (each: raw controls → `components/ui/*`, legacy globals.css classes gone,
raw colors → tokens; canonical section shape for full-page views):**
- `features/confirm/ConfirmDestructive.tsx` — the shared destructive-confirm dialog (used by
  admin ×4 + extensions). Hand-rolled `role="dialog"` → `Dialog`; raw buttons → `Button`
  (`variant="destructive"`/`"outline"`); type-to-confirm `<input>` → `Input`; second-gate
  `<input type="checkbox">` → `Switch`; reversible/irreversible pill → `Badge`; `bg-red-500`/
  `text-white`/`text-red-400` → destructive tokens. Dismiss-any-way (Escape/overlay) routes
  through `onCancel` (which does nothing but close) — a small a11y gain, behavior preserved.
- `features/channel/ChannelView.tsx` — `.page-header`/`.page-title`/`.page-subtitle`/`.scope-pill`
  → `AppPageHeader`; wrapped in the canonical `section aria-label="channel view"
  flex h-full min-w-0 flex-col bg-bg text-fg`; error strip → Flows-style destructive-token
  `role="alert"` strip. (The `bg-emerald-500` online-presence dot is kept — a semantic status
  green with no token equivalent.)
- `features/channel/ChannelList.tsx` — list-item `<button>` → `Button` (FlowRail roster shape),
  create `<input>`/`<button>` → `Input`/`Button`, error `<div>` → `Alert variant="destructive"`.
- `features/channel/MessageComposer.tsx` — `.control-field` `<input>` → `Input`, `.soft-button`
  `<button>` → `Button`.
- `features/channel/palette/CommandPalette.tsx` — both `<input>`s → `Input`, submit `<button>` →
  `Button`, `catalogError` `text-red-600` → `text-destructive`.
- `features/channel/palette/argWidgets/SqlArg.tsx` — `.control-field` `<textarea>` → `Textarea`,
  suggestion `<button>`s → `Button variant="outline"`.
- `features/channel/query/QueryCard.tsx` — chart/table toggle `<button>` → `Button`, query_error
  `text-red-600` → `text-destructive`.

**ESLint freeze advanced:** removed the 7 migrated paths (6 `channel/*` + `confirm/`) from
`LEGACY_VIEWS` in `ui/eslint.config.js` → they are now guarded as **errors** on regression.
`LEGACY_VIEWS`: **28 → 21**. Lint warnings: **150 → 120**.

## Decisions & alternatives

- **Copied Flows, not Dashboard.** Per the plan, `FlowsView`/`FlowRail` are the canonical shape;
  `ChannelList` follows the `FlowRail` roster-item pattern (ghost `Button` + token active state).
- **Dialog on Radix, not hand-rolled.** `@radix-ui/react-dialog` is already a dep (`sheet.tsx`
  uses it) — reusing it gives real focus management, which the scope explicitly wanted for
  `ConfirmDestructive`. Rejected keeping the bespoke backdrop `div` (no focus trap, duplicated).
- **Switch hand-authored, not `@radix-ui/react-switch`.** That package isn't installed and adding
  a dep for one primitive is overkill; a `role="switch"` button matches the a11y contract and the
  scope's sanctioned "generate by hand matching `sidebar.tsx`" path. `getByLabelText("acknowledge")`
  + click-to-toggle keeps working unchanged.
- **Second-gate checkbox → Switch.** The decision "boolean toggles → switch" applies; the ack is a
  boolean toggle. Tests query it by `aria-label`, not by `type=checkbox`, so it's transparent.
- **RRulesView not added-then-removed from `LEGACY_VIEWS`.** It was already conformant and unlisted
  (so already an *error* target). Only the raw-color gap remained; nothing to allowlist.
- **Kept the `bg-emerald-500` presence dot.** No green/success token exists; the amber accent would
  mislead ("online" ≠ accent). Left as a documented semantic exception, not a token violation.

## Tests

Presentation-only change — the applicable category from `scope/testing/testing-scope.md` is the
**gateway component test (real node, no mocks/fakes — CLAUDE §9)**: every migrated view keeps its
existing `*.gateway.test.tsx` green against a real seeded gateway, and I added a **responsive
regression** for the representative Wave 1 view. Capability-deny / workspace-isolation are
**inherited, not re-proven** (a restyle changes no cap-gating; the gateway re-checks regardless) —
and are still exercised by the unchanged ChannelList ws-isolation test and the admin deny tests
that render the migrated `ConfirmDestructive`.

- **New responsive test** (`ChannelView.gateway.test.tsx` → "does not overflow horizontally at a
  narrow (phone) viewport"): renders in a 360px container, asserts the canonical
  `min-w-0 flex-col` section's `scrollWidth ≤ 360` — matching the established SystemView pattern.

Migrated views + shared callers (real gateway):
```
✓ src/features/channel/ChannelView.gateway.test.tsx        (5 tests)   ← +1 responsive
✓ src/features/channel/ChannelList.gateway.test.tsx        (3 tests)
✓ src/features/channel/palette/CommandPalette.gateway.test.tsx (6 tests)
✓ src/features/rules/RulesView.gateway.test.tsx            (7 tests)
✓ src/features/confirm/ConfirmDestructive.test.tsx         (4 tests)
✓ src/features/admin/WorkspacesAdmin.gateway.test.tsx      (2 tests)   ← renders ConfirmDestructive
✓ src/features/admin/RolesAdmin.gateway.test.tsx           (3 tests)
✓ src/features/admin/TeamsAdmin.gateway.test.tsx           (2 tests)
✓ src/features/admin/PeopleAdmin.gateway.test.tsx          (3 tests)
✓ src/features/extensions/ExtensionsView.gateway.test.tsx  (4 tests)   ← renders ConfirmDestructive
```

Full suites:
```
pnpm test           → Test Files 33 passed (33) | Tests 242 passed (242)
pnpm lint           → ✖ 120 problems (0 errors, 120 warnings)   [was 150; 0 errors]
pnpm test:gateway   → Test Files 5 failed | 40 passed (45) | Tests 7 failed | 216 passed (223)
```

The 7 gateway failures are **pre-existing and outside this session's scope** — verified by running
them against a clean `git stash` of the working tree:
- `studio/StudioView.gateway.test.tsx` (1) + `system/SystemView.gateway.test.tsx` (1) **fail
  identically on baseline** (environmental: studio needs a wasm build toolchain; system needs live
  bus stats). I touched neither file.
- `workflow/WorkflowView.gateway.test.tsx` + others are a **cross-test seeding race**
  ("not a member of any workspace") — they **pass in isolation**. None touch channel/confirm/rules.
- No failure occurs in any `channel/*`, `confirm/`, or `rules/` test.

`tsc --noEmit`: the only errors are 2 pre-existing ones in `features/flows/FlowsCanvas.gateway.test.ts`
(confirmed present on baseline stash); zero in any file this session touched.

## Debugging

None — nothing broke that this session introduced. No `debugging/` entry opened. The two
StudioView/SystemView gateway failures were confirmed pre-existing (baseline stash reproduced them)
and are unrelated to this presentation change, so they are not this session's to fix.

## Public / scope updates

- No `public/` promotion yet — the migration is incremental; `public/frontend/frontend.md` gets
  promoted when `LEGACY_VIEWS` empties (Wave 5), per the plan.
- `scope/frontend/shadcn-migration-scope.md`: Wave 0 + Wave 1 are done; the open question
  "run `shadcn add` vs hand-author the missing three" is resolved for this session — **hand-authored
  token-bound** (alert/dialog on the existing radix-dialog dep, switch as a role="switch" button),
  matching `sidebar.tsx`, because the shadcn registry/CLI isn't reachable in this environment.
- `docs/STATUS.md` updated: Wave 0 + Wave 1 shipped, **Wave 2 (dashboard) is next up**,
  `LEGACY_VIEWS` 28 → 21.

## Dead ends / surprises

- Expected `RulesView` to be an unguarded lint offender (per the scope's "it's absent from
  `LEGACY_VIEWS`" warning). It was actually already migrated and lint-clean — the only debt was one
  `text-amber-600`. Reported rather than assumed.

## Follow-ups

- **Wave 2 (dashboard)** is the next unit — `DashboardView` (rebuild header on `AppPageHeader`),
  `Grid`, `AddWidget`, `DashboardRoster`, `vars/VariableEditor`. NOT started (stop-after-Wave-1).
- `switch`/`alert` gain more callers in later waves (ingest `SchemaForm`, admin/data banners).
- The pre-existing StudioView/SystemView gateway flakes deserve their own investigation (out of scope).
