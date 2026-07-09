# Frontend scope — the new-panel wizard makes "bind this panel to a saved RULE" a one-glance, labelled path

Status: **SHIPPED (2026-07-09)**. The recommendation below (keep the three cards; fix the scent) was
approved and built: the Workspace-source card subtitle front-loads "rule / series / saved query", the
Rules group leads the workspace source list, and an empty workspace shows an honest line. Proven by
`ui/src/features/panel-builder/wizard/sourceStepRuleDiscoverability.gateway.test.tsx` (6 tests, green).
Durable behaviour promoted to
[`public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md).

Child of [`rules-for-widgets-scope.md`](rules-for-widgets-scope.md) → grandchild of
[`rules-as-source-scope.md`](rules-as-source-scope.md). Those two shipped the **picker** and the
**render** halves: a saved rule *is* a selectable panel source
(`{tool:"rules.run", args:{rule_id, route:false}}`, Rules group, typed params) and a panel bound to
one now draws real rows. This scope closes the **discoverability** half they left open — the parent
scope's line "The wizard gets it for free … the Rules group appears there with no wizard change"
([`rules-for-widgets-scope.md`](rules-for-widgets-scope.md), Goals) turned out to be the bug: the
group *renders*, but it is buried where a user looking to bind a rule never looks.

## The confusion (from real screenshots)

Wizard step **1. Source** — "Where does this panel read from? Pick one." — offers three cards:

| Card | Subtitle |
| --- | --- |
| **Insights** | "A triage list of findings from rules, flows & agents. No data source needed." |
| **Workspace source** | "A series, saved query, **or rule** already in this workspace." |
| **Datasource** | "Author a query against a registered datasource, with saved queries." |

A user who wants to bind the panel to a saved **rule** finds **no card labelled "Rule"**. "Rule" is a
three-word tail in the *Workspace source* subtitle — easy to miss. In the screenshots the user instead
clicked **Datasource** (the only card that *names* querying), landed in the SQL/PRQL `QueryWorkbench`,
and got stuck — "As I see NOW to do this, should I be a source???". They could not find where a saved
rule is selected. The capability exists and works; this is purely a **labelling / information-scent**
gap in the wizard's first step.

## The real code (what happens today)

- **Three cards, one flattened combobox.**
  [`ui/src/features/panel-builder/wizard/SourceStep.tsx`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx)
  defines the three `Track`s — `insights` / `workspace` / `datasource`
  ([`SourceStep.tsx:44`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L44), card list
  [`:95-122`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L95-L122)). The
  "Workspace source" card's subtitle is the only place "rule" appears
  ([`:107`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L107)).
- **Picking "Workspace source" reveals a single `SourceCombobox`**
  ([`:259-274`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L259-L274)) that flattens
  **seven** groups behind one control, filtered only to drop `flows`
  (`READ_SOURCE_GROUPS.filter(({ group }) => group !== "flows")`).
- **The Rules group is real but one of seven.**
  [`READ_SOURCE_GROUPS`](../../../../packages/source-picker/src/SourcePicker.tsx) lists
  `Series / Live (Zenoh) / Direct SurrealDB / Installed extension / Extension widgets / Flows / Rules /
  Saved queries` ([`SourcePicker.tsx:21-30`](../../../../packages/source-picker/src/SourcePicker.tsx#L21-L30)).
  Under "Workspace source" the user sees six group headings; **"Rules" is the seventh-listed, near the
  bottom**, with no signpost that a rule lives there.
- **The rule entry itself is correct.** Each saved rule becomes a `rules` group entry emitting
  `{tool:"rules.run", args:{rule_id, route:false}}`
  (`rulesEntries`, [`packages/source-picker/src/sourcePicker.ts`](../../../../packages/source-picker/src/sourcePicker.ts);
  loader wiring [`useSourcePicker.ts:49-51`](../../../../ui/src/features/dashboard/builder/useSourcePicker.ts#L49-L51)).
  `selectEntry` in the wizard adopts it verbatim
  ([`SourceStep.tsx:152-163`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L152-L163)).
- **The other tracks flow cleanly**, which is why the mislabel stings: Insights is a one-click complete
  choice that auto-advances
  ([`SourceStep.tsx:127-133`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L127-L133),
  `InsightsBasics.tsx`); Datasource opens a named `<Select>` of registered datasources then the full
  `QueryWorkbench` ([`SourceStep.tsx:276-311`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L276-L311)).
  Only the rule path has no named entry point.

**Diagnosis:** the click cost is already ≤2 (Workspace source → open combobox → pick under "Rules"),
but the *scent* is zero. A user scanning three card titles + subtitles sees "Insights", a vague
"Workspace source", and "Datasource"; nothing says "Rule". They optimize on the wrong card.

## The question

Should a saved rule be a **first-class top-level card** in step 1 (Insights / Series / Saved query /
**Rule** / Datasource), **or** should "Workspace source" **expand into a clearly-labelled sub-picker**
where Rule is a named, obvious group (not a subtitle mention)?

### Recommendation: keep the card grouping; fix the scent — a named "Rule" affordance inside Workspace source, plus a subtitle rewrite

Adopt **Option B (labelled sub-picker)** with a targeted tightening, **not** Option A (a 5th card).

Concretely, three small changes, all pure labelling/layout inside `SourceStep.tsx` — no new track, no
per-source branching in the flow logic (CLAUDE §10 held: source kinds stay generic; only the labels and
group ordering change):

1. **Rewrite the "Workspace source" card so the reader sees its contents, not a catch-all.** Lead the
   subtitle with the three *named* things it contains, rules first among equals:
   *"Bind to a saved **rule**, **series**, or **saved query** already in this workspace."* The word the
   user is hunting is now in the card, front-loaded — the scent exists at the card scan.
2. **When Workspace source is chosen, surface the groups as named sub-choices, not a bare combobox.**
   Reorder `READ_SOURCE_GROUPS` for this surface so **Rules leads** (Rules / Series / Saved queries / …),
   and render the group headings prominently enough that "Rules" is visible at a glance — the combobox
   already groups; the fix is *order + heading legibility*, so a user who clicked in on the word "rule"
   immediately sees the "Rules" heading rather than scrolling past six other groups.
3. **Give the empty-Rules case an honest line.** A workspace with no saved rules (or without the
   `mcp:rules.list:call` grant → deny-tolerant empty group,
   [`useSourcePicker.ts:49-51`](../../../../ui/src/features/dashboard/builder/useSourcePicker.ts#L49-L51))
   should say "No saved rules yet — create one in Rules" rather than silently omitting the heading, so
   the path is discoverable even before the first rule exists.

**Why this over a 5th card (Option A, rejected):**

- **A 5th card fractures a single generic seam into per-kind UI.** "Workspace source" is exactly the
  set of `rules.run` / `series.read` / `query.run` read tools the shipped picker already unifies under
  one combobox. Promoting *rule* to its own card invites the same for *series* and *saved query* (why is
  one workspace read a card and the others a sub-list?), and now the wizard has five cards, three of which
  are the same generic `useSourcePicker` surface sliced by group. That is the noun-explosion CLAUDE §10
  warns against and `source-picker-package-scope.md` deliberately collapsed — one picker vocabulary,
  many groups. The bug is scent, not structure; fix scent.
- **The card row stays scannable.** Three intent-level buckets — *pre-built triage* (Insights),
  *something already in this workspace* (Workspace source), *author against an external datasource*
  (Datasource) — is the right first cut. Five cards where two pairs are near-synonyms is *more* to
  scan, not less; it trades one buried label for four competing ones.
- **It generalizes for free.** The same reorder+heading fix makes *series* and *saved query* equally
  glanceable inside Workspace source — no rule special-case, and the next group added to the picker
  (a future read kind) inherits the treatment with zero wizard edits.

**Why not "leave the combobox, only rewrite the subtitle":** the subtitle rewrite alone gets the user to
click the right card, but they still land on a seven-group combobox with Rules seventh — the second half
of the confusion. Reordering Rules to lead + a legible heading is what makes it *one glance* after the
click, which is the bar the ask set.

### Option A — a first-class "Rule" card (rejected, recorded for the why)

Add Rule (and by parity Series, Saved query) as top-level cards, so step 1 reads Insights / Series /
Saved query / Rule / Datasource. **Pro:** maximal scent — the word "Rule" is a card title. **Con:** it
special-cases three groups of one generic picker into bespoke cards, breaks the three-bucket scan, and
sets a precedent every future read-source group must either follow (more cards) or violate
(inconsistent). Rejected: it buys discoverability by dismantling the generic seam the picker package was
built to be.

## Non-goals

- **No rule authoring in the wizard.** Unchanged from the parent scopes — the wizard *consumes* a saved
  rule by id; authoring stays in the Rules workbench. The empty-state line links out to Rules, it does
  not open an editor inline.
- **No new track / no flow-logic branch.** `Track` stays `insights | workspace | datasource`; the
  derived-track logic ([`SourceStep.tsx:86-93`](../../../../ui/src/features/panel-builder/wizard/SourceStep.tsx#L86-L93))
  is untouched. Rules remain a *group within* the workspace track.
- **No per-source special-casing beyond labelling** (CLAUDE §10). The reorder is a data change to a
  group-label list for one surface; the wizard still treats every source kind through the same
  `selectEntry` → `targetFromEntry` path.
- **No new caps / no backend change.** Discoverability only; `rules.list` / `rules.run` and their grants
  are as shipped.

## Testing plan

Per [`docs/scope/testing/testing-scope.md`](../../testing/testing-scope.md) — a **real** gateway
interaction test (no fakes, rule 9), extending the existing wizard-source suite
([`ui/src/features/panel-builder/wizard/panelWizard.gateway.test.tsx`](../../../../ui/src/features/panel-builder/wizard/panelWizard.gateway.test.tsx),
which already drives `source track workspace` → `wizard source` → `rules.run`) and the shipped
[`ui/src/features/dashboard/builder/rulesSource.gateway.test.tsx`](../../../../ui/src/features/dashboard/builder/rulesSource.gateway.test.tsx).

New file: `ui/src/features/panel-builder/wizard/sourceStepRuleDiscoverability.gateway.test.tsx`, against
a spawned gateway node (`pnpm test:gateway`) seeded with a **real** saved rule
(`rules.save`) — no `*.fake.ts`. It must prove:

1. **≤2 clicks from step 1 to the rule picker.** From a fresh wizard on step "1. Source", click the
   card whose accessible name/text carries "rule" (the rewritten Workspace-source card), then the source
   combobox — the seeded rule is offered under a **visible "Rules" heading that is the first group** in
   the list (assert the "Rules" `optgroup`/heading precedes "Series"/"Saved queries"). No third click,
   no scroll-past-six-groups.
2. **The emitted source is exactly the shipped shape.** After picking the rule entry, assert
   `state.targets[0]` (via the wizard's `picked:` readout / save payload) is
   `{tool:"rules.run", args:{rule_id:"<seeded id>", route:false}}` — the render half's contract,
   unchanged.
3. **Card scent regression.** Assert the Workspace-source card's accessible text contains "rule"
   (front-loaded), guarding the subtitle rewrite so a future edit can't silently re-bury it.
4. **Empty-Rules honesty.** With a workspace that has **no** saved rules, choosing Workspace source →
   Rules shows the "No saved rules yet" line (not a missing heading) — the deny-tolerant / empty path is
   still discoverable.
5. **Mandatory platform tests** (rule per CLAUDE / testing-scope):
   - **Capability-deny:** a workspace without `mcp:rules.list:call` sees an empty Rules group with the
     empty-state line — no crash, no leak of rule ids — exercising the deny-tolerant loader
     ([`useSourcePicker.ts:49-51`](../../../../ui/src/features/dashboard/builder/useSourcePicker.ts#L49-L51)).
   - **Workspace isolation:** a rule saved in workspace **A** is **not** offered in the wizard opened in
     workspace **B** (the picker is `ws`-scoped, [`useSourcePicker.ts`](../../../../ui/src/features/dashboard/builder/useSourcePicker.ts)).

Green output from `pnpm test:gateway` (this file) is the definition of done for the *build* pass that
follows approval.

## Deliverables of the build pass (after approval — not this pass)

- The three labelling/ordering changes in `SourceStep.tsx` (card subtitle, Rules-first group order for
  the workspace surface, empty-Rules line). No new files unless the reordered group list warrants a
  small named constant (e.g. `WORKSPACE_SOURCE_GROUPS`) beside `READ_SOURCE_GROUPS`.
- The gateway test above, green.
- Promote the shipped behaviour into
  [`doc-site/content/public/frontend/dashboard.md`](../../../public/frontend/dashboard.md) and flip the
  index line below to "SHIPPED".
- A `docs/sessions/frontend/panel-wizard-source-discoverability-session.md` working log; a
  `docs/debugging/frontend/` entry only if something breaks.

## Open questions

1. **Group order for the whole workspace surface.** Rules-first is right for *this* ask, but Series is
   the most common workspace read overall. Options: (a) Rules-first only when the user arrived via the
   rule-scented card (context-sensitive) vs (b) a fixed Rules / Series / Saved queries order everywhere.
   Lean (b) for one predictable order; revisit if telemetry shows series-pickers scrolling.
2. **Should Insights' subtitle also name rules?** It already says "findings from rules, flows & agents",
   which is a *different* rule relationship (findings, not `output` rows — see
   [`rules-for-widgets-scope.md`](rules-for-widgets-scope.md) Non-goals). Likely leave as-is, but a
   user might conflate the two; worth a usability check.
