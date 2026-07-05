# Agent-personas scope — the persona is session state, not a workspace toggle (persona-session)

Status: scope (the ask). Sub-scope **#5** of [`agent-personas-scope.md`](agent-personas-scope.md).
Depends on #1 ([persona-model](persona-model-scope.md)) shipped — it reuses that record and its
run-assembly seam unchanged. Promotes to
[`public/agent-personas/agent-personas.md`](../../public/agent-personas/agent-personas.md) once shipped.

Sub-scope #1 shipped a **single** selector — `agent.config.active_persona`, one mutable record per
workspace — and wired the picker's "Use" button to write it. That makes "which persona is active" a
**global, last-writer-wins toggle**: two members of one workspace can't hold different personas, and
one member's two browser tabs stomp each other. But a persona is a *focus* chosen per task, often per
message — it is **session state**, like which channel a tab is looking at, not workspace
infrastructure. This slice moves "active" out of the shared workspace record and re-homes the
**default** into the prefs chain (member → workspace-default), leaving the *current* pick where it
belongs: on the client, sent per invoke through the override seam #1 already built.

## Goals

- **Concurrent, independent picks.** Two members of one workspace, and one member's N tabs, each hold
  their own active persona with **zero** server-side contention. Picking in tab A never changes tab B.
- **A per-member default.** "When *I* open a fresh agent surface in this workspace, start me on
  Data Analyst." Per-user, per-workspace, member-writable.
- **A workspace default (the on-by-default / global story).** An admin sets the persona a fresh
  session lands on when the member has no personal default — including "none" (un-narrowed). This is
  the auditable, admin-owned global, *not* whatever the last person clicked.
- **Headless callers land on the workspace default.** Flows, webhooks, cron, jobs — anything with no
  client session — resolve to the workspace default, which is exactly right for unattended runs.
- **No new backend plumbing for the current pick.** The per-invoke `persona` override (#1,
  `AgentPayload.persona` / `agent.invoke { persona }`, highest precedence) already makes a run
  self-describing; the client just always sends it. This slice adds *defaults*, not a new live path.

## Non-goals

- **No server-side session/tab identity.** We do not mint a per-(member, tab) "session" record. The
  client holds the current pick and sends it per invoke; the durable job record already captures which
  persona ran. (Rejected alternative — see Intent.)
- **Runtime/definition selection is untouched.** `agent.config.active_definition` and
  `default_runtime` stay workspace-scoped for now — they are genuinely "which engine/model this
  workspace runs," not per-session focus. This slice sets the *pattern* (session > member > workspace)
  that a later definition slice can adopt; it does not do that migration.
- **No change to run assembly.** `apply.rs`, `resolve_effective`, the menu-narrow / identity-fold /
  skill-pin — all unchanged. Only step (2) of `resolve_persona` (the "active" lookup) is re-homed.
- **No persona partitioning of memory** (the umbrella's kept decision) — a persona switch, tab, or
  member still shares the one workspace+member memory.

## Intent / approach

**Three layers, resolved highest-first — the exact shape #1's precedence already implies, with the
middle layer re-homed from a shared toggle to the prefs chain:**

```
1. session pick   (the tab)         → agent.invoke { persona } / AgentPayload.persona   [client-held, per invoke]
2. member default (per-user/ws)     → Prefs.agent_persona   (user_prefs:[ws,member])    [member-writable]
3. workspace default (admin)        → Prefs.agent_persona   (workspace_prefs:[ws])       [admin-writable]
   else none (un-narrowed)
```

1. **The current pick is client state, sent per invoke.** The picker "Use" button stops writing any
   server record. The UI keeps the picked persona per tab (`sessionStorage`, keyed by workspace) and
   attaches it as `persona` on **every** agent invoke. The backend already treats an explicit id as
   highest precedence ([`resolve_persona`](../../rust/crates/host/src/agent/personas/resolve.rs) step
   1) — so **this layer needs no Rust change**. Two tabs → two picks; two members → two picks; no
   store, no race, no invalidation.

2. **The default is a new prefs axis** — `agent_persona: Option<String>` on `lb_prefs::Prefs`, the
   **fifth reuse** of the closed-struct nullable-axis pattern (`ui_theme`, `insight_notifications` are
   the precedents: whole-fold, serde-default, a *non-i18n* axis riding the prefs record). Member sets
   it via the shipped `prefs.set` path ("Set as my default" in the picker). Workspace-default is the
   *same axis* at the `workspace_prefs:[ws]` level, written via the shipped **admin-gated**
   `prefs.set_default` ("Set as workspace default"). The prefs chain's member→ws-default fold is
   exactly the layer-2→layer-3 precedence we want, **for free**.

   Like `insight_notifications`, `agent_persona` lives on `Prefs` but **not** on `ResolvedPrefs` — it
   is not an i18n axis and no `format.*` reads it. The host reads it with a **targeted two-link fold**
   at run assembly (member record, then ws-default record — the first `Some` wins), not through the
   `format.*` resolution path.

3. **`resolve_persona` step (2) swaps its source.** Today it reads
   `agent.config.active_persona`; it will instead fold `Prefs.agent_persona` (member override → ws
   default). The dangling-id posture is **kept verbatim**: an id that no longer resolves → `warn!` +
   run un-narrowed, never an errored run. Step (1) explicit-override and step (3) none are unchanged.

**Rejected: a server-side session/tab record.** Persisting a `session:[member, tab]` "active persona"
adds a session-identity concept the platform otherwise has nowhere else, needs a GC story, and buys
nothing: the per-invoke override already makes each run self-describing and durable (the job record is
the audit trail of what ran). The client is the right owner of "what this tab is looking at" — the
same way the UI already owns which channel/panel a tab shows.

**Rejected: keep `active_persona` and add a member overlay beside it.** Two "active" concepts (a ws
toggle *and* a member default) is the ambiguity that caused this bug. Collapsing "active" to a
per-invoke arg and re-homing "default" into the one prefs chain leaves exactly **one** default
mechanism with a clear owner per layer.

**Rejected: a brand-new `member_agent_config` table.** Prefs already *is* the per-member-per-workspace
record with a member→ws-default fold and shipped member/admin write gates. A parallel table would
re-implement that fold and its two-tier caps for no gain (rule: one datastore, don't sneak in a new
persistence shape).

## How it fits the core

- **Tenancy / isolation:** `agent_persona` rides the existing `user_prefs:[ws,member]` and
  `workspace_prefs:[ws]` records — already the hard wall. A ws-B member's default can never resolve a
  ws-A persona: the id is read at assembly through #1's namespace-walled `read_persona_for_assembly`
  (a `builtin.*` id from the reserved ns, any other id **only** from the run's `ws`). Mandatory
  isolation test below.
- **Capabilities:** unchanged wall. Layer 1 (invoke override) and the persona read are **narrowing-
  only** (#1's decided posture — a run-assembly persona read is not cap-gated because it can only
  remove tools). Layer 2 write = member's shipped `mcp:prefs.set:call`. Layer 3 write = admin's
  `mcp:prefs.set_default:call`. **Deny path:** a member calling `prefs.set_default` to set a
  workspace-wide persona is denied at the wall (the correct posture — the global is an admin decision).
  *Dev note:* dev-login's `member_caps()` omits `mcp:prefs.set_default:call` (there is no
  `*.set_default` wildcard), so ws-default persona tests must seed an admin via `signInWithCaps`
  (recorded in [`dev-login-missing-set-default-cap`] memory).
- **Placement:** either — pure state read at run assembly on the symmetric dispatch seam; no role
  branch. Headless (cloud/edge) callers carry no session and fold straight to the ws default.
- **MCP surface:** **no new verbs.** Reads: the two prefs records via the shipped store reads (folded
  at assembly). Writes: the shipped `prefs.set` (member default) and `prefs.set_default` (ws default).
  Selection-per-run: the shipped `agent.invoke { persona }` / `AgentPayload.persona`. CRUD of the
  persona *records* stays in #1's `agent.persona.*`. No live feed (a default changes rarely; the
  picker refetches), no batch. This slice is a **re-homing**, not a new API.
- **Data (SurrealDB):** one new nullable column, `agent_persona option<string>`, added to the prefs
  schema (member + ws-default tables share the `Prefs` shape). **Migration:** at boot/seed, if a
  workspace has `agent.config.active_persona` set, copy it once into that workspace's
  `workspace_prefs.agent_persona` (idempotent UPSERT), then stop reading `active_persona`. The
  `active_persona` field stays **decodable** on `AgentConfig` (old records, ignored) — a one-way
  retire, no dual-read window.
- **Bus (Zenoh):** none — a persona/default is pure state; nothing rides the bus (unchanged from #1).
- **Sync / authority:** node-local read at assembly, same as every prefs read; the prefs records are
  workspace-authoritative and sync exactly as `ui_theme`/`insight_notifications` do. Offline: a stored
  default resolves offline; a client's session pick rides in the (durable) invoke request.
- **Secrets:** none.

## Example flow

1. **Admin sets the global.** In Settings → Agent, the admin picks `builtin.system-manager` and clicks
   **Set as workspace default**. The UI calls `prefs.set_default { agent_persona: "builtin.system-manager" }`
   (admin-gated). `workspace_prefs:[ws].agent_persona` is now `system-manager`.
2. **Member A, tab 1** opens the dock. No session pick yet, no personal default → the fresh surface
   defaults its picker to the workspace default (`system-manager`). Member A posts a goal. The invoke
   sends `persona: "builtin.system-manager"` (the resolved default the tab is showing). Run narrows to
   the system-manager focus.
3. **Member A, tab 2** opens and picks `builtin.data-analyst` from the picker (client-only —
   `sessionStorage`). Posts a goal → invoke sends `persona: "builtin.data-analyst"`. **Tab 1 is
   unaffected** — it still holds `system-manager`. Two tabs, two live personas, one member.
4. **Member A clicks "Set as my default"** in tab 2. UI calls `prefs.set { agent_persona: "builtin.data-analyst" }`
   (member-gated). Now `user_prefs:[ws, A].agent_persona = data-analyst`. A *future* fresh tab for
   member A defaults to data-analyst; the ws default still governs everyone else.
5. **Member B** opens the dock: no personal default → folds to the ws default (`system-manager`).
   Member A's personal default never touched B. **Concurrent, isolated.**
6. **A webhook fires a flow** that invokes the agent with no `persona` arg and no session. Resolution:
   no explicit → fold `Prefs.agent_persona` (no member record for a system principal → ws default
   `system-manager`) → run under the workspace default. Unattended, deterministic.
7. **The ws default is later cleared** (`prefs.set_default { agent_persona: null }`). A fresh session
   with no personal default resolves to **none** — the run is un-narrowed, exactly as a persona-less
   workspace behaves today.

## Testing plan

Mandatory categories from [`scope/testing/testing-scope.md`](../testing/testing-scope.md) that apply:

- **Workspace isolation (mandatory).** A member-default `agent_persona` set in ws-A never resolves for
  a ws-B run; a ws-A custom persona id in ws-A's default is unreadable from ws-B (folds to none +
  `warn!`, per #1's namespace wall). Real records in the real store, both workspaces.
- **Capability deny (mandatory).** A member calling `prefs.set_default { agent_persona }` is denied at
  the wall (only `prefs.set` — the member default — succeeds for a non-admin). Seed the admin via
  `signInWithCaps` for the ws-default write leg.
- **Concurrency / independence (the headline).** Two members of one workspace resolve **different**
  active personas in the same tick (member-default A vs ws-default) with no cross-write; two invokes
  carrying different explicit `persona` ids under one member both narrow correctly (proves the
  client-per-tab model needs no server state). Real store, real dispatch seam.
- **Resolution precedence.** Table test over the fold: explicit invoke id > member default > ws
  default > none; a dangling member/ws default → `warn!` + un-narrowed (dangling-id posture kept from
  #1); explicit-but-unknown id still a named error (#1, unchanged).
- **Migration.** A workspace seeded with legacy `agent.config.active_persona` boots with
  `workspace_prefs.agent_persona` populated to the same id; a second boot is idempotent; a workspace
  with no legacy value gets no ws default.
- **UI gateway (real spawned gateway, no fakes — rule 9).** Against `test_gateway`: the picker "Use"
  button writes only `sessionStorage` and sends `persona` on invoke (no store write); "Set as my
  default" round-trips through real `prefs.set`; the admin-only "Set as workspace default" affects a
  second member's fresh-session default. Seed a second member with `addMember` before the second-user
  login.

No mocks: real SurrealDB (`mem://`), real prefs records, real dispatch, real spawned gateway. The only
sanctioned fake anywhere in this topic stays the provider HTTP (`MockProvider`) — untouched here.

## Skill doc

**Update, not new.** `skills/agent/SKILL.md` (which #1's session created for the persona how-to) gains
a section: how a session pick, a member default (`prefs.set`), and a workspace default
(`prefs.set_default`) resolve, and that the *current* pick is a per-invoke `persona` arg, not a stored
"active." No new drivable surface is introduced (the verbs are all shipped), so no new SKILL.md — but
the persona how-to is now **wrong** until updated (it documents `active_persona`), which is a finding,
not a nicety. The implementing session owns the edit, grounded in a live run.

## Risks & hard problems

- **The dual-owner trap resurfacing.** The whole bug was two "active" concepts. Keep it collapsed: the
  current pick is **only** the invoke arg; the **only** stored thing is the default. Resist adding a
  "sticky server-side last pick" — that reintroduces the toggle.
- **Migration edge — a legacy `active_persona` naming a ws-custom persona.** The copy into
  `workspace_prefs` is a plain id string; it resolves through #1's namespace wall unchanged, so a
  ws-custom id keeps working. A `builtin.*` id likewise. Test both legs.
- **Client discipline.** The model only works if the UI **always** sends `persona` on invoke. A tab
  that forgets silently falls back to the member/ws default — usually benign, but confusing if the tab
  *shows* a different persona than it sends. The picker must send exactly what it displays; add a
  gateway test asserting the invoke payload matches the shown persona.
- **Headless "no session" is a feature, not a gap.** Confirm every headless invoke path (flow node,
  webhook, cron, job) reaches `resolve_persona` with `persona: None` so it folds to the ws default —
  a path that hard-codes a persona would bypass the admin's global.

## Open questions

1. **Retire `active_persona` fully, or keep the field forever?** Recommendation: keep it
   **decode-only** on `AgentConfig` (serde-default, never read) so old records/replays don't break,
   and drop the schema column read from `AGENT_CONFIG_COLUMNS`. Removing the field outright risks an
   offline edge replaying an old `agent.config` patch. Decide at implementation.
2. **Does the picker need a visible "(workspace default)" / "(your default)" annotation** on the
   active chip so a member understands *why* a persona is selected on a fresh tab? Recommendation:
   yes, a small annotation — it's the difference between "I picked this" and "this is the house
   default," and it makes the three layers legible. UI-only.
3. **Should `default_runtime`/`active_definition` follow into prefs now** (the same session>member>ws
   argument)? Recommendation: **no, defer** — this slice sets the pattern; a definition migration is a
   separate ask with its own migration and picker work. Note it in the umbrella as the next candidate.
4. **Empty string vs `null` for "explicitly no persona" at the ws-default layer.** `Prefs`'s
   `skip_serializing_if = "Option::is_none"` means `null` = "inherit," but the ws-default *is* the
   bottom of the fold, so `null` there already means "none." Confirm the picker's "None" option writes
   `null` (clear the axis), not an empty-string sentinel — matches #1's `filter(|s| !s.is_empty())`
   guards. Decide at implementation.

## Related

- [`agent-personas-scope.md`](agent-personas-scope.md) — the umbrella; this is sub-scope **#5**,
  correcting #1's single-toggle selection into a session/member/workspace model. Update the umbrella's
  sub-scope table and the architecture map's "writes `agent.config.active_persona`" line.
- [`persona-model-scope.md`](persona-model-scope.md) (#1) — the persona **record**, the run-assembly
  seam, and the per-invoke `persona` override this slice leans on entirely; only its "Selection" bullet
  (`agent.config.active_persona`) is superseded here.
- `scope/prefs/` + the `lb_prefs::Prefs` closed struct — the nullable-axis pattern reused
  (`ui_theme`, `insight_notifications` precedents: on `Prefs`, **not** on `ResolvedPrefs`, host-read
  targeted). Memories: [[prefs-closed-struct-not-kv]], [[dev-login-missing-set-default-cap]].
- `scope/agent/agent-config-scope.md` — the `agent.config` record `active_persona` is retired from.
- README `§6.5` (the AI core / dispatch seam), `§7` (capabilities), the prefs section.
