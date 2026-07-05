# Agent-personas — persona-session #5 (roster + context focus + per-tab pin) (session)

- Date: 2026-07-05
- Scope: ../../scope/agent-personas/persona-session-scope.md
- Stage: S8 (data plane) — post-ship correction of #1's selection model
- Status: done

## Goal

Replace #1's single mutable `agent.config.active_persona` workspace-wide toggle with the three ideas
the scope names: a workspace **roster** (`enabled_personas`), exactly **one** persona per run
**suggested client-side** from the page surface (`Persona.surfaces` + a sticky per-tab **pin**) with
defaults re-homed to a nullable `Prefs.agent_persona` axis (member → ws-default fold). Zero new MCP
verbs; run assembly (`apply.rs`/`resolve_effective`) untouched. The fix is for the dual bug #1
shipped with: two members (or one member's two tabs) couldn't hold different focuses, and hand-picking
a focus workspace-wide is backwards when the dock already knows where the user is.

## What changed

**Backend (already shipped green by the prior handover — verified, not redone):**

- `Persona.surfaces: Vec<String>` added (model.rs); seeded on every built-in in `personas.toml`
  (taken from the LIVE dock vocabulary in `ui/src/features/routing/surface.ts`).
- `AgentConfig.enabled_personas: Option<Vec<String>>` (None = all enabled); `active_persona` is
  `#[serde(default, skip_serializing)]` (decode-only — column tolerated for old records, never read).
- `AGENT_CONFIG_COLUMNS` projects `enabled_personas` (not `active_persona`); `DEFINE FIELD` added.
- `Prefs.agent_persona: Option<String>` whole-fold nullable axis (NOT in `ResolvedPrefs`).
- `resolve_persona` reworked to the new fold: explicit (roster-checked) → member default → ws default
  → none, first `Some` wins; `is_enabled(roster, id)` helper (None or empty = all enabled).
- `agent.persona.list` returns `PersonaListItem = Persona & { enabled }` (computed server-side).
- One-shot boot migration `migrate_active_persona` (idempotent; admin write wins); wired into
  `node/main.rs` and `role/gateway/src/bin/test_gateway.rs`.

**UI (this session — the work that was NOT STARTED in the prior handover):**

- **The wire** (`ui/src/lib/channel/payload.types.ts`, `features/channel/useChannel.ts`,
  `features/agent-dock/useDockSession.ts`): `AgentPayload.persona?: string`; `encodeAgent` takes an
  optional 5th positional arg; `postAgent` and `useDockSession.ask(goal, persona?)` thread it. The
  agent_invoke IPC + `invokeAgent` API also gained the `persona` arg (`lib/ipc/http.ts`,
  `lib/agent/agent.api.ts`). All additive — existing callers unchanged.
- **Persona API** (`ui/src/lib/agent/agentPersona.api.ts`): `Persona.surfaces: string[]`; new
  `PersonaListItem = Persona & { enabled: boolean }`; `listPersonas()` returns the list-item shape.
- **Config API** (`ui/src/lib/agent/config.api.ts`): `enabled_personas?: string[]` replaces
  `active_persona` (removed entirely — no decode-only ghost in the UI shape).
- **Prefs client** (`ui/src/lib/prefs/set.ts`): `PrefsPatch` extended with `agent_persona?: string`
  (added explicitly via intersection — the axis is on `Prefs` but NOT on `ResolvedPrefs`, so the
  existing `Pick<ResolvedPrefs, …>` doesn't include it; precedent: `ui_theme` IS on `ResolvedPrefs`,
  `agent_persona` deliberately isn't).
- **The dock persona layer** (new files):
  - `features/agent-dock/personaPin.ts` — pure `sessionStorage` helpers, keyed
    `lb.agent-dock.persona-pin.<ws>` (per-tab is the WHOLE point; SSR/unavailable guards mirror
    `useDockChrome`'s `localStorage` guards).
  - `features/agent-dock/usePersonaFocus.ts` — the hook: fetches `listPersonas()` once (tolerates
    deny → no suggestion), reads the LIVE page surface (passed in as a string by `AgentDock`), and
    resolves `{ current, suggestion, options, pin, clearPin }` — pin > first enabled surface-match
    (id-sorted) > null. A pin that left the roster or got disabled silently falls through to the
    suggestion (so the dock never holds a stale id that would 400 on invoke).
  - `features/agent-dock/DockPersonaChip.tsx` — the chip in the dock header. ALWAYS shows what the
    next invoke will send + why ("pinned" / "from this page" / "workspace default"); a switcher over
    enabled personas pins one; "Clear pin" returns to the suggestion. `data-persona-id` +
    `data-focus-reason` attributes for testability (decoupled from human label text).
  - `features/agent-dock/AgentDock.tsx` — wires the chip + threads `personaFocus.current?.id` into
    `DockComposer`'s `ask` (chip and payload can never disagree — the chip's `current?.id` IS what
    `ask` passes).
- **Settings rework** (`features/settings/agent/`):
  - `usePersonaCatalog.ts` — roster (`enabled_personas`) + member default (`getPrefs().agent_persona`)
    + optimistic ws default (no ws-default read verb exists). `toggleEnabled` writes the roster
    (disabling the last enabled one is refused — empty roster = all-enabled by design).
    `setMemberDefault`/`clearMemberDefault` write `prefs.set` (`""` to clear);
    `setWsDefault`/`clearWsDefault` write `prefs.set_default`.
  - `PersonaCatalog.tsx` — completely reworked: roster toggle per row + "Set as my default" (member
    cap) + "Set as workspace default" (admin cap) + clear-default buttons; the "Use" pick + the
    Active highlight are GONE.
  - `PersonaSection.tsx` — `canSetRoster` (CAP.agentConfigSet) + `canSetMemberDefault`
    (CAP.prefsSet) + `canSetWsDefault` (CAP.prefsSetDefault) gate the affordances. Members see the
    roster + ws-default read-only but CAN set their own default.
  - `PersonaEditor.tsx` — added a `surfaces` StringListField (record-only edit, rule 10).

## Decisions & alternatives

- **`PrefsPatch.agent_persona` via intersection, not `Pick`.** `agent_persona` lives on `Prefs`
  (stored) but NOT on `ResolvedPrefs` (the folded result the formatter reads) — the precedent
  (`ui_theme`) is on BOTH, so the existing `Pick<ResolvedPrefs, …>` shape doesn't reach it. Add it
  explicitly as `& { agent_persona?: string }`. Alternative rejected: add it to `ResolvedPrefs` —
  wrong; `format.*` never reads it and a resolved value would lie about "decided" when the chain only
  holds a member/ws axis, not a "decided" default.
- **Empty roster (Some([])) = cleared = all enabled.** MERGE can't write null; an empty list is the
  "back to all" signal (mirrors `#1`'s `filter(|s| !s.is_empty())` guards). **Disabling ALL personas is
  unsupported by design** — `nextRoster` refuses to produce an empty list (keeps the last one
  enabled). Alternative rejected: a sentinel "none" value — adds a state the dock + host both must
  handle, for a use case with no recorded demand.
- **`""` clears a prefs axis (the MERGE-can't-write-null workaround).** The "Clear my default" /
  "Clear ws default" buttons call `setPrefs({ agent_persona: "" })`. The consumer
  (`resolve_persona`) treats empty-string as unset via `filter(|s| !s.is_empty())`. Resolves scope
  open question 4.
- **Multi-match suggestion = id-sorted first.** `usePersonaFocus` does
  `options.find((p) => p.surfaces.includes(surface))` over the roster-as-returned (which the host
  returns id-sorted). NOT seed order. Resolves scope open question 2 — deterministic, no new priority
  field, and the host's existing order is what every reader already sees.
- **Disabled default (vs explicit) folds to none with `warn!`.** Mirrors the dangling-id posture;
  scope was silent here. Alternative rejected: an errored run — would make a single admin roster
  change break every member's headless invokes.
- **`agent.persona.list` computes `enabled` server-side; raw roster still on `agent.config.get`.**
  Resolves scope open question 1 — one fetch for the dock + picker, no client-side roster join.
- **`insights-analyst.surfaces = []`** until an insights page surface exists (committed for the
  future; commented in `personas.toml`).
- **`wsDefaultId` is optimistic only.** There is no ws-default read verb (the host has
  `get_workspace_prefs` internally but doesn't expose a client read). The catalog tracks it
  optimistically after an admin write; a reload forgets it (display nicety — `resolve_persona` is the
  source of truth at run time). Alternative rejected: add a ws-default read verb — out-of-scope new
  surface for a display-only nicety.
- **Dock chip's `current?.id` IS what `ask` passes.** The chip and the wire payload can never
  disagree by construction — there's no second source of truth. One gateway test (`DockPersonaChip`)
  pins this end to end. Alternative rejected: a separate "pending persona" state — exactly the dual
  concept that caused #1's bug.

## Tests

Real gateway, real records, real dispatch — NO mocks, NO fakes (rule 9). Mandatory categories
included.

### Backend (`cargo test`, verified green — 38 persona-suite tests)

```
$ cargo test -p lb-host --test agent_persona_session_test --test agent_persona_test \
                                  --test agent_persona_catalog_test --test agent_config_test
test result: ok. 9 passed  (agent_persona_session_test)
test result: ok. 21 passed (agent_persona_test)
test result: ok. 8 passed  (agent_persona_catalog_test)
test result: ok. 6 passed  (agent_config_test)

$ cargo test -p lb-host --test prefs_mcp_test
test result: ok. 3 passed
```

The new `agent_persona_session_test.rs` covers: precedence table (incl. empty-string-clears),
dangling default vs explicit-unknown, roster None/Some/empty-clears + list flags, explicit-disabled
named error + disabled-default folds to none, ws isolation (incl. ws-A custom id as ws-B default →
none), capability deny (member denied roster + prefs.set_default, allowed prefs.set), independence
(two members + two explicit ids, tokio::join!), migration (copy/no-clobber/idempotent), surfaces-as-
data record round-trip.

### UI unit (`pnpm test`)

```
Test Files  104 passed (104)
     Tests  640 passed (640)
```

Includes new units: `payload.test.ts` (encodeAgent persona + omit-when-undefined byte-identical) and
`personaPin.test.ts` (sessionStorage helpers + the two-tabs-are-independent fake-storage proof).

### UI gateway (`pnpm test:gateway` — real spawned `test_gateway`)

```
✓ src/features/agent-dock/DockPersonaChip.gateway.test.tsx (6 tests)
✓ src/features/settings/PersonaSettings.gateway.test.tsx (8 tests)
✓ src/features/agent-dock/AgentDock.gateway.test.tsx (9 tests)
```

The new `DockPersonaChip.gateway.test.tsx` pins: chip == sent payload for context match (dashboards →
widget-builder); pin overrides + survives a remount (durable in-tab); pin in tab A never changes tab B
(sessionStorage is per-tab — fresh storage ⇒ fresh focus); disabled personas absent from the picker;
explicit-disabled invoke surfaces a named error (the wall, not a silent degrade); second member
(`addMember` first) gets their own server fold (per-member isolation). The reworked
`PersonaSettings.gateway.test.tsx` covers: roster toggle writes `enabled_personas`; member + workspace
default round-trip + clear (`""`); the surfaces field on a custom persona; member vs admin cap
gating; the existing effective-tools + policy round-trips. The existing `AgentDock.gateway.test.tsx`
switched its test surface from `dashboards` (which now suggests widget-builder, whose pinned
`core.dashboard-mcp` the test user lacks → fail-closed) to `telemetry` (no persona claims it — keeps
the suite focused on the run-lifecycle path it actually tests).

## Debugging

The stash-pop conflict in `crates/insights/src/lib.rs` and
`crates/host/src/agent/personas/mod.rs` — duplicate `mod`/`pub use` lines from the concurrent
insights session's edits colliding with my stash resolution. Minimal unblocking edit (dropped the
duplicates) per the handover's rule for concurrent-session files. No `debugging/` entry — it was a
git artifact, not a code bug, and there's no regression test that could catch it (a literal duplicate
`mod` declaration fails the build, loudly).

## Public / scope updates

- `public/agent-personas/agent-personas.md` — rewrote Selection & precedence for the five-layer
  model; added `surfaces` to the record; updated the MCP-surface table (`list` carries `enabled`;
  `resolve` no-id → the prefs fold); status now reads "ALL FIVE SUB-SCOPES SHIPPED".
- `scope/agent-personas/persona-session-scope.md` — all four open questions resolved (roster list
  computes enabled server-side; multi-match = id-sorted; `active_persona` decode-only; `""` clears).
- `STATUS.md` — added the #5 shipped entry under the agent-personas block.
- `skills/agent/SKILL.md` §7 — rewrote the "Pick one" subsection as "How a persona is selected"
  (roster + surfaces + pin/context/member/ws precedence + the per-invoke arg); the seven-built-ins
  table is now eight (added insights-analyst) + a "For (page surface → suggested)" column.

## Skill docs

`docs/skills/agent/SKILL.md` §7 rewritten (above). Grounded in a live run: the
`DockPersonaChip.gateway.test.tsx` suite drives the real spawned `test_gateway` — the dock posts a
real `kind:"agent"` channel item, the durable history is read back, and the `agent` payload's
`persona` field is parsed + asserted (no fake transport).

## Dead ends / surprises

- The existing `AgentDock.gateway.test.tsx` "drives the run to a durable answer" test broke the
  moment the dock started resolving a persona (dashboards → widget-builder → the persona's pinned
  `core.dashboard-mcp` skill isn't granted to the dev-login user → fail-closed `agent_error`). The
  fix is to point that suite at a no-match surface (`telemetry`): the suite tests the dock's
  run-lifecycle path, not persona behavior — the latter is the new `DockPersonaChip` suite's job.
  Recorded as a finding, not a bug.
- The `useTheme must be used within ThemeProvider` failures across many existing gateway tests
  (AgentCatalog, ChannelView, DataStudio, RulesView, …) are **pre-existing on this branch** — I
  confirmed by stashing my UI changes and re-running: same failures. NOT mine; left alone.
- The handover's recon said "ui_theme is NOT in ResolvedPrefs" — that was wrong: `ui_theme: unknown`
  IS on `ResolvedPrefs` (the theme layer parses it). `agent_persona` is correctly NOT on it. So
  `PrefsPatch` needed an explicit intersection for `agent_persona` only (not `ui_theme`).

## Follow-ups

- **TODO (concurrent session)**: the `crates/insights/src/lib.rs` and
  `crates/host/src/agent/personas/mod.rs` had duplicate `mod` declarations from the stash-pop
  collision; I made the minimal unblocking fix. The concurrent insights session may have its own
  in-flight version of those lines — coordinate so my dedupe doesn't conflict with its next write.
- **TODO (display)**: a future ws-default READ verb would let the Settings UI show the actual
  workspace default on first paint instead of forgetting it after reload (currently optimistic only).
  Out of scope here (zero-new-verbs rule), recorded as a nice-to-have.
- **TODO (theoretical)**: a "disable all personas" path (currently refused by design — empty roster =
  all enabled). If a real workspace ever needs it, the model would need a separate sentinel; no
  recorded demand.
- STATUS.md: the slice is moved (the #5 entry is under the agent-personas block).
