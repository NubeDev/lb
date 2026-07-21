# Flows scope — authoring a flow trigger's schedule (the friendly cron builder)

Status: scope (the ask). Promotes to `public/flows/flows.md` once shipped.

The cron **backend** for flows is fully shipped — `react_cron` scans every `mode:"cron"` trigger
node, fires one run per due instant off a durable per-node cursor, and parses the schedule with
`croner` (5-field Vixie cron). What is missing is the **authoring UX**: there is *no* frontend today
that lets a user set a flow trigger node's `config.cron`. A user who wants "run this every 15 minutes"
has to hand-type `*/15 * * * *` into raw config — and `0 0 */3 * 2#1` is where humans give up. This
scope adds a **friendly schedule builder** — presets → a structured builder → an always-visible
"next N runs" preview — that writes `config.cron` on a flow trigger node. The stored value stays a
cron string (the reactor is untouched); we only add better front doors. Natural-language authoring is
deliberately deferred to a thin `ai.*` door, not a new core crate.

## Goals

- A **schedule config form** for the flow `trigger` node (kind `NodeKind::Trigger`, `config.mode`)
  that authors `config.cron` — the one thing the shipped reactor already consumes.
- **Presets** covering the ~80% ("Every 15 minutes", "Hourly", "Daily at 02:00", "Weekdays at 08:00",
  "1st of month at …") as one-click choices that expand to a cron string.
- A **structured builder** for the rest ("every N minutes/hours", "on [days] at [HH:MM]") that compiles
  to cron without the user seeing cron syntax — Node-RED's inject-node ergonomics.
- An **always-visible "next N runs" preview** (default 5) computed from the current cron string, so the
  author *sees* the schedule is right before saving. This is the trust primitive; it is non-optional.
- A **raw cron escape hatch** with live validation for power users — never removed.
- **Zero backend change.** No new verb, no new table, no reactor change; the schedule remains
  `config.cron` written through the existing `flows.node.update` / `flows.save` path.

## Non-goals

- **No natural-language cron parsing in a core crate.** The two candidate crates
  (`natural-cron-rs`, `expressive-cron`) are noted and **deferred**: NL authoring, if wanted, becomes a
  thin front door that routes the phrase through the existing **AI-gateway** (`ai.*`) → a cron string →
  validated by `croner` → shown as "next N runs". No new dependency in core (rule 1). Out of scope here.
- **No second scheduler.** The schedule belongs to the **trigger node**, never to a rule (a schedule
  field on a saved rule would spawn a rule-cron reactor beside the flow-cron reactor — precisely the
  "three schedulers" trap `rules-workflow-convergence-scope.md` deleted). "Run a rule every 15 min" is
  authored as `cron trigger → rule node`, not a scheduled rule.
- **No new cron dialect.** 5-field Vixie cron as `croner` already parses it. No seconds field, no
  `@yearly` macros in v1 (a preset covers the intent).
- **No calendar/weekly-block scheduler.** The `ce-wiresheet` `SchedulePanel` (weekly calendar +
  exceptions) belongs to the component engine; a flow trigger is a cron instant, not a weekly-occupancy
  schedule. Not conflated here.
- **No interval-node UX beyond a note.** The `flipflop` interval node authors `config.period_secs`, a
  different builtin; a matching "every N seconds" affordance is a small sibling follow-up, flagged not
  built.
- **No timezone picker in v1.** The reactor computes on the node's logical clock (UTC); a per-trigger tz
  is an open question, not v1.

## Intent / approach

**Reuse a pattern that already works, retarget the write.** Two prior cron-authoring surfaces already
exist in-repo, and neither authors a flow trigger — the work is to retarget one, not invent a picker:

- **`react-js-cron` via `CronBuilder.tsx`** — the reminders slice's *chosen* visual cron authoring
  component (`key-stack.md` row "Reminders cron builder (UI)"): a lossless 5-field cron read/write
  widget, antd-based but wrapped in **one** component and restyled to the shell's Tailwind/shadcn tokens
  via a locally-scoped antd `ConfigProvider` (antd is *not* pulled into the global theme). This is the
  platform's **sanctioned** cron-builder dependency and the recommended base — reuse `CronBuilder.tsx`
  and point its commit at the flow node's `config.cron`.
- **`packages/ce-wiresheet/src/ui/CronPanel.tsx`** — the component-engine cron widget (6 presets,
  validation, a **"next 5 runs" preview** via `cronNextRuns()`, tested in `cron.test.ts`). It binds to a
  `ce-wiresheet` component over the wiresheet transport, not a flow trigger. Its value here is the
  **preset table + the next-runs preview helper**, which `react-js-cron` does not itself provide.

**Recommendation: `react-js-cron`/`CronBuilder.tsx` for the builder + presets, plus the CronPanel
`cronNextRuns()` helper for the next-runs preview.** That keeps one sanctioned cron dependency (no new
one), reuses the shell-restyled component, and adds the trust primitive (next-runs) the reminders widget
lacks. The cron string, the presets, and the next-runs math are all already written; only the **commit
target** changes — from a reminder record / wiresheet component to the flow trigger node's `config.cron`.

This keeps the change **additive and rule-1 clean**: the schedule stored on the node is still a plain
cron string, the reactor (`react_cron` + `croner`) is untouched, and the whole feature is a frontend
config form plus (optionally) one tiny read-only preview helper. The backend already treats the schedule
as opaque data.

**Where the form lives.** The flows canvas already renders schema-driven config forms from the node
descriptor (`flows-canvas-scope.md`). The `trigger` descriptor in `builtins/core.rs` declares
`config.mode` + `config.cron`; this scope adds a **descriptor-declared custom editor** (or a
descriptor `format: "cron"` hint the canvas maps to the builder component) so the generic form renders
the friendly builder for the `cron` field instead of a bare text input. That keeps the descriptor the
single contract (`node-descriptor-scope.md`) and avoids the canvas branching on a node id (rule 10).

**Preview math — where does "next N runs" run?** Two options, and we pick **client-side** for v1: a
small TS cron-next helper (the `cronNextRuns()` the CronPanel already ships) computes the preview with no
round-trip, so the author sees it update as they build. It must agree with `croner`'s semantics — so the
frontend helper is validated by a **golden test** that pins a set of `(cron, now) → next-5` vectors
identical to what `croner`'s `next_after` produces on the backend (a real cross-check, not a mock). If
the two ever diverge, that's a bug, caught by the shared vector fixture. A backend `flows.cron.preview`
verb is the rejected alternative (below).

**Alternative rejected — a backend `flows.cron.preview` verb.** Considered: a host verb that takes a cron
string and returns the next N instants from `croner` (authoritative). Rejected for v1: it adds a verb +
cap + a round-trip on every keystroke for a preview that must feel instant, and the client helper already
exists and is testable against the same crate's semantics via shared vectors. If a future dialect makes
client/backend parity hard to maintain, promote the preview to a verb then — the shared-vector test is
the tripwire that tells us when.

**Alternative rejected — NL cron via a core crate now.** `natural-cron-rs` / `expressive-cron` both do
NL→cron, the right *idea*. Rejected as a core dependency: (a) core stays lean and symmetric (rule 1);
(b) presets + the structured builder already cover the overwhelming majority with zero deps; (c) when NL
is genuinely wanted, the platform already has an `ai.*` seam — "turn this phrase into a 5-field cron,
then I validate it with croner and show you the next 5 runs" — which reuses infrastructure instead of
vendoring a parser. So NL is a *deferred front door over `ai.*`*, not a v1 crate.

## How it fits the core

- **Tenancy / isolation:** the schedule is a field on a flow node inside a `ws`-scoped `Flow` record; it
  is written through the existing `flows.node.update` / `flows.save` verbs, which are already
  workspace-walled. This feature adds **no new data path**, so isolation is inherited: a ws-B user
  editing a schedule can only reach ws-B flows through the same gated verb. The mandatory isolation test
  rides the existing flow-save path (a ws-B save cannot target a ws-A flow's node).
- **Capabilities:** no new capability. Authoring a schedule is `flows.node.update` / `flows.save` under
  the caller's existing flow-write grant (`mcp:flows.save:call` / `mcp:flows.node.update:call`); a
  caller lacking it is denied at that verb, unchanged. The friendly builder is UI over that same gated
  write — it cannot widen authority. Deny test: rides the existing flow-write deny test (no new verb to
  add one for).
- **Symmetric nodes:** N/A to the backend (untouched). The frontend builder is placement-agnostic UI. No
  `if cloud`.
- **One datastore:** none added. The schedule is a string field on the existing `Flow` record in
  SurrealDB. No new table, no new persistence.
- **MCP surface (API shape — judged):**
  - **CRUD:** **none new.** The schedule is written by the *existing* `flows.save` / `flows.node.update`
    verbs (a node config edit is exactly `flows.node.update` per `flow-runtime-control-scope.md`). This
    scope reuses them; it introduces no write verb.
  - **Get / list:** **none new.** The schedule reads back as part of the node config on `flows.get`.
  - **Live feed:** N/A — authoring is an edit, not a stream. (The *firing* of the schedule is the
    existing `flows.watch` run feed; unchanged.)
  - **Batch:** N/A — a schedule edit is a single bounded node write.
  - The one *possible* new read verb — `flows.cron.preview` — is **rejected for v1** (client-side
    helper instead; see Intent). If promoted later it is a pure, bounded read (`{cron, n} → {next[]}`),
    gated `mcp:flows.cron.preview:call`.
- **Data (SurrealDB):** `config.cron` on the trigger node of the `Flow` record — state, already there.
  No motion. State vs motion held: authoring writes state; the reactor turns the schedule into motion
  (a run) elsewhere, unchanged.
- **Bus (Zenoh):** none from authoring. (The schedule *fires* runs via the existing cron reactor →
  job → run; not part of this scope.)
- **Sync / authority:** the `Flow` record is authoritative on its hosting node like any record; a
  schedule edit is an ordinary workspace write. Offline: the builder is client-side; a save follows the
  normal flow-save durability. The next-runs preview is a pure client computation, no authority needed.
- **Secrets:** none.
- **SDK/WIT impact:** none. This is a frontend config form + descriptor hint + a preview helper; it does
  not touch the wasm/native ABI. The descriptor `format:"cron"` hint (if used) is additive metadata on
  an existing descriptor field, not an ABI change.

## Example flow

A facilities analyst schedules the `cooler-foodsafety` rule to run every 15 minutes.

1. On the flows canvas they drop a **`trigger`** node and a **`rule`** node (`config.rule =
   "cooler-foodsafety"`), wiring `trigger → rule`. The rule raises an insight in-cage via
   `insight.raise(#{…})`.
2. They open the trigger node's config. The form shows **mode = cron** and, for the `cron` field, the
   **friendly builder** (not a raw text box): a preset dropdown, a structured "every N …" row, and a raw
   cron escape hatch — with a live **"Next 5 runs"** panel below.
3. They pick the preset **"Every 15 minutes"** → the builder sets `config.cron = "*/15 * * * *"` and the
   preview immediately lists the next five instants. (Or they type "every 15 minutes" in the raw box and
   the same preview confirms it; NL parsing is not needed for this case.)
4. They **Save** → the canvas commits via the existing `flows.node.update` (or `flows.save`) under their
   flow-write grant. The stored `config.cron` is a plain string; nothing else changed.
5. They **Enable** the flow (`flows.enable`, unchanged). The shipped `react_cron` reactor picks up the
   `mode:"cron"` node, parses `*/15 * * * *` with `croner`, and fires one run every 15 minutes — the
   rule evaluates and raises/dedups the insight. The author never wrote cron syntax and *saw* the
   schedule before enabling it.
6. **Deny path:** a read-only member without the flow-write grant opens the same builder but the Save is
   denied at `flows.node.update` (opaque) — the UI is over the same gated write, no widening.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), on real infra — the schedule write goes through
the **real** `flows.save`/`flows.node.update` verbs against a real spawned node, no fakes (rule 9):

- **Capability-deny (mandatory):** a caller without the flow-write grant that opens the builder and tries
  to save the schedule is **denied at `flows.node.update`/`flows.save`** (opaque) — reuses the existing
  flow-write deny test; the friendly builder adds no bypass.
- **Workspace-isolation (mandatory):** a ws-B principal cannot write a schedule onto a ws-A flow's
  trigger node — the edit rides the ws-walled flow-save path; a ws-B save targeting a ws-A flow id is
  rejected. Verified against the real store.
- **Preview parity (the load-bearing test):** a **shared vector fixture** of `(cron, now) → next-5`
  cases is asserted **twice** — once by the frontend `cronNextRuns()` helper (`pnpm test`) and once by
  the backend `croner` `next_after` path — and the two must produce byte-identical instants. This is the
  guard that the client preview never lies about what the reactor will do. Include DST-adjacent and
  end-of-month cases (`*/15`, `0 2 * * *`, `0 8 * * 1-5`, `0 0 1 * *`).
- **Preset correctness (unit):** each preset expands to the exact cron string it claims, and that string
  is `croner`-valid (round-trip: preset → cron → validate → next-runs non-empty).
- **Structured builder → cron (unit):** "every N minutes/hours", "on [days] at HH:MM" compile to the
  expected cron; invalid combinations (e.g. minute > 59) are rejected before commit.
- **Raw cron validation (unit):** an invalid raw string surfaces the validation error and blocks save;
  a valid one shows next-runs. Mirrors `croner::Cron::is_valid` semantics via the shared fixture.
- **Integration (real gateway/UI):** a canvas `*.gateway.test.tsx` builds a cron trigger with a preset
  against a real spawned node, saves via the real verb, reads the flow back, and asserts `config.cron`
  is the expected string — then (optionally) that `react_cron` fires it on the injected clock (the
  react_cron test already proves firing; this test proves the *authored value reaches the reactor*).
- **Frontend (Vitest):** the trigger node's config form renders the friendly builder for the `cron`
  field (not a bare input); presets populate; the next-runs panel updates on change.

## Risks & hard problems

- **Client/backend cron parity.** The single real risk. The preview must match what `croner` actually
  fires, or the builder becomes a trust bug ("it said 2am, it ran at 3am"). Mitigation is the **shared
  vector fixture** asserted on both sides — not two independent implementations hoping to agree. DST and
  end-of-month are the classic divergence points; they are named test cases.
- **Descriptor-driven custom editor seam.** The canvas renders forms generically from the descriptor
  (rule 10 — no branching on node id). Rendering a *custom* builder for one field means the descriptor
  must carry a `format:"cron"`-style hint the canvas maps to a component by data, not by node id. Getting
  that seam right (a format registry, not a special case) is the design care point; it also pays off for
  future custom editors.
- **Preset scope creep.** Six presets is the CronPanel precedent and covers most needs; the temptation is
  a preset for every shape. Hold the line — the structured builder + raw box are the tail; presets are
  the head.
- **NL pressure.** Users will ask for "just let me type it in English." The answer is the deferred
  `ai.*` door, *with* the next-runs preview as the confirmation — not a vendored parser in core. The
  preview is what makes even an AI-produced cron safe to trust.

## Open questions

1. **Descriptor hint vs. canvas-side field map.** Does the friendly builder attach via a new descriptor
   field hint (`format:"cron"` on the `cron` config field) that the generic form maps to a component, or
   a canvas-side registry keyed by `(node kind, field name)`? **Recommend the descriptor hint** — it
   keeps the contract in the descriptor (`node-descriptor-scope.md`) and stays rule-10 clean (the canvas
   maps a *format*, never a node id). Resolve in build.
2. **Where the next-runs helper lives.** Reuse `ce-wiresheet`'s `cronNextRuns()` by extracting it to a
   shared package, or copy the small helper into the flows canvas package? **Recommend extract-to-shared**
   only if a clean home exists (a `packages/*` cron util); otherwise copy the ~30-line helper and share
   the *test vector* fixture (the parity guard is the fixture, not the code). Decide against the actual
   package layout at build time.
3. **Timezone.** v1 computes on the node's UTC logical clock (matching the reactor). Is a per-trigger
   timezone needed before a production fleet, or is "schedules are UTC, documented" acceptable for v1?
   **Recommend UTC-only v1**, tz as a named follow-up (it touches the reactor's clock, not just the UI —
   a bigger change than authoring).
3. **Interval (`flipflop`) parity.** Should the same slice add an "every N seconds" affordance for the
   `flipflop` node's `config.period_secs`, or is that a separate sibling? **Recommend separate sibling**
   — different field, different builtin, smaller; don't couple it to the cron builder.
4. **NL door timing.** Ship the `ai.*` NL front door in a fast-follow, or wait for explicit user demand?
   **Recommend wait** — presets + builder cover the head; add NL when a real user hits the tail and asks.

## Related

- `flows/react_cron.rs`, `reactor_loop.rs` (the shipped cron reactor this authors for);
  `reminders/src/next_after.rs` (`croner` parse + `next_after` — the backend semantics the preview must
  match).
- `scope/flows/flows-canvas-scope.md` (the schema-driven config form the builder plugs into),
  `node-descriptor-scope.md` (the descriptor contract + the `format` hint), `triggers-lifecycle-scope.md`
  (the `manual|cron|event|inject|boot` trigger kinds; `config.mode`/`config.cron`),
  `flow-runtime-control-scope.md` (`flows.node.update` — the per-node config write this reuses).
- `scope/flows/rules-workflow-convergence-scope.md` (the `rule`/`rhai` nodes + `rules.eval` the example
  flow uses; the "one scheduler" line — why the schedule lives on the trigger, not on a rule).
- `scope/rules/scheduled-rules-scope.md` — **the sibling from the other direction**: a
  `#[schedule(...)]` directive on a *rule* that compiles to (and manages) exactly the kind of `cron →
  rule` flow this scope makes hand-authorable. Both share the next-runs preview fixture and the
  deferred-NL-via-`ai.*` posture; one generates the managed flow, the other authors a trigger directly.
- `scope/insights/insights-scope.md` (the `insight.raise` in-cage handle the example rule uses).
- `key-stack.md` "Reminders cron builder (UI)" row — **`react-js-cron` / `CronBuilder.tsx`**, the
  platform's sanctioned shell-restyled cron widget this scope reuses as the builder base.
- `packages/ce-wiresheet/src/ui/CronPanel.tsx` + `cron.test.ts` — the component-engine cron widget whose
  **preset table + `cronNextRuns()` next-5-runs preview** this scope borrows; note it drives the
  *component engine*, not flow triggers.
- Deferred NL crates (not adopted): `natural-cron-rs`, `expressive-cron` — the NL→cron idea, deferred to
  an `ai.*` front door per this scope's non-goals.
- README `§6.2` (state vs motion), `§3` rules 1/5/6/10; `docs/key-stack.md` (the `croner` row).
- `public/flows/flows.md` (promotion target).
