# Agent-personas scope — roster, context focus & session picks (persona-session)

Status: scope (the ask). Sub-scope **#5** of [`agent-personas-scope.md`](agent-personas-scope.md).
Depends on #1 ([persona-model](persona-model-scope.md)) shipped — it reuses that record and the
run-assembly seam unchanged. Promotes to
[`public/agent-personas/agent-personas.md`](../../public/agent-personas/agent-personas.md) once shipped.

Sub-scope #1 shipped selection as a **single mutable workspace toggle** —
`agent.config.active_persona`, written by the picker's "Use" button. Live use showed that model is
wrong twice over. First, it's the wrong *scope*: two members of one workspace (or one member's two
tabs) can't hold different personas — the pick is global, last-writer-wins. Second, it's the wrong
*interaction*: a persona is a per-task focus, and the agent dock already knows **where the user is**
(the page context sent on every invoke), so making a human hand-pick one workspace-wide persona is
backwards. This slice replaces the toggle with three ideas: the workspace **enables a roster** of
personas (many at once); each run still applies **exactly one**, suggested automatically from the
**page context** (on flows → flow-author) and overridable with a sticky per-tab **pin**; and the
stored *defaults* move to the prefs chain (member → workspace-default), where defaults live.

## Goals

- **A workspace enables many personas.** The Settings page becomes a roster (enable/disable per
  persona), not a one-of-N "Use" pick. Built-ins are all enabled by default; disabling curates what
  members see and what context can suggest.
- **Context picks the focus by default.** The dock derives the suggested persona from the page the
  user is on — flows canvas → flow-author, dashboards → widget-builder — with **zero** user action.
  The mapping is **data on the persona record** (a `surfaces` list), never a branch in core (rule 10).
- **The user can pin, per tab.** A manual pick in the dock overrides the context suggestion and
  sticks for that tab until cleared. Two members / two tabs are fully independent.
- **Defaults live in prefs.** When neither pin nor context decides: the member's default persona
  (per-user, per-workspace), then the workspace default (admin-set — the on-by-default global),
  then none (un-narrowed). Headless callers (flows, webhooks, cron) land on the workspace default.
- **Exactly one persona per run, always.** Composition of focuses stays where #1 put it: `extends`
  on a record (rules-author *is* flow+data; system-manager *is* all six).

## Non-goals

- **No runtime union of N personas into one run.** Considered and **rejected** (the headline
  decision): a persona's `identity` is prose — *"You are a flow author…"* — and unioning three
  identities, menus, and pinned-skill sets rebuilds the exact "confused by everything" symptom this
  topic exists to fix (and blows the ≤4-pinned-skills context budget). "Many at once" is served by
  `extends` composition **as a record** (zero code, one deliberate identity) — a workspace wanting
  flow+data+widgets authors a custom persona extending those three.
- **No server-side session/tab identity.** No per-(member, tab) record, no GC story. The client
  holds pin + context match and sends the resolved id per invoke; the durable job record is the
  audit of what ran.
- **Runtime/definition selection untouched.** `active_definition` / `default_runtime` stay
  workspace-scoped; this slice sets the session>member>workspace pattern a later slice may adopt.
- **No change to run assembly.** `apply.rs`, `resolve_effective`, menu-narrow / identity-fold /
  skill-pin: unchanged. Only *selection* is reworked.
- **No persona partitioning of memory** (umbrella decision, kept).

## Intent / approach

**Selection becomes a five-layer resolution, the top two client-side, the rest server-side:**

```
CLIENT (the dock, per tab)
  1. pin            — the user picked one in this tab (sessionStorage); sticky until cleared
  2. context match  — current page surface ∈ persona.surfaces, over the ENABLED roster
        │ the dock always sends the id it shows, as the invoke `persona` arg (shipped seam, #1)
        ▼
SERVER (resolve_persona — step 2 re-homed)
  3. member default    — Prefs.agent_persona on user_prefs:[ws,member]     (member-writable)
  4. workspace default — Prefs.agent_persona on workspace_prefs:[ws]       (admin-writable)
  5. none              — un-narrowed, exactly as today
```

1. **The roster.** `agent.config` swaps `active_persona` for an additive optional
   `enabled_personas: Option<Vec<String>>` — `None` (default) = **all** personas enabled (built-ins +
   workspace customs, on-by-default out of the box); `Some(list)` = only those ids. Same record, same
   MERGE patch, same admin `agent.config.set` gate as `active_definition`. Enablement is
   *curation of the advertisement layer*: a disabled persona is hidden from `agent.persona.list`'s
   picker view and from context matching, and an **explicit invoke of a disabled persona fails with a
   named error** (curation must not be silently bypassable; the wall beneath is unchanged either way).
2. **The context map is data on the record.** `Persona` gains `surfaces: Vec<String>` — opaque
   strings compared for equality against the `context.surface` the dock already sends on every invoke
   (agent-dock scope). Built-ins declare theirs in `personas.toml` (flow-author `["flows"]`,
   rules-author `["flows","rules"]`, widget-builder `["dashboards","data-studio"]`, …; system-manager
   `[]` — it is the fallback map, not a page). **The client does the matching** over the enabled
   roster it fetched via `agent.persona.list`: the host never sees a surface→persona rule, never
   branches on either id (rule 10 — swap test: a new page + a new persona pair up by editing records
   only). More than one match → the dock suggests the first enabled match and offers the rest in the
   switcher; no match → fall through to the server layers (send no `persona`).
3. **Pin beats suggestion, per tab.** The dock stores an explicit pick in `sessionStorage` (keyed by
   workspace). Pin set → send it; else context match → send it; else send nothing. The dock always
   displays what it will send, annotated with *why* ("pinned" / "from this page" / "workspace
   default") — the chip and the run must never disagree.
4. **Defaults are a prefs axis.** `agent_persona: Option<String>` on `lb_prefs::Prefs` — the fifth
   reuse of the closed-struct nullable-axis pattern (`ui_theme`, `insight_notifications` precedents:
   on `Prefs`, **not** `ResolvedPrefs`; not an i18n axis; no `format.*` reads it). Member default via
   shipped `prefs.set`; workspace default via shipped admin-gated `prefs.set_default`.
   `resolve_persona` step (2) swaps its source from `agent.config.active_persona` to a targeted
   two-link fold (member record → ws-default record, first `Some` wins). Dangling-id posture kept
   verbatim: `warn!` + run un-narrowed, never an errored run.

**Rejected: runtime union of picked personas** — see Non-goals; identity prose does not union, and
`extends` already composes curated combinations as records with child-wins identity semantics.

**Rejected: server-side surface→persona mapping table.** A host-side map is either a branch on two
opaque ids (rule 10 leak) or a new admin-managed table duplicating what the persona record + client
match do with zero new state. The persona already owns "what I'm for"; `surfaces` is more of the same.

**Rejected: keep `active_persona` beside the new layers.** Two "active" concepts is the ambiguity
that caused the bug. One-way migration: at boot, a set `active_persona` is copied once into the
workspace-default prefs axis (idempotent), then never read again (field stays decode-only on
`AgentConfig` so old records/replays don't break).

**Rejected: a new `member_agent_config` table for the member default.** Prefs already *is* the
per-member-per-workspace record with a member→ws-default fold and shipped two-tier write gates.

## How it fits the core

- **Tenancy / isolation:** `enabled_personas` rides the ws-scoped `agent.config`; `agent_persona`
  rides the existing `user_prefs:[ws,member]` / `workspace_prefs:[ws]` records; `surfaces` rides the
  persona record (built-in reserved ns / ws-custom). All already behind the hard wall. A ws-B default
  or pin can never resolve a ws-A custom persona (#1's namespace-walled `read_persona_for_assembly`,
  unchanged). Mandatory isolation test below.
- **Capabilities:** the wall is untouched — every layer here is *advertisement/selection*, narrowing
  only. Writes: roster = admin `mcp:agent.config.set:call` (existing); member default = member
  `mcp:prefs.set:call`; ws default = admin `mcp:prefs.set_default:call`. **Deny paths:** a member
  writing the roster or the ws default is denied at the wall. *Dev note:* dev-login `member_caps()`
  omits `mcp:prefs.set_default:call` — seed an admin via `signInWithCaps` for ws-default test legs.
- **Placement:** either — pure state reads on the symmetric dispatch seam; the client-side layers are
  role-free by construction. Headless callers send no `persona` and fold to the ws default.
- **MCP surface:** **one field on two existing records; zero new verbs.** `agent.config.get/set`
  carry `enabled_personas`; `agent.persona.list` output includes `surfaces` + an `enabled` flag
  (computed against the roster) so the picker/dock need one fetch; `prefs.set`/`prefs.set_default`
  carry the new axis; per-run selection is the shipped `agent.invoke { persona }` /
  `AgentPayload.persona`. No live feed (roster/defaults change rarely; refetch), no batch.
- **Data (SurrealDB):** additive nullable fields — `enabled_personas` on `workspace_agent_config`,
  `surfaces` on the persona tables + `personas.toml` re-seed, `agent_persona` on the prefs schema.
  Plus the one-shot `active_persona` → ws-default-prefs boot migration (idempotent UPSERT).
- **Bus (Zenoh):** none — selection is pure state read at assembly; nothing rides the bus.
- **Sync / authority:** node-local reads, same as every prefs/config read; offline a stored default
  resolves locally and a pin rides inside the durable invoke request.
- **Secrets:** none.
- **State vs motion / stateless extensions / symmetric nodes:** unchanged; no extension touched.
- **SDK/WIT impact:** none.
- **File layout:** touched files stay one-verb (`resolve.rs` swap, `model.rs`+`validate.rs` for
  `surfaces`, `config/{model,store}.rs` for the roster, a new `migrate_active_persona.rs` beside the
  seeders); UI changes live in the persona Settings feature dir + the dock.

## Example flow

1. **Fresh workspace.** No roster set (`None` = all enabled), no defaults. Member A opens the flows
   canvas and asks the dock a question. The dock's roster fetch shows flow-author matches surface
   `"flows"` → chip reads *"Focus: Flow author — from this page"*; invoke sends
   `persona: "builtin.flow-author"`. Zero setup, focused run.
2. **A wants data instead.** Still on flows, A opens the switcher and pins Data analyst. Pin lands in
   this tab's `sessionStorage`; every invoke from this tab now sends data-analyst — until A clears
   the pin (chip returns to the context suggestion). A's **other tab** on dashboards keeps
   suggesting widget-builder. Member B, same workspace, same moment: untouched by all of it.
3. **Admin curates.** In Settings the admin disables extension-builder and workspace-admin for this
   workspace (`agent.config.set { enabled_personas: [...] }`) and clicks "Set as workspace default"
   on system-manager (`prefs.set_default { agent_persona: "builtin.system-manager" }`). Disabled
   personas vanish from every member's picker and from context matching; an explicit invoke naming
   one fails with the named disabled error.
4. **A page with no persona.** Member B asks from a page whose surface matches nothing. The dock
   sends no `persona`; the server folds: B has no member default → ws default system-manager → run
   under the broad operator, which hands off ("for flows work, switch to Flow author").
5. **Headless.** A webhook-triggered flow invokes the agent: no session, no `persona` arg → ws
   default system-manager. Deterministic, admin-owned.
6. **Composition, done right.** A wants flow+data+widgets as one focus: the admin creates custom
   persona `studio` with `extends = ["builtin.flow-author","builtin.data-analyst","builtin.widget-builder"]`,
   `surfaces = ["flows","dashboards"]` — a record, zero code. It appears in the roster and now
   context-suggests on both pages.

## Testing plan

Mandatory categories from [`scope/testing/testing-scope.md`](../testing/testing-scope.md):

- **Workspace isolation (mandatory).** A ws-A member default / ws-A roster never affects a ws-B run;
  a ws-A custom persona id stored as ws-A's default is unresolvable from ws-B (folds to none +
  `warn!`). Real records, real store, both workspaces.
- **Capability deny (mandatory).** Member denied on `agent.config.set { enabled_personas }` and on
  `prefs.set_default { agent_persona }`; member allowed on `prefs.set { agent_persona }`. Admin legs
  seeded via `signInWithCaps`.
- **Precedence table (host).** explicit invoke id > member default > ws default > none; dangling
  member/ws default → `warn!` + un-narrowed; explicit-but-unknown id → named error (#1, unchanged);
  explicit-but-**disabled** id → named disabled error.
- **Roster semantics (host).** `None` ⇒ all enabled (list shows every persona `enabled: true`);
  `Some(list)` filters the picker view + fails explicit disabled invokes; a roster naming a deleted
  persona is inert (no error).
- **Independence (the headline).** Two members of one workspace resolve different personas in the
  same tick with zero cross-writes; two invokes with different explicit ids under one member both
  narrow correctly (proves per-tab needs no server state).
- **Migration.** Legacy `active_persona` set → after boot, ws-default prefs axis carries it and a
  second boot is idempotent; unset → no ws default appears.
- **Surfaces as data (rule-10 swap test).** A custom persona created with a novel `surfaces` entry is
  suggested by a dock pointed at that surface — record edit only, zero code change.
- **UI gateway (real spawned `test_gateway`, rule 9 — no fakes).** Dock chip == sent `persona`
  (context match); pin overrides and survives within the tab; pin in tab A never changes tab B
  (two clients); "Use" writes no server record; disabled personas absent from the picker; second
  member (`addMember` before second login) sees their own suggestion.

No mocks: real SurrealDB (`mem://`), real records, real dispatch, real gateway. The only sanctioned
fake anywhere in the topic remains the provider HTTP (`MockProvider`).

## Skill doc

**Update, not new.** `skills/agent/SKILL.md`'s persona how-to currently documents
`active_persona` — wrong once this ships (a finding, not a nicety). The implementing session rewrites
it: roster, `surfaces`, pin/context/member/ws precedence, and that the *current* pick is a per-invoke
arg, grounded in a live run. No new drivable surface ⇒ no new SKILL.md.

## Risks & hard problems

- **The dual-owner trap resurfacing.** The bug was two "active" concepts. Keep exactly one stored
  thing per layer: roster (config), defaults (prefs), pin (client). Resist any "sticky server-side
  last pick" — that's the toggle again.
- **Chip/run divergence.** The model only holds if the dock always sends what it displays. One
  gateway test pins this: assert the invoke payload equals the chip's persona for pin, context, and
  default states.
- **Surface-string drift.** `surfaces` entries only work if the dock's `context.surface` vocabulary
  is stable. The built-ins' entries must be taken from the *live* dock values at implementation time
  (grep the UI, don't guess); a drifted string degrades silently to "no match → default" (safe, but
  quietly useless). List the vocabulary in the session doc.
- **Enablement vs explicit invoke.** Failing explicit invokes of disabled personas is a behavior
  change from #1 (which had no disabled state). Named error, clear message — but flows/scripts that
  hard-code a persona id can start failing when an admin disables it. That is curation working as
  intended; the error must say so.
- **Migration edge.** A legacy `active_persona` naming a since-deleted persona: migrate the id
  anyway (the fold's dangling-id path already warns + runs un-narrowed — no special case).

## Open questions

1. **Does `agent.persona.list` compute `enabled` server-side or ship the roster raw?** ✅ RESOLVED
   (shipped): **compute it server-side**. List returns `PersonaListItem = Persona & { enabled: bool }`
   per row (computed against the workspace roster — None or empty = all enabled); the raw roster still
   rides `agent.config.get` for the Settings editor. One fetch for the dock + picker, no client-side
   roster join. Backend: `crates/host/src/agent/personas/list.rs`; UI: `agentPersona.api.ts`.
2. **Multi-match ordering.** ✅ RESOLVED (shipped): when two enabled personas match one surface, the
   **id-sorted (store list order)** first match is suggested; the rest ride the chip switcher. NOT
   seed order — the host's `agent.persona.list` is already id-sorted, and we keep that order end to
   end so the dock + a future debug view agree. No new priority field. (`usePersonaFocus.ts`'s
   `options.find((p) => p.surfaces.includes(surface))` over the roster-as-returned.)
3. **Retire `active_persona` fully?** ✅ RESOLVED (shipped): **decode-only** on `AgentConfig`
   (`#[serde(default, skip_serializing)]` — never serialized, never read by resolution), dropped from
   `AGENT_CONFIG_COLUMNS` reads; the column is tolerated in old records and migrated once at boot into
   the ws-default prefs axis. UI: removed from `AgentConfig` in `config.api.ts`.
4. **"None" at the ws-default layer.** ✅ RESOLVED (shipped): the picker writes **`""`** to clear the
   axis (the MERGE-can't-write-null workaround — `setPrefs/setDefaultPrefs({ agent_persona: "" })`).
   The consumer's `filter(|s| !s.is_empty())` treats `""` as unset. Same guard `#1` already used; the
   Settings UI's "Clear my default" / "Clear ws default" buttons call exactly this. **Empty roster
   (Some([])) is also "cleared" (= all enabled)** — disabling ALL personas is unsupported by design.

## Related

- [`agent-personas-scope.md`](agent-personas-scope.md) — the umbrella; this is sub-scope **#5**,
  replacing #1's single-toggle selection with roster + context focus + session picks.
- [`persona-model-scope.md`](persona-model-scope.md) (#1) — the record, the run-assembly seam, and
  the per-invoke override this slice leans on; only its "Selection" bullet is superseded.
- [`persona-catalog-scope.md`](persona-catalog-scope.md) (#3) — `personas.toml` gains `surfaces`
  per built-in; the catalog's `extends` composition is the sanctioned "many at once".
- `scope/agent/agent-dock-scope.md` — the page-context (`{surface, path, search}`) envelope the
  context match reads; `scope/agent/agent-config-scope.md` — the record the roster rides and
  `active_persona` retires from.
- `scope/prefs/` + `lb_prefs::Prefs` — the nullable-axis pattern (`ui_theme`,
  `insight_notifications` precedents). Memories: [[prefs-closed-struct-not-kv]],
  [[dev-login-missing-set-default-cap]].
- README `§6.5` (AI core / dispatch seam), `§7` (capabilities).
