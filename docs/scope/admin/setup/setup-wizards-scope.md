# Setup wizards scope — how (and why) to build a guided wizard, reuse-first

Status: scope (the ask) + playbook. This doc is written **for an AI agent** about to add or change a
wizard in the Setup tab. Read it before writing any wizard code.

> Read with: `../../extensions/extensions-scope.md` (rule 10 — core knows no extension),
> `../../testing/testing-scope.md` §0 (no fakes) + §2 (deny + isolation tests),
> `../../../FILE-LAYOUT.md` (one responsibility per file),
> `../../../ABOUT-DOCS.md` (every session writes docs + tests). Live code:
> `ui/src/features/admin/wizard/StepFlow.tsx` (the framework),
> `ui/src/features/admin/setup/` (the hub, the catalog, the wizards).

## What a "setup wizard" is, and why they exist

A **setup wizard** is a short, guided, multi-step flow that takes a user from "nothing" to "a working
thing", by **orchestrating verbs and editors that already exist** — never by adding a new backend or a
new editor. The Setup tab (`AdminView` → Setup) lands on an **icon-card picker** (`WizardPicker` over
`catalog.ts`); each card opens one wizard inside the shared `SetupHub`.

Why we build them:

- **They lower the activation cliff.** The platform is deep — People/Teams/Roles/Nav, theme/branding,
  ingest/keys. A newcomer shouldn't have to know which five tabs to visit in which order. A wizard is
  the *guided path* over surfaces that also stand alone.
- **They are pure orchestration, so they cost almost nothing and never drift.** Because a wizard reuses
  the exact same components and verbs the standalone tabs use, it can't fall out of sync with them and
  it can't invent a second, weaker code path. Onboarding uses the *real* People/Teams/Roles/Nav verbs;
  Appearance renders the *real* Theme/Layout/Sidebar/Branding editors; Ingest reuses the *real*
  `CreateSeriesWizard` + `useApiKeys`. One behavior, two homes.
- **They compose.** Adding a wizard is one catalog entry + one hub branch + a `FlowStep[]`. The
  framework owns stepping; you own only which existing pieces to sequence.

## THE HARD RULE: reuse existing code. This is a must, not a preference.

**A wizard MUST NOT reimplement a feature that already exists. If the surface it guides already has an
editor, hook, or verb, the wizard reuses that exact code.** This is the single most important rule in
this doc. It is the analog, for wizards, of CLAUDE §9 ("no mocks, no fake backends") and §10 ("core
knows no extension"): a hand-rolled second copy of an editor is a *fake of a real feature* — it lets
the wizard *look* done while diverging from the truth, and an AI can't tell the copy from the original.

Concretely, when the wizard touches an **existing** feature you **must**:

1. **Find the existing seam first.** Before writing a step, locate the component/hook/verb the
   standalone surface already uses. Grep the feature dir. If a self-contained editor exists
   (`ThemeTab`, `BrandingTab`, `SidebarTab`, `CreateSeriesWizard`), **render it verbatim**. If a hook
   owns the write (`useApiKeys`, `useSetup`), **call it** — do not re-issue its `invoke(...)` yourself.
2. **Reuse the verb, never re-describe it.** The wizard calls the same `lib/*.api.ts` function the tab
   calls (`saveSchema`, `createApiKey`, `joinTeam`, …). No new gateway route, no new host verb, no
   duplicated request shape. If you find yourself writing an `invoke("something", …)` that already
   exists elsewhere, stop and import that instead.
3. **If the existing editor isn't reusable, make it reusable — don't fork it.** If a component is
   page-coupled (owns its own load/save and can't be dropped elsewhere), the correct move is to lift a
   `value + onChange` (or a provider) out of it so **both** the tab and the wizard use one component —
   NOT to copy its markup into the wizard. A fork is a bug; an extraction is the fix. (See the
   Appearance case: the theme editors are provider-coupled, so the wizard renders them inside the
   existing `ThemeProvider` rather than re-deriving theme state.)
4. **Prove the reuse in the session doc.** Every wizard session ends with a **reuse ledger** table
   (see below) naming, per step, exactly what was reused and whether any new code was written. A row
   that says "reimplemented X" for an X that already exists is a failed review.

New code is allowed only for genuinely new glue: the step sequence, a prefilled snippet/string builder,
a small confirmation banner. If a step's body is more than thin framing around an existing piece, you're
probably rebuilding something — go find the original.

## The framework (reuse THIS to build a wizard)

`ui/src/features/admin/wizard/StepFlow.tsx` is the generic, domain-free wizard shell. **Every wizard
uses it; none re-implements stepping.**

- `StepFlow({ steps, finishLabel?, onFinish? })` — renders the step rail (jump back to any reached
  step), a scrolling body, and a Back / Continue / Finish footer. Owns *only* navigation state.
- `FlowStep` = `{ key, label, hint, render(ctx), canAdvance? }`. `render(ctx)` returns the step body;
  `ctx.next()/back()` let a step drive navigation from its own button; `canAdvance` gates the footer
  Next (use it to block advancing until a step's work is actually done — see the ingest lesson).
- `StepShell({ icon, title, blurb, children })` — the shared icon-badged step header. Use it so every
  step across every wizard reads identically.

The rail itself is `setup/Stepper.tsx` (already generic). Don't rebuild either.

## Anatomy of a wizard (the four files you touch)

1. **`setup/catalog.ts`** — add one `WizardEntry` (`id`, `title`, `blurb`, `icon`) and extend the
   `WizardId` union. The card grid renders whatever is listed; this is the *only* registry.
2. **`setup/<Name>Wizard.tsx`** — declare a `FlowStep[]` and hand it to `StepFlow`. Each step's
   `render` returns `<StepShell …>` wrapping a **reused** editor/hook. Gate steps/controls on caps for
   DISPLAY (`hasCap(caps, CAP.x)`); the gateway re-checks server-side (rule 5) — hiding is convenience,
   never the boundary.
3. **`setup/SetupHub.tsx`** — add one branch: `{active === "<id>" && <YourWizard … onDone={() =>
   setActive(null)} />}`.
4. **`<Name>Wizard.gateway.test.tsx`** — drive it against a **real** in-process gateway (no fakes; see
   testing §0). Assert the real effect landed (read it back over the gateway), not just that the UI
   rendered.

That's the whole surface. If you're editing more than these four files (plus a genuine extraction per
rule 3), you're either adding a real feature — which belongs in its own scope, not a wizard — or you're
duplicating something.

## Rules that still apply (a wizard is not an exception)

- **Rule 5 — capability-first.** A wizard SHOWS controls; the gateway is the wall. Gate step visibility
  and buttons on caps, but never rely on hiding for security. Every write is re-checked server-side.
- **Rule 6 — workspace is the hard wall.** The wizard operates in the session's workspace; it never
  passes a `ws` that widens the caller's reach.
- **Rule 10 — core knows no extension.** A wizard in `ui/src/features/admin` must not branch on an
  extension id. Reach extension pages/verbs only through the generic seams (`ext.list`,
  `<id>.<tool>` MCP dispatch). An "Ingest wizard" targets the generic `ingest.write`/`series.*` verbs;
  it never names a protocol bridge.
- **FILE-LAYOUT — one responsibility per file.** The wizard file owns the flow; each reused editor
  stays in its own file; a pure helper (e.g. a snippet builder) is its own file. No `utils.ts`.
- **ABOUT-DOCS — docs + tests are deliverables.** No wizard is "done" without a `sessions/…` entry
  (with the reuse ledger) and a green real-gateway test.

## The reuse ledger (required in every wizard session doc)

End the session doc with a table that makes the reuse auditable — one row per step:

| Step | Reused from (component / hook / verb) | New code written? |
|---|---|---|
| … | `features/…/ExistingEditor`, `lib/…/existing.api.ts` | none / a named glue file only |

A reviewer (human or AI) reads this table first. If any row reimplements an existing feature, the work
is rejected — reuse it or extract it, then update the row.

## Worked lessons (from the wizards already shipped)

- **Appearance wizard.** Theme/Layout editors are coupled to `ThemeProvider` (no per-control
  value/onChange). Reuse = render `ThemeTab`/`LayoutTab` inside the existing provider the app shell
  already mounts — NOT re-deriving theme state. `BrandingTab`/`SidebarTab` own their own load/save, so
  they drop in as-is. Zero editor duplication.
- **Ingest wizard — and the "it didn't actually create it" bug.** First cut carried the series *name*
  forward but persisted nothing on Continue, so the key minted but no series existed. The fix teaches
  the rule: **a step that claims to create a thing must actually persist it before `canAdvance`**, and
  the real-gateway test must **read the effect back** (here, `loadSchema` over the gateway), not just
  check the UI. It also surfaced a domain truth worth writing down: a series is defined by its schema
  record; a producer's first sample fills in data — so `series.list` won't show a schema-only series
  until data arrives ("ready to receive samples"). Reuse: `CreateSeriesWizard` + `saveSchema` +
  `useApiKeys` + the shared `CopyButton`; the only new code is `pythonSnippet.ts`, which mirrors
  `clients/python/example.py` so the copied code is the *real* client path, not an invented API.

## Non-goals

- **No new backend for a wizard.** If a wizard seems to need a verb that doesn't exist, that verb is a
  real feature with its own scope — build it there first, then orchestrate it.
- **No wizard-only editor.** If you need an editor, it must be usable outside the wizard too; put it in
  the feature that owns it and reuse it here.
- **No branching on extension ids** (rule 10). Wizards target generic, mediated seams only.
