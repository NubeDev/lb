# Frontend scope — the webhooks admin page (adopt the `AppPage` shell)

Status: **scope** (the ask). Promotes to `public/frontend/webhooks-admin.md` once shipped.
Target stage: **S9+ collaboration UI** — a frontend-only restyle/UX slice over the
**shipped** webhooks backend ([`public/ingest/webhooks.md`](../ingest/webhooks.md),
[`scope/ingest/webhooks-scope.md`](../ingest/webhooks-scope.md)). **No host changes, no new
MCP verbs, no new caps, no new tables.** The verbs this page calls
(`webhook.create/list/get/revoke/rotate`) and the inbound route `/hooks/{ws}/{id}` already
exist and are gateway-tested; this scope is the page's **look + wizard UX**.

The webhooks admin page (`ui/src/features/admin/WebhooksAdmin.tsx`) shipped with a flat
`AdminPanel` header (icon · title · action · plain workspace text — see
[`AdminPanel.tsx`](../../../ui/src/features/admin/AdminPanel.tsx)), so it reads visually
**distinct** from the surfaces a user opens around it — **Dashboards** and **Rules**, which
both wrap in the canonical [`AppPage`](../../../ui/src/components/app/page.tsx) shell (accent
gradient header, boxed icon, title + description, workspace chip, settings link, page-transition
`Reveal`). The user-visible symptom is exactly what the title says: *the webhooks page doesn't
match how the dashboard and rules pages look*. This scope closes that gap. Every full-screen
surface in the shell must obey [`ui-standards-scope.md`](ui-standards-scope.md) rule 2
("Header is `AppPageHeader`"); the webhooks page is the most visible offender and the trigger
for this slice. The same gap exists for every sibling admin tab (People · Teams · Roles ·
Workspaces · API Keys) — they all share `AdminPanel` — and is named below as the **follow-up
migration**, not silently folded in.

## Goals

- **The webhooks page adopts `AppPage`.** `WebhooksAdmin` wraps in
  [`AppPage`](../../../ui/src/components/app/page.tsx) (`label="webhooks admin"`,
  `icon={Webhook}`, `title="Webhooks"`, `description="…receive inbound HTTP as ingest samples…"`),
  so its header is the same `AppPageHeader` (accent wash, boxed icon, two-hue hairline,
  workspace chip, settings link) the dashboard and rules pages render. The page-transition
  `Reveal` (gated by the member's motion pref) carries over for free.
- **The wizard upgrades to the surface discipline.** The create flow (name → auth mode →
  optional HMAC header → create) and the **one-time secret banner** (the `lbk_…` bearer OR the
  shared secret, shown once with a copy button + "you won't see this again") keep their
  behavior but read as part of the page: same shadcn primitives, same tokens, the create form
  no longer wrapped in a stray `bg-panel` box that fights the page chrome. The **New webhook**
  action lives in the `AppPage` header's `actions` slot (where Dashboards puts *Delete* and
  Rules puts *Run*/*Save*), not in a per-tab `AdminPanel` `action` prop.
- **The roster table reads like the dashboard roster.** `Name · Mode · URL · Status · actions`
  stays, but the table is the project's token-bound table styling (no raw `text-xs`/`border-b`
  sprinkled inline), status uses the shared `Badge` (active / revoked), and the row actions
  (Rotate · Revoke) are the shadcn `Button` variants already used elsewhere — `ghost` for
  Rotate, `destructive` for Revoke. The empty state is the shared `AppEmptyState` (the
  dashboard's empty card), not a bare `<p>` sentence.
- **One workspace badge, not two.** Today the page shows the workspace both via the NavRail
  and as plain `{ws}` text inside `AdminPanel`. `AppPage`'s `WorkspaceBadge` (the chip with
  the accent dot) is the one canonical place; the duplicate is removed.
- **Mobile/responsive.** The page resizes cleanly to ≤640px: the header actions collapse
  (the New webhook button stays; the description hides), the table scrolls horizontally
  inside a card rather than forcing the page wide, the wizard form stacks. (Inherited
  requirement from `ui-standards-scope.md` rule 3 — the admin tabs are the most-flagged
  offenders on mobile.)

## Non-goals

- **No backend changes.** No new `webhook.*` verbs, no cap changes, no route changes, no
  record-shape changes. The page is a caller of the shipped, gateway-tested surface. If the
  wizard reveals a real backend gap, that is a **named follow-up** for `scope/ingest/`, not a
  silent widening here.
- **No new functionality.** The wizard keeps its two modes (`bearer`/`signature`), one-time
  secret, rotate, revoke, list — exactly the verbs that ship today. This slice is a **restyle
  + layout/UX polish**, not "more webhooks features."
- **No migration of the other admin tabs in this scope.** `PeopleAdmin`/`TeamsAdmin`/
  `RolesAdmin`/`WorkspacesAdmin`/`ApiKeysAdmin` all share `AdminPanel` and have the same gap.
  They are the named follow-up under [`admin-console-scope.md`](admin-console-scope.md) (see its
  Related) — folding them
  in here would make the slice unreviewable and break the "one ask per scope" rule.
- **No removal of `AdminPanel` itself.** It still has callers (the five sibling tabs above +
  the AdminView tab shell). It retires when its **last caller** migrates — the standard
  "delete on last caller" from `ui-standards-scope.md`. This slice is one caller migrating.
- **No new tests against the gateway verbs.** The verbs are already covered
  (`WebhooksAdmin.gateway.test.tsx` exercises create → list → rotate → revoke over a real
  gateway). This slice's tests are **presentation** assertions (the right shell renders, the
  one-time secret banner shows once, the empty state is honest, the page is mobile-wide) —
  not re-proving the verb path.
- **No provider-specific UX.** A webhook is a **generic** inlet (`scope/ingest/webhooks-scope.md`
  non-goal 1, rule 10). The wizard shows two auth modes and a header-name field; it never
  offers a "GitHub" or "Slack" preset. Same line the backend holds.

## Intent / approach

**Migrate one admin tab onto the canonical shell, leaving behavior intact.** The webhooks page
already does the right thing semantically (cap-gated verbs, one-time secret, rotate/revoke);
the work is to re-host it in `AppPage` and bring its sub-pieces (wizard, banner, table, empty
state) onto the shared primitives. The mechanical shape:

```
features/admin/WebhooksAdmin.tsx
  ├─ <AppPage label="webhooks admin" icon={Webhook} title="Webhooks"
  │          description="…" workspace={ws} error={error}
  │          actions={<Button>…New webhook</Button>}>      ◄── replaces AdminPanel(action=…)
  │    ├─ <OneTimeSecret …/>            (the post-create/rotate banner — already its own component)
  │    ├─ {creating && <WebhookCreateForm …/>}   (extract the inline fieldset → its own file)
  │    └─ <WebhookRoster …/>             (the table — extract from the inline <table>)
  │            └─ empty → <AppEmptyState …/>     (the shared empty card, not a bare <p>)
  │    </AppPage>
```

- **Extract sub-pieces during the migration (FILE-LAYOUT).** Today `WebhooksAdmin.tsx` holds
  the page, the create form, the table, and the one-time-secret banner in one file. The
  migration is the moment to split: `WebhooksAdmin.tsx` (page + wiring), `WebhookCreateForm.tsx`
  (the wizard step), `WebhookRoster.tsx` (the table + empty), `OneTimeSecret.tsx` (already
  isolated in spirit — promote it out of the same file). One verb per file; the file stays
  under the 400-line hard ceiling. `useWebhooks.ts` is already the hook and is unchanged.
- **Tokens, not literals.** The current inline `text-xs text-muted`, `border-border/50`,
  `bg-accent/10` ad-hoc classes get re-pointed at the shared token-bound table/`Badge`/
  `AppEmptyState` pieces — the same pieces the dashboard roster uses. No raw Tailwind palette
  colors (rule 4 in `ui-standards-scope.md`).
- **Action placement matches Dashboards/Rules.** `New webhook` lives in `AppPage`'s `actions`
  slot (right side of the header), not inside the body — same place Dashboards puts *Delete*
  and Rules puts *Run*/*Save*. The create form opens inline below the header (the way Rules
  reveals its name-on-save form), not as a modal — keeps the wizard in the reading flow.
- **The cap gate stays exactly where it is.** The page shows when the session holds
  `mcp:webhook.manage:call` (`lib/session/admin-caps.ts`); the gateway re-checks every verb.
  Moving from `AdminPanel` to `AppPage` does not change that — `AppPage` is markup only. The
  gate is in `NavRail`'s `allowed` (whether the nav item shows at all) and the gateway (the
  truth); the page renders the deny if a forged direct call gets that far (the existing
  `error` prop).

**Rejected alternatives:**

- *Polish `AdminPanel` to look more like `AppPageHeader`.* Rejected — that's the fork
  `ui-standards-scope.md` exists to close. Two header components that "almost match" is the
  breakdown; one canonical shell is the call. `AdminPanel` retires on its last caller, not by
  being slowly mutated toward `AppPage`.
- *Wait and migrate all admin tabs in one PR.* Rejected — the user pointed at webhooks
  specifically; bundling six tabs makes the slice unreviewable, breaks FILE-LAYOUT's "one ask
  per scope," and delays the most-flagged page. One tab now; the rest as a named follow-up.
- *Rewrite the wizard as a modal/dialog.* Rejected — the inline reveal matches Rules'
  name-on-save pattern (a workbench page reveals a form below the header, not a modal). A
  modal is a bigger change with its own a11y/focus concerns; the inline form already works.
- *Add provider presets (GitHub/Slack) while we're touching the wizard.* Rejected — rule 10
  and `scope/ingest/webhooks-scope.md` non-goal 1. The wizard stays generic; provider shaping
  is downstream.

## How it fits the core

- **Tenancy / isolation (rule 6):** unchanged. The page is a caller of already-ws-walled verbs
  (`webhook.list/get/create/revoke/rotate` operate in the session's workspace); the inbound
  route is `/hooks/{ws}/{id}`. The isolation tests are the **shipped** ones
  (`WebhooksAdmin.gateway.test.tsx` runs two-session over a real gateway); this slice adds no
  new data path to wall. **No new isolation test needed beyond the shipped one** (stated, not
  skipped) — the page is a *caller* of already-isolated verbs.
- **Capabilities (rule 5/7):** no new cap. The page renders for a session holding
  `mcp:webhook.manage:call`; the gateway re-checks every verb server-side (the UI gate is
  display convenience, never the boundary — mirrors `admin-console-scope.md`'s load-bearing
  rule). The deny path is the standard `error` strip on `AppPage` — the same place Dashboards
  renders a load error.
- **Symmetric nodes (rule 1):** no `if cloud`. The page is the same on Tauri and the browser;
  the verbs it calls are role-mounted already. `AppPage` is markup; it has no role branch.
- **MCP surface — consumed, not added (§6.1):** **no MCP tools added.** The slice consumes the
  shipped `webhook.create/list/get/revoke/rotate` and renders `webhook:{ws}:{id}` series
  metadata. No CRUD, live feed, or batch is introduced.
- **Data (SurrealDB):** no new tables, no new records, no schema change. The page reads what
  `webhook.list/get` already return (id, name, series, auth_mode, URL, status, created/last-
  hit — **never** the hash or shared secret). No `localStorage` durable state (rule 4); the
  create form, the open-row state, and the one-time-secret banner are transient React state.
- **Bus (Zenoh):** none — the roster is an administrative read (refresh on action), not a
  live feed. The hits themselves ride the ingest series stream (motion); a "live hits" panel
  on this page is a named follow-up, not part of this slice.
- **Secrets:** unchanged. The one-time-secret banner shows the credential **once** (returned
  by `webhook.create`/`webhook.rotate`), then it's discarded from UI state; the list **never**
  renders a hash or shared secret. The shipped Rust test pins the wire; this slice's UI test
  re-asserts the rendered DOM (no secret in the roster).
- **Stateless extensions:** N/A — the webhooks service is host-native; this slice is pure
  frontend.
- **Durability:** read-mostly UI. The verbs are state; no must-deliver effect originates here.
- **One responsibility per file (FILE-LAYOUT):** the migration is the moment to split the page
  into `WebhooksAdmin` (page + wiring) · `WebhookCreateForm` (wizard) · `WebhookRoster`
  (table + empty) · `OneTimeSecret` (banner). One component per file; the hook
  (`useWebhooks.ts`) and the api (`lib/admin/webhooks.api.ts`) are unchanged. No `utils`.
- **SDK/WIT impact:** none — pure frontend restyle over existing host verbs.
- **Skill doc:** **N/A** — no agent-/API-drivable surface is added or changed. The shipped
  webhooks verbs (and how to drive them) are already covered by the ingest/webhooks public
  doc; this slice changes no verb shape, route, or payload.

## Example flow

1. **Open.** A workspace-admin clicks **Webhooks** in the NavRail (it shows because the
   session holds `mcp:webhook.manage:call`). The page opens in `AppPage`: the boxed `Webhook`
   icon, **Webhooks** title, one-line description ("Receive inbound HTTP as ingest samples"),
   the workspace chip, the settings link, and a **New webhook** button on the right of the
   header. Visually identical in voice to the Dashboards and Rules pages beside it.
2. **Create.** They click **New webhook**. The create form reveals below the header (the way
   Rules reveals its name-on-save form): name · auth mode (`bearer` / `signature` segmented
   control) · signature header (only in `signature` mode) · **Create webhook**. They name it
   `plant-alerts`, pick `signature`, header `X-Signature`, click **Create webhook**.
3. **One-time secret.** `webhook.create` returns the URL + the shared secret. The
   `OneTimeSecret` banner renders at the top of the body: the inbound URL (always), the shared
   secret with a **Copy secret** button, and the "you won't see this again" line. They copy
   it, click **Dismiss**. The banner state is gone; the roster refetches and shows the new row.
4. **List.** The roster table renders the row: `plant-alerts · signature ·
   https://…/hooks/acme/wh_9f2… · active · [Rotate] [Revoke]`. Status is a `Badge`. A second
   webhook in `bearer` mode shows `bearer` and the same columns. The list **never** renders a
   hash, secret, `bearer_key_id`, or `secret_ref` — same wire pin as today, re-asserted by the
   UI test.
5. **Rotate.** They click **Rotate** on a row → `webhook.rotate` returns a fresh one-time
   secret → the `OneTimeSecret` banner appears again with the new secret; the old one is dead.
   The roster row stays (same URL, same series, same id).
6. **Revoke.** They click **Revoke** on a row → `webhook.revoke` tombstones it → the row's
   status becomes `revoked` and the Rotate/Revoke actions disappear. The URL 410s on the next
   hit (server-side, already shipped).
7. **Empty.** A workspace with no webhooks lands on the shared `AppEmptyState` card (icon,
   "Create a webhook", one-line help) — not a bare `<p>` sentence. Same empty voice as
   Dashboards' "Select or create a dashboard."
8. **Mobile.** At ≤640px the description hides, the table scrolls inside its card, the wizard
   form stacks, the workspace chip truncates. No horizontal page-scroll.

## Testing plan

Real store + real gateway + real `caps::check`, seeded with real webhook records via the real
`webhook.create` write path — **no mocks, no `*.fake.ts`** (CLAUDE §9). Mandatory categories
from `scope/testing/testing-scope.md` §2:

- **Capability-deny (mandatory, inherited):** the page does **not** render for a session
  lacking `mcp:webhook.manage:call` (the NavRail `allowed` gate); a forged direct call to any
  webhook verb is denied server-side (the UI gate is not the boundary). This is the **shipped**
  deny behavior; the test stays green after the restyle. Re-assert the rendered path: a
  non-managing session never sees the one-time-secret banner even if state were somehow set.
- **Workspace-isolation (mandatory, inherited):** a ws-B admin's webhooks page shows only
  ws-B webhooks; every verb against a ws-A id is denied/empty. The shipped two-session
  `WebhooksAdmin.gateway.test.tsx` stays green — the restyle changes no isolation surface.
- **No mocks / real seed:** every roster row is a real `webhook.create`; every rotate/revoke
  is a real verb. No fabricated list.

Plus this slice's specific cases (Vitest, real in-process gateway):

- **Shell renders `AppPage`.** The page mounts an `AppPage` with `label="webhooks admin"`,
  the `Webhook` icon, and the workspace chip — assert the header's distinguishing marks (the
  accent boxed icon, the workspace chip with the accent dot) are present. (Regression for
  "doesn't drift back to `AdminPanel`.") The `AdminPanel` import is gone from this file.
- **New webhook lives in the header.** The **New webhook** button is in the header actions
  slot, not the body — assert by role + position.
- **One-time secret shows once + dismisses.** After `webhook.create`, the banner shows the
  URL + the secret + a copy button + "you won't see this again"; dismiss clears it; the
  roster never re-renders the secret. (Re-asserts the wire pin at the DOM level.)
- **Roster renders the right columns and never a secret.** `Name · Mode · URL · Status` for
  each row; status uses `Badge`; no cell renders the hash/shared-secret/`bearer_key_id`/
  `secret_ref`. Rotate/Revoke hide on a `__revoked__` row.
- **Empty state is the shared card.** A workspace with zero webhooks renders the
  `AppEmptyState` (assert its distinguishing marks), not a bare paragraph.
- **Mobile/responsive.** At a narrow container (≤640px), no horizontal page-scroll; the
  description hides; the table region scrolls rather than forcing width. (The standard
  `ui-standards-scope.md` rule-3 check, applied to this page.)
- **`WebhooksAdmin.gateway.test.tsx` stays green** unchanged — the create → list → rotate →
  revoke → deny path over a real gateway is the behavior gate; the restyle must not regress
  it. (If an assertion breaks because a class name or label changed, update the assertion to
  the new label — do not weaken the behavior.)

## Risks & hard problems

- **`AdminPanel` is shared by five other admin tabs.** Migrating `WebhooksAdmin` off it does
  not delete it; the five siblings still import it. The risk is leaving the impression that
  `AdminPanel` is "the admin look" — it isn't anymore (it's the **legacy** admin look; the
  canonical shell is `AppPage`). Mitigation: a comment in `AdminPanel.tsx` marking it legacy
  + a cross-link to this scope, and the follow-up migration named in `admin-console-scope.md`.
- **Splitting the file during migration.** Today `WebhooksAdmin.tsx` is one file with the
  page, the form, the table, and the banner. The risk is a behavior-drop in the split (a lost
  prop, a changed label the gateway test relied on). Mitigation: split in small green steps
  (extract `OneTimeSecret` first — it's already self-contained; then `WebhookRoster`; then
  `WebhookCreateForm`), running the gateway test after each step.
- **The one-time-secret banner is load-bearing.** If the restyle drops the "you won't see this
  again" affordance, or the dismiss state, or the copy button, users **lose secrets** in
  practice. Mitigation: the banner is its own component (`OneTimeSecret.tsx`), its props are
  unchanged, and the UI test pins its marks (secret shown once, dismiss clears, roster never
  re-renders it).
- **Mobile density.** The roster table is the hardest piece at narrow widths (URL + status +
  two actions per row). Mitigation: horizontal scroll inside a card (the standard admin-table
  answer per `ui-standards-scope.md` "Responsive regressions in dense tables"), not a full
  reflow. Verify at ≤640px in the responsive test.
- **Drift back.** A future edit that re-wraps the page in `AdminPanel` (because "the other
  admin tabs look like that") would silently undo this slice. Mitigation: the ESLint rule in
  `ui-standards-scope.md` (legacy allowlist) and the shell-renders-`AppPage` regression test.

## Open questions

Decisions are **made** so the slice codes with no open question:

**Resolved (decisions taken):**

- **Adopt `AppPage`, do not polish `AdminPanel`.** The canonical shell is `AppPage`; this
  slice migrates one caller. Decided.
- **Webhooks only this slice; the other admin tabs are a named follow-up.** Bundling six tabs
  in one scope breaks "one ask per scope." Decided.
- **Inline create form (Rules-style), not a modal.** Matches the surrounding workbench pages;
  a modal is a bigger a11y/focus change with no win. Decided.
- **Split during migration (4 files: page / form / roster / banner).** FILE-LAYOUT compliance;
  the current single file is over the spirit of "one verb per file" even if under the 400-line
  hard ceiling. Decided.
- **No provider presets, no live-hits panel, no new verbs.** All out of scope (rule 10 +
  "no backend changes"). Decided.

**Named follow-ups (not silent gaps):**

- **Migrate the other five admin tabs (`People`/`Teams`/`Roles`/`Workspaces`/`API Keys`) onto
  `AppPage`** — the same gap, scoped as a follow-up under [`admin-console-scope.md`](admin-console-scope.md)
  (see its Related). When the last caller is gone, `AdminPanel.tsx` is deleted (standard
  delete-on-last-caller).
- **A "live hits" panel on the webhooks page** — show the recent `Sample`s on the selected
  hook's series (`series.read`/`series.watch` on `webhook:{ws}:{id}`). A genuine new feature
  (a motion surface on an admin page), so it's its own scope, not part of this restyle.
- **The deferred `webhook` flow source node** — already a named follow-up in
  `scope/ingest/webhooks-scope.md`; out of scope here.
- **Re-check whether `WebhooksAdmin.gateway.test.tsx` assertions need label updates** — if the
  restyle changes a button label the test asserts on, update the test to the new label; do
  not change the label back to make the test pass.

## Related

- [`scope/ingest/webhooks-scope.md`](../ingest/webhooks-scope.md) + [`public/ingest/webhooks.md`](../../public/ingest/webhooks.md) — the **shipped** backend this page drives
  (the verbs, the inbound route, the auth modes, the one-time-secret discipline).
- [`scope/frontend/ui-standards-scope.md`](ui-standards-scope.md) — the cross-cutting standard
  this slice enforces on one page (rule 2: "Header is `AppPageHeader`"; rule 3: responsive;
  rule 4: tokens not literals).
- [`scope/frontend/admin-console-scope.md`](admin-console-scope.md) — the sibling admin
  surfaces; the **follow-up migration** of the other five tabs onto `AppPage` lives there.
- [`scope/frontend/dashboard-scope.md`](dashboard-scope.md) + [`scope/frontend/rules-editor-ux-scope.md`](rules-editor-ux-scope.md) — the canonical `AppPage` surfaces this slice
  matches (Dashboards for roster + empty; Rules for inline-create-form + workspace chip).
- [`scope/frontend/frontend-scope.md`](frontend-scope.md) + [`scope/frontend/ui-design-scope.md`](ui-design-scope.md) — the shell plan + visual direction this inherits unchanged.
- Canonical code: [`ui/src/components/app/page.tsx`](../../../ui/src/components/app/page.tsx),
  [`ui/src/components/app/page-header.tsx`](../../../ui/src/components/app/page-header.tsx),
  [`ui/src/components/app/empty-state.tsx`](../../../ui/src/components/app/empty-state.tsx),
  [`ui/src/features/admin/WebhooksAdmin.tsx`](../../../ui/src/features/admin/WebhooksAdmin.tsx)
  (the file this slice migrates),
  [`ui/src/features/admin/AdminPanel.tsx`](../../../ui/src/features/admin/AdminPanel.tsx)
  (the legacy shell, retired on last caller).
- README **§3** (core principles), **§6.13** (frontend / extension UIs), FILE-LAYOUT §4 (one
  responsibility per file).
