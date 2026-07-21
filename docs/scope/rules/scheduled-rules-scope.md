# Rules scope — scheduled rules (a `#[schedule(...)]` directive, no canvas)

Status: scope (the ask). Promotes to `public/rules/rules.md` once shipped.

A user on the Rules page wants "run this rule every 15 minutes → it raises insights" **without ever
opening a flow canvas**. Today that requires hand-building a `cron → rule` flow. This scope lets a rule
**declare its own schedule** with a directive at the top of its body —
`#[schedule("every 15 minutes")]` — parsed (not executed) at save time by a natural-language cron
parser into a `croner` cron string. The schedule is **authoring sugar that compiles to a managed flow**:
on save, the host builds/updates/deletes a hidden-but-visible `cron → rule` flow that matches the
directive, and the **existing flow cron reactor** fires it. The rule is self-describing (read line 1,
know when it runs), the user never touches the canvas — and there is still **exactly one scheduler**
(the flow reactor), never a second one on rules.

## Goals

- A **`#[schedule(...)]` directive** at the top of a rule body: `#[schedule("every 15 minutes")]`
  (natural language) or `#[schedule(cron = "*/15 * * * *")]` (explicit). Parsed at save, stored as
  structured schedule metadata on the `rule` record. **Never executed** — it's a declaration.
- **Natural-language → cron** via a small parser (`natural-cron-rs` candidate), compiling the phrase to
  a 5-field cron string that `croner` then owns (validation + next-runs). The parser is a *text
  compiler*, not a time engine.
- A **schedule syncer** on `rules.save`: derive the managed `cron → rule` flow from the directive and
  reconcile it — create if new, update `config.cron` if changed, **delete** the managed flow if the
  directive is removed. Idempotent (same directive on re-save = no-op).
- **One scheduler preserved.** The managed flow's existing cron trigger + `react_cron` fire the run.
  No rule-cron reactor, no schedule-fires-anything-itself. The directive only decides the flow's
  `config.cron`.
- The **rule page shows the parsed schedule** — the resolved cron, the next 5 runs, and a link to the
  managed flow (visible, marked "from rule schedule") as a power-user escape hatch.
- `insight.raise` **stays in the rule body** (the in-cage handle). The schedule does not add or manage
  an insight step — the rule already raises what it raises.

## Non-goals

- **No second scheduler.** The directive is compiled to a flow; the *flow* cron reactor fires it. A
  rule-cron reactor scanning rule directives at runtime is explicitly rejected — it is the exact "three
  schedulers" trap `rules-workflow-convergence-scope.md` deleted. The directive is read at **save**,
  never on a firing tick.
- **No schedule as a runtime construct.** `#[schedule]` is metadata parsed once at save; it does not run
  in the cage, has no cage handle, and cannot be set from rule execution.
- **No insight machinery here.** Whether/what the rule raises is the rule body's job (in-cage
  `insight.raise`); this scope schedules the *run*, nothing more. A schedule on a rule that raises
  nothing simply runs and raises nothing.
- **No natural-cron *time engine* in core.** The NL parser only emits a cron **string**; `croner` (the
  sanctioned engine) owns parsing/validation/next-runs. The parser never computes a fire time.
- **No second time dialect.** 5-field Vixie cron as `croner` parses it. NL phrases the parser can't map
  cleanly are a save error with a helpful message + the raw-cron escape hatch — not a new grammar.
- **No page-field or code-free authoring door in v1.** The directive-in-body is the one door
  (user decision). A rule-page schedule *field* that writes the directive is a named follow-up, not v1.
- **No flow-authoring UX change.** The managed flow uses the shipped canvas/reactor as-is; this scope
  builds it programmatically, it does not change how flows are authored.

## Intent / approach

**The directive and the scheduler are separable — that's the whole design.** The appeal of "put the
schedule at the top of the code" is a *self-describing rule* and *no canvas*. The danger is that "the
rule schedules itself" implies a runtime that scans rules and fires them — a second scheduler. We keep
the appeal and drop the danger by making the directive **compile to a managed flow** rather than drive a
new runtime:

1. **Extract at save, don't execute.** `rules.save` already states "the body is NOT executed at save."
   The syncer extracts the `#[schedule(...)]` directive from the body text (a cheap top-of-file scan,
   before the cage ever runs), parses it, and stores structured `schedule` metadata on the `rule`
   record. The cage is untouched — the directive is a comment-like annotation to the engine, invisible
   at run time.
2. **Compile NL → cron once.** `natural-cron-rs` (candidate) turns `"every 15 minutes"` into
   `"*/15 * * * *"`; `croner::Cron::is_valid` validates it; `next_after` (the shipped reminders helper)
   computes the next-5-runs preview. The explicit form `#[schedule(cron = "...")]` skips the NL step.
3. **Reconcile a managed flow.** The syncer builds the derived flow `flow:{ws}:schedule:{rule_id}` — a
   two-node `cron trigger (config.cron = <compiled>) → rule node (config.rule = <rule_id>)`, enabled,
   `start_on_boot = true`, marked `managed_by = "rule-schedule:{rule_id}"`. On re-save it diffs the
   desired vs. stored flow and issues the minimal `flows.save`/`flows.node.update`/`flows.delete` to
   converge. Directive removed → managed flow deleted. **The managed flow is derived state; the rule
   directive is the source of truth.**
4. **The flow reactor fires it.** From here it is an ordinary enabled cron flow — `react_cron` +
   `croner` do exactly what they already do. Nothing new fires anything.

**Alternative rejected — a rule-cron reactor (schedule lives on the rule, a reactor scans + fires
rules).** This is the naive reading of "run this rule on a schedule," and it is rejected outright: it
reintroduces a second scheduler beside the flow cron reactor — the "three schedulers, three run models"
the convergence scope deleted. Compiling to a managed flow reuses the one scheduler and inherits its
durability (per-node cursor, fire-once, restart-safe) for free.

**Alternative rejected — schedule as a first-class `rule` record field written by an MCP verb, no
directive.** Considered (and it's the named v1-alternative from the UX discussion): a `schedule` field on
the rule set by `rules.schedule.set`, driving the same managed flow. Rejected *as the v1 door* only
because the user chose the **directive-in-body** so the rule is self-describing in one place. The syncer
machinery is identical either way — so a page-field/verb door is a **cheap additive follow-up** (it
writes the same metadata the directive produces), not a redesign. Recorded as the deferred second door.

**Alternative rejected — natural-cron as the firing engine.** Using the NL parser's own scheduling
(if it exposes one) instead of compiling to `croner`. Rejected: it would fork the time authority and
duplicate `croner`. The parser's *only* job is `phrase → cron string`; everything downstream is the
shipped cron path. (License note: `natural-cron-rs` must be MIT/Apache to enter `rust/crates/*`; verify
before adoption — else vendor a thin phrase-matcher or route the phrase through `ai.*`, per
`flow-trigger-schedule-authoring-scope.md`'s deferred-NL posture.)

## How it fits the core

- **Tenancy / isolation:** the `schedule` metadata is a field on the `ws`-scoped `rule:{ws}:{id}` record;
  the managed flow is `flow:{ws}:schedule:{rule_id}` in the same workspace namespace. The syncer runs
  under the saving caller's principal and writes only ws-scoped records. A ws-B save can neither read a
  ws-A rule nor build a ws-A managed flow (mandatory isolation test). The managed flow's runs are
  ws-walled like any flow run.
- **Capabilities:** the directive is compiled during `rules.save` (gated `mcp:rules.save:call`, held).
  The syncer's flow writes go through the **existing** `flows.save`/`flows.node.update`/`flows.delete`
  verbs under the **same caller** — so scheduling a rule requires the caller to hold **both**
  `mcp:rules.save:call` **and** the flow-write grant. A caller with rule-write but not flow-write gets a
  **clear deny** at the sync step (the save persists the directive as metadata but reports the managed
  flow could not be built — no silent partial). No new capability is introduced; scheduling is the
  *intersection* of the two existing grants (no widening — a rule cannot gain flow-authoring authority
  its caller lacks). Deny test per path.
- **Symmetric nodes:** the syncer is placement-agnostic pure logic (runs wherever `rules.save` runs); the
  managed flow's `placement` defaults to the rule's node role. No `if cloud`. The firing is the existing
  symmetric reactor.
- **One datastore:** the `schedule` metadata is a field on the existing `rule` record; the managed flow
  is an existing `Flow` record with an additive `managed_by` marker field. No new table, no new store.
- **MCP surface (API shape — judged):**
  - **CRUD:** **no new schedule verbs in v1.** The schedule is created/updated/deleted **as a side
    effect of `rules.save`** (directive present/changed/removed) — the one write door the user chose.
    The managed flow's CRUD is the existing `flows.*`. *(The deferred second door —
    `rules.schedule.set/clear` writing the same metadata — is the named follow-up if a code-free path is
    wanted; each would be its own verb+cap.)*
  - **Get / list:** `rules.get` gains a **`schedule` block** in its response (`{ raw, cron, next_runs,
    flow_id, managed, drift? }`) so the rule page renders the schedule + preview from one read. A
    `rules.list` filter `scheduled: true` surfaces "what runs on a timer" (the roll-up). No separate
    schedule-get verb — the schedule is part of the rule.
  - **Live feed:** N/A for the schedule itself (it's metadata). The *firing* streams over the existing
    `flows.watch` run feed on the managed flow; the rule page can deep-link to it.
  - **Batch:** N/A — one directive per rule, compiled on that rule's save.
- **Data (SurrealDB):** `schedule` object on `rule:{ws}:{id}` (`{ raw, cron }` — the compiled result,
  so reads don't re-parse); the managed `flow:{ws}:schedule:{rule_id}` carries `managed_by`. State only.
- **Bus (Zenoh):** none from scheduling. Firing is the existing cron-reactor → job → run motion,
  unchanged. State vs motion held (directive = state; the fire = the flow's motion).
- **Sync / authority:** the `rule` record is authoritative on its hosting node; the managed flow is
  derived state reconciled on every save (source of truth = the directive), so a restart re-derives
  nothing lost — the flow already persists and `start_on_boot` re-arms it. Offline: save is a local
  write; the reconcile is local.
- **Secrets:** none.
- **SDK/WIT impact:** none — the directive is rule-body text parsed host-side; no cage handle, no ABI
  change. The `managed_by` flow field is additive record metadata.

## Example flow

A facilities analyst schedules `cooler-foodsafety` from the Rules page — no canvas.

1. They open the rule and add one line at the top, then **Save**:
   ```rhai
   #[schedule("every 15 minutes")]

   let hot = source("cooler.temp").last("15m").rollup("5m","max").filter("max > 5.0");
   if hot.size() > 0 {
       insight.raise(#{ dedup_key: "cooler:"+equip, severity: "critical",
                        title: "cooler over temp", tags: #{ siteRef: site, kind: "overtemp" } });
   }
   ```
2. `rules.save` persists the rule (body not executed), then the **syncer** extracts the directive →
   `natural-cron` compiles `"every 15 minutes"` → `"*/15 * * * *"` → `croner` validates → stores
   `schedule: { raw: "every 15 minutes", cron: "*/15 * * * *" }` on the rule.
3. The syncer **builds the managed flow** `flow:acme:schedule:cooler-foodsafety` —
   `cron trigger (config.cron="*/15 * * * *") → rule node (config.rule="cooler-foodsafety")`, enabled,
   `start_on_boot`, `managed_by="rule-schedule:cooler-foodsafety"` — via `flows.save`.
4. The **rule page** now shows: `● scheduled · every 15 minutes · next: 14:15 · 14:30 · 14:45 …` with
   **Pause / Edit / Unschedule** and a small **open as flow →** link (the managed flow, marked "from
   rule schedule").
5. The shipped `react_cron` reactor fires the managed flow every 15 min → the rule runs →
   `insight.raise` dedups on `(ws, "cooler:"+equip)`, bumping `count` on repeat hits. **No canvas was
   opened; one scheduler ran it.**
6. The analyst changes the line to `#[schedule("every hour")]` and saves → the syncer diffs and issues
   one `flows.node.update` setting `config.cron="0 * * * *"`. Deletes the line and saves → the managed
   flow is deleted; the rule reverts to run-on-demand.
7. **Deny path:** an analyst with rule-write but not flow-write saves a directive → the rule + its
   `schedule` metadata persist, but the response reports the managed flow could not be built (flow-write
   denied) — no silent half-state; the schedule shows `pending: needs flow-write`.
8. **Drift:** a power user opens the managed flow and hand-edits its cron. On the next `rules.save`,
   `rules.get`'s `schedule.drift` flags "managed flow diverged"; the syncer **re-asserts** the
   directive's value (source of truth is the rule) — documented, not silent.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), real infra — real store, real `rules.save`/
`flows.*` verbs against a real spawned node, seeded rules, no fakes (rule 9). The **only** sanctioned
fake is the model provider behind `ai.*` (a true external), not needed here unless the deferred NL-via-AI
door is exercised.

- **Capability-deny (mandatory):** `rules.save` denied without `mcp:rules.save:call`; a directive save
  by a caller **with** rule-write but **without** flow-write → the schedule metadata persists but the
  managed-flow build is **denied** and reported (no silent partial, no widening — scheduling is
  `rule-write ∩ flow-write`). Opaque deny on each underlying verb.
- **Workspace-isolation (mandatory):** a ws-B save cannot build/read a ws-A managed flow; `rules.get`'s
  schedule block on a ws-A rule is invisible to ws-B; the managed flow's runs are ws-walled. Real store.
- **Directive parse (unit):** `"every 15 minutes"` → `"*/15 * * * *"`, `"weekdays at 08:00"` →
  `"0 8 * * 1-5"`, `#[schedule(cron="0 2 * * *")]` passes through; an unparseable phrase → a save error
  with a helpful message (not a silent no-schedule); the compiled cron is `croner`-valid.
- **Preview parity:** the rule-page next-runs and the reactor's actual firing agree — assert the same
  `(cron, now) → next-5` vector fixture used by `flow-trigger-schedule-authoring-scope.md` (shared
  guard; client preview never lies about what `react_cron` fires).
- **Sync reconcile (the core behavior):** save-with-directive builds the managed flow (assert its two
  nodes + `config.cron` + `managed_by`); re-save unchanged = no-op (same version, no extra write);
  change directive = one `flows.node.update` to the new cron; remove directive = managed flow deleted;
  the rule reverts to on-demand.
- **Firing end-to-end (real reactor):** enable + advance the injected clock 15 min → exactly one run of
  the managed flow fires → the rule runs → an insight is raised (assert the `insight` record + dedup on
  a second tick). Reuses the shipped `react_cron` test harness.
- **Drift:** hand-edit the managed flow's cron → `rules.get` reports `schedule.drift = true`; next save
  re-asserts the directive value.
- **Idempotent restart:** managed flow survives a node restart via `start_on_boot` (it's a record);
  no duplicate flow is built on the next save.
- **Frontend (Vitest + gateway):** the rule page renders the schedule block + next-5 preview from a real
  `rules.get`; a `*.gateway.test.tsx` saves a directive against a spawned node and asserts the managed
  flow exists with the right cron; the `scheduled:true` list filter returns exactly the scheduled rules.

## Risks & hard problems

- **The "second scheduler" temptation.** The single biggest risk is an implementer reading "schedule on
  the rule" and building a rule-cron reactor. The scope forbids it: the directive is compiled to a
  **managed flow** at **save time**, and the **flow** reactor fires. Any code that scans rule directives
  on a tick is the bug. (A workspace-wide grep for a rule-schedule reactor is the ship gate.)
- **NL parser fidelity + license.** `natural-cron-rs` must (a) be MIT/Apache to enter core, and (b) map
  the common phrases correctly. Mitigation: it only emits a cron **string** (validated by `croner`,
  previewed as next-5-runs), and the explicit `cron="..."` form + the visible preview are the escape
  hatch when NL guesses wrong. If license/fidelity fail, vendor a thin phrase-matcher or route via
  `ai.*` (the deferred-NL posture from the sibling scope) — the seam is `phrase → cron string`, swappable.
- **Managed-flow drift & ownership.** A user editing the managed flow directly creates two truths. The
  rule directive is declared source of truth; the syncer re-asserts on save and `rules.get` surfaces
  `drift`. The open question is whether to *lock* the managed flow from manual edits or allow-and-flag
  (recommend allow-and-flag v1 — you said the flow can be visible).
- **Partial failure on save.** Rule-write succeeds, flow-write denied → the schedule is "pending." The
  contract must be explicit (persist metadata, report the flow gap, show `pending` on the page) so the
  user isn't told "scheduled" when nothing fires. Named in the deny test.
- **Directive extraction robustness.** Parsing `#[schedule(...)]` from body text (before the cage runs)
  must not misfire on the string appearing inside rule logic/comments. Anchor it to a top-of-file
  annotation position with a strict grammar; a malformed directive is a save error, not a silent skip.

## Open questions — RESOLVED (build 2026-07-21; see `sessions/rules/scheduled-rules-session.md`)

1. **Directive syntax.** — **RESOLVED: attribute-style `#[schedule(...)]`.** A strict top-of-body scan
   (a line whose first non-space chars are `#[schedule`), so the token inside rule logic is never
   mistaken for it. The directive line is **stripped before the cage compiles the body** (`#` is
   reserved in rhai — this was a run-time bug caught by the firing test, see the debug entry).
2. **NL parser choice.** — **RESOLVED: vendored thin phrase-matcher, NOT `natural-cron`.** Verdict:
   `natural-cron` is MIT (license OK) but a `0.0.2`, ~17%-documented crate whose API is a cron
   *builder/validator*, not a `phrase → cron` NL parser — too immature for a core crate (rule 1). We
   vendored `lb_rules::schedule::compile_phrase` for the common phrases; the explicit `cron="..."` form
   is the escape hatch and `croner` (in-tree, MIT) stays the ONE time engine. The seam is
   `phrase → cron string`, swappable to `ai.*` later.
3. **Managed-flow edit policy.** — **RESOLVED: allow-and-flag drift.** `rules.get` reports
   `drift:true` when the managed flow's cron diverges; the next save re-asserts the directive value.
4. **The deferred second door.** — **RESOLVED: deferred.** The directive is the v1 door; a
   `rules.schedule.set/clear` verb (or rule-page field writing the same metadata) is the named
   follow-up.
5. **Timezone.** — **RESOLVED: UTC v1, documented.** Per-directive tz is a follow-up (it touches the
   reactor clock, not just parsing).

**Shipped (backend + tests):** slices 1–4 (directive compile, the managed-flow syncer, the
`rules.get`/`rules.list` read surface, end-to-end firing on the real `react_cron` reactor). **Deferred:**
the frontend rule-page schedule block (the backend contract is complete + tested; the React surface
lives in a product host, and must assert the shared `(cron,now)→next-5` fixture for preview parity).

## Related

- `scope/rules/rules-engine-scope.md` (the saved rule + `rules.save`/`rules.get` this extends),
  `long-running-rules-scope.md` (job-backed runs — a scheduled long rule composes: the managed flow's
  rule node can run async).
- `scope/flows/flow-trigger-schedule-authoring-scope.md` — **the sibling**: the friendly cron builder +
  the shared next-runs preview fixture; the deferred-NL-via-`ai.*` posture this scope inherits. That
  scope makes the *managed flow's* trigger authorable directly; this scope generates it from a rule.
- `scope/flows/rules-workflow-convergence-scope.md` — **the "one scheduler" line** this scope holds
  (why the directive compiles to a flow instead of a rule-cron reactor); the `rule` node + `rules.eval`
  the managed flow uses.
- `scope/flows/triggers-lifecycle-scope.md` (`config.mode="cron"`/`config.cron`, `start_on_boot`, the
  reactor this managed flow rides), `flow-run-scope.md`, `flow-runtime-control-scope.md`
  (`flows.node.update` the syncer uses).
- `scope/insights/insights-scope.md` (the in-cage `insight.raise` the example rule uses — unchanged by
  scheduling).
- `crates/reminders/src/next_after.rs` (`croner` parse + `next_after` — validation + next-runs the
  preview reuses); `key-stack.md` `croner` row; candidate `natural-cron-rs` (license-pending).
- README `§6.2` (state vs motion), `§6.9` (jobs — the run), `§3` rules 1/5/6/10; `public/rules/rules.md`
  (promotion target).
- Skill: this changes a drivable surface (`rules.save` gains schedule side effects; `rules.get` gains a
  schedule block) — the implementing session updates `skills/rules/SKILL.md` with a schedule-a-rule
  walkthrough grounded in a live run (see §6 checklist / self-check below).
