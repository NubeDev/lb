# Session — render-widgets skill: template-first routing + schema-first querying

Date: 2026-07-07 · Branch: `insights-v1` · Corpus version bumped 0.1.6 → **0.1.7**

## The problem

Two live failures when the dock agent is asked "make me a render widget using jsx/html":

1. **Wrong view.** The agent built a named view (barchart/table) instead of `view:"template"`
   with `options.code`. The skill's template section existed but sat last; the model anchored
   on the named-view choreography that came first.
2. **Guessed schema.** Against `demo-buildings` the agent guessed columns
   (`No field named r.id`), then fell back to synthetic `store.query` inline rows — valid JSON,
   but not the datasource the user asked for.

## What changed (docs/skills/render-widgets/SKILL.md only, + version bump)

- **"⚡ ROUTE FIRST" section at the very top**: jsx/html/template/custom/markup → `view:"template"`,
  explicitly "a named view is the WRONG answer even if the data would fit one". Named views are
  now the *fallback* for standard/generic asks. The frontmatter description leads with the same rule
  (the description is what the model sees at skill-selection time).
- **Schema-first, twice**: choreography step 2 and the template authoring workflow both now say
  call `federation.schema { source }` BEFORE `federation.query` and write SQL against the returned
  names only. The template workflow adds: if the query errors, fix the SQL against the schema —
  **never** fall back to synthetic inline rows.
- `rust/node/Cargo.toml` 0.1.6 → 0.1.7 so the idempotent boot seeder re-seeds the corpus.

Alternative rejected: splitting into two skills (`core.render-templates` + `core.render-widgets`)
for unambiguous trigger match. Kept one skill first — the routing header is cheaper and the two
paths share the whole fence/pin/capability story; split only if routing still fails live.

## Verified green

- `cargo test -p lb-host --lib channel::widget_extract` — 9 passed
- `cargo test -p lb-host --test channel_agent_worker_test` — 14 passed
- `cargo test -p lb-host --test widget_pin_test` — 12 passed
- `cargo build -p node` — corpus compiles in
- Dev node restarted: `boot: seeded 37 core skills @0.1.7` incl. `core.render-widgets`, granted in `acme`
- `ui e2e/agent-dock-render-widget-preview.spec.ts` — 1 passed (dock render path)

## Still open (needs a live GLM run to confirm)

- Does the routing header actually make GLM pick `view:"template"` for "jsx/html"? Test in a **new**
  dock session on the datasources page (persona = data-analyst).
- GLM swallowed-answer bug remains intermittent
  (`docs/debugging/agent/run-finished-empty-after-tool-work-answers-with-preamble.md`).
- If routing still fails: split the skill in two (the fallback lever noted above).

## Follow-up (same day): "render widget" trigger + pin widget-name

Live transcript confirmed the template routing + schema-first + prove-first fixes WORK
(view:"template", proper joins, proven query, self-recovered from two SQL dialect errors).
Additional changes, corpus 0.1.8 → **0.1.9**:

- Skill + both persona identities: "render widget" / "render template" are now first-class
  triggers for view:"template" (the earlier trigger list needed a jsx/html keyword; the user's
  natural phrase is "render widget").
- Skill: documented the federation SQL dialect gotcha (window fn over an aggregate in the same
  SELECT fails — aggregate in a subquery first) and the envelope `title` field.
- **Pin widget-name** (the user ask): `mint_cell_from_envelope` now reads `envelope.title` into
  the cell title (`rust/crates/host/src/dashboard/pin.rs`; the reusable panel already fell back
  to it); the pin descriptor documents `title`; `PinToDashboard.tsx` grew a "Widget name
  (optional)" input (prefilled from the envelope's own title) that injects `title` into the
  envelope. No new verb, no client-side cell construction — the host still mints.
- Tests: new `pin_envelope_title_names_the_cell_and_the_panel` (widget_pin_test, now 13) and the
  real-gateway UI test types a widget name and asserts the reloaded cell title. Both green;
  channel_agent_worker_test 14 green.
