# Agent-personas scope — the built-in persona catalog (persona-catalog)

Status: **SHIPPED** (the 7 built-ins as `personas.toml` data, verb-lists cross-verified against the live
inventory; 8 catalog tests incl. the confusion demo green). Sub-scope #3 of `agent-personas-scope.md`.
Session: [`sessions/agent-personas/persona-catalog-session.md`](../../sessions/agent-personas/persona-catalog-session.md).
Promoted to [`public/agent-personas/agent-personas.md`](../../public/agent-personas/agent-personas.md).
See the **Implementation finding** below (the palette-catalog reach) — recorded, not coded around.

Ship the **built-in personas as data**: a `personas.toml` seed (the `agents.toml` move) defining
the seven workspace-selectable focuses. This doc is deliberately concrete — each persona's exact
tool list and pinned skills — because a persona missing a verb is a broken persona; the lists
below come from the live verb inventory (caps seed `role/gateway/src/session/credentials.rs` +
host verb registrations), not from memory. **Zero new code**: #1 built the record and the
application; this is content + seed + the Settings picker rows.

Conventions: every listed tool still passes the unchanged wall (`persona ∩ agent ∩ caller`) —
an admin-tier verb in a persona does nothing for a member caller. Globs are trailing-`*` on the
tool id. Pins are body-injected skills (≤ 4 each, context budget); the filtered catalog carries
the rest of each persona's granted skills as activate-on-demand.

## The seven built-ins

### 1. `builtin.data-analyst` — "full access to datasources, SurrealDB and so on"

- **identity:** a data analyst over the workspace's datasources and store; explores schemas,
  writes/saves queries, reads series, answers with tables/charts (`viz.query`); verifies
  against the real store, never invents columns.
- **granted_tools:** `federation.query`, `federation.schema`, `datasource.list`,
  `datasource.test`, `datasource.add`, `datasource.remove`, `query.*` (save/run/compile/delete),
  `viz.query`, `series.*` (read/latest/find/list/watch), `store.query`, `store.schema`,
  `tags.find`, `tags.add`, `ingest.write`. Admin lens when held: `store.tables`, `store.scan`,
  `store.graph`.
- **grounding_skills (pinned):** `core.datasources`, `core.query`, `core.store-read`,
  `core.ingest-series`. Catalog: `core.tags`, `core.lb-cli`, `core.testing-datasources`.

### 2. `builtin.flow-author` — write and operate flows

- **identity:** a flow author on the typed-node DAG engine; builds/enables/injects/watches
  flows, wires envelopes and `${steps.x}` bindings, debugs via `flows.node_state` and run
  history.
- **granted_tools:** `flows.*` (all: save/get/list/delete/nodes/run/resume/suspend/cancel/
  patch_run/runs.get/runs.list/watch/node.get/node.update/node_state/enable/inject) + the reads
  flows bind to: `series.read`, `series.latest`, `series.find`, `series.list`,
  `federation.query`, `bus.watch`, `viz.query` (dashboard-binding read-back).
- **grounding_skills (pinned):** `core.flows-mcp`, `core.ingest-series`, `core.query`.
  Catalog: `core.dashboard-widgets` (flow↔dashboard binding), `core.lb-cli`.

### 3. `builtin.widget-builder` — Data Studio / charts / GenUI / render templates

- **identity:** a dashboard and widget builder; authors panels, pins tool results, builds GenUI
  cells and render templates, always through the one viz bridge and existing verbs.
- **granted_tools:** `dashboard.*` (get/list/save/delete/share/catalog/pin), `panel.*`
  (get/list/save/delete/share/usage), `layout.get`, `layout.set`, `template.*`
  (save/get/list/delete), `viz.query`, `query.run`, `query.save`, `query.compile`,
  `federation.query`, `federation.schema`, `series.read`, `series.latest`, `series.list`.
  (GenUI adds no verbs — a `view:"genui"` cell persists through `dashboard.save`; the persona
  needs only the source-read verbs above, exactly the genui-scope posture.)
- **grounding_skills (pinned):** `core.dashboard-mcp`, `core.genui-widget`, `core.panels`,
  `core.dashboard-widgets`. Catalog: `core.graphics-canvas`, `core.query`,
  `core.testing-charts`, `core.testing-dashboard`.

### 4. `builtin.rules-author` — rules, with flows + data auto-loaded

- **identity:** authors and tests rules; because rules + workflow are converging on the flows
  engine, it composes the flow-author and data-analyst surfaces rather than duplicating them.
- **extends:** `builtin.flow-author`, `builtin.data-analyst` — **the `extends` showcase** (#1):
  resolve-at-read means when those parents grow, rules-author follows; when the rules-in-flows
  work lands, updating the parents updates this persona for free.
- **granted_tools (own):** `rules.*` (run/save/get/list/delete), `rules.help`,
  `reminder.*` (create/list/get/delete/fire — scheduled rule triggers).
- **grounding_skills (pinned, own):** `core.rules` (+ inherited pins deduped; #1 caps total
  body pins at 4 — inherited pins beyond that demote to catalog).

### 5. `builtin.workspace-admin` — nav, users, teams, access

- **identity:** a workspace administrator; manages the nav surface, members/teams/roles/grants,
  invites users, sets workspace defaults; states the deny reason when the caller lacks a cap
  rather than retrying.
- **granted_tools:** `nav.*` (get/list/resolve/save/delete/share/set_default), `members.*`
  (list/add/manage), `identity.manage`, `user.manage`, `user.disable`, `teams.list`,
  `teams.manage`, `workspace.list`, `workspace.create`, `workspace.rename`, `grants.assign`,
  `grants.list`, `roles.define`, `roles.list`, `roles.manage`, `authz.resolve`,
  `prefs.set_default`, `prefs.catalog`, `apikey.manage`, `webhook.manage`.
  **Deliberately excluded:** `workspace.delete`, `workspace.purge`, `authz.revoke-tokens` —
  destructive/security verbs stay human-driven even for an admin caller (the persona narrows;
  see Risks).
- **grounding_skills (pinned):** `core.nav`, `core.auth-caps`, `core.prefs`. Catalog:
  `core.testing-nav`, `core.lb-cli`.

### 6. `builtin.channels-operator` — channels, inbox/outbox, messaging

- **identity:** operates the messaging plane — posts/reads channels, drives inbox triage and
  outbox delivery status, renders recipient-localized messages, schedules reminders.
- **granted_tools:** `channel.post`, `channel.list`, `channel.history`, `channel.edit`,
  `channel.delete`, `channel.chart_pref.get`, `channel.chart_pref.set`, `inbox.*`
  (list/record/resolve), `outbox.status`, `outbox.enqueue`, `bus.publish`, `bus.watch`,
  `message.render`, `message.render_recipient`, `reminder.*`. (Note: `channel.post/list/delete`
  authorize on `bus:chan/{cid}:pub|sub` — the persona advertises them; the bus cap remains the
  wall, unchanged.)
- **grounding_skills (pinned):** `core.channels-inbox-outbox`, `core.prefs`
  (recipient-render). Catalog: `core.jobs`, `core.lb-cli`.

### 7. `builtin.system-manager` — "in general an agent to fully manage the system"

- **identity:** the general operator; broad surface, explicitly instructed to *hand off*: for
  deep work it tells the user which focused persona fits (it is the map, not every territory).
- **extends:** `builtin.data-analyst`, `builtin.flow-author`, `builtin.widget-builder`,
  `builtin.rules-author`, `builtin.workspace-admin`, `builtin.channels-operator`.
- **granted_tools (own):** `system.*` (overview/topology/subsystem/tools/acp),
  `telemetry.read`, `telemetry.purge`, `ext.list`, `ext.disable`, `ext.uninstall`,
  `secret.set`, `secret.list`, `secret.delete`, `history.list`, `history.compensations`,
  `undo`, `redo`, `agent.runtimes`, `agent.config.get`, `agent.config.set`, `agent.def.*`,
  `agent.persona.list`, `agent.persona.get`, `prefs.*`. **Excluded:** `secret.get` (broad
  secret readback), `undo.any`, `workspace.delete/purge`, `authz.revoke-tokens` — same
  human-only posture.
- **grounding_skills (pinned):** `core.lb-cli`, `core.mcp` (#2), `core.auth-caps`,
  `core.agent`. Catalog: everything the parents carry (activate-on-demand keeps context sane —
  this persona leans hardest on filtered-catalog-over-pins).

*(The eighth, `builtin.extension-builder`, is specified in `persona-coding-scope.md` — it seeds
from the same `personas.toml` but carries its own safety posture.)*

## Non-goals

- New verbs, caps, or records (none — content only).
- Per-persona model/budget policy (definitions / `agent-close-out` B).
- Localization of labels/identities (English seed; the prefs/message pipeline can localize
  picker labels later if asked).

## Intent / approach

**Curation follows the platform's own area boundaries** — each persona is a `docs/scope/<area>`
surface plus the reads that area genuinely composes (flow-author gets `series.read` because
flows bind sources; widget-builder gets `query.run` because cells re-run queries). The two
deliberate stances: **destructive verbs are excluded from every persona** (advertising
`workspace.purge` to a model invites a catastrophic proposal; a human runs those, or a custom
persona opts in explicitly — recorded as the rejected alternative "trust the wall alone": the
wall holds, but advertisement shapes model behavior, and a persona is exactly the advertisement
layer), and **`system-manager` is extends-composed, not hand-flattened** (the union stays
current as parents evolve — rejected: a flattened list, which would rot on every new verb).

## How it fits the core

- **Tenancy / capabilities / placement:** all #1 — this slice adds records to the reserved
  seed namespace only. Every list above is advertisement; `caps::check` under
  `persona ∩ agent ∩ caller` is unchanged (re-asserted per persona in the tests).
- **Rule 10:** every id above — including any extension tool a custom persona adds (e.g.
  `mqtt.publish`) — is opaque data in `personas.toml`; no host code names a persona or a tool.
- **Data:** seed records, idempotent, version-bumped like `agents.toml`.
- **MCP / bus / secrets / WIT:** none new. **No mocks:** tests run the real seeded records.
- **Skill doc:** each persona row above lands in `skills/agent/SKILL.md`'s persona table
  (pick-by-name how-to).

## Example flow

1. Boot seeds the eight `builtin.*` personas (idempotent).
2. In Settings → Agent the picker shows them with label + description + pin-grant status; the
   admin picks `data-analyst`, grants its two ungranted skills (the #2 batch), done.
3. The user asks the dock "which sites had abnormal energy use last week?" — the run is
   grounded in `core.datasources`/`core.query`, its menu is the 20-odd data verbs instead of
   ~150 tools, and it answers via `federation.query` + `viz.query` without wandering into
   `flows.*` or the repo.
4. Next week the rules-in-flows work ships new `flows.*` verbs → `flow-author` (parent) is
   version-bumped in `personas.toml` → `rules-author` inherits on next resolve, untouched.

## Testing plan

- **Seed:** idempotent re-seed; `builtin.*` writes rejected; all eight resolve via
  `agent.persona.get` with non-empty tools + pins.
- **Per-persona menu (table-driven, real loop):** for each persona, a run's `AllowedTool` menu
  = the list above ∩ caller grants — assert one in-list tool present, one out-of-list granted
  tool absent, one excluded destructive verb absent **even for an admin caller**.
- **Capability-deny (§2.1):** `workspace-admin` persona under a *member* caller proposing
  `members.manage` → wall denies (persona never widens — the headline, per-persona).
- **Workspace-isolation (§2.2):** persona picks are per-workspace; ws-B's active
  `system-manager` never affects ws-A (re-run of #1's test over real seeds).
- **`extends` resolution:** `rules-author` menu ⊇ flow-author ∪ data-analyst ∪ own;
  `system-manager` follows a parent's change without a seed edit to itself.
- **The confusion before/after (the umbrella gate):** one task run full-surface vs under its
  matching persona; session doc records menu size + outcome.

## Implementation finding (recorded 2026-07-05, #3 session)

**The reachable menu a persona narrows is the *palette-descriptor catalog + loaded extension tools*,
NOT the full ~175-verb surface.** `reachable_tools` reads `tools.catalog`, which is
`host_descriptors()` ∩ caps — and `host_descriptors()` (`rust/crates/host/src/tools/descriptor.rs`)
is a **curated palette list** (~11 host verbs: `federation.query`, `query.*`, `agent.invoke`,
`reminder.*`, `dashboard.catalog`/`pin`, secrets) plus whatever extensions register. The vast
majority of host verbs (`rules.*`, `flows.*`, `dashboard.save`, `nav.*`, `roles.*`, `channel.*`,
`system.*`, …) are **callable** (they dispatch in `tool_call.rs`) but are **not** palette-advertised,
so a model never sees them in its menu today — with OR without a persona.

**What this means for personas (all still true, but scoped honestly):**
- A persona's `granted_tools` list is the **complete, forward-looking allow-list** — it names every
  tool that *would* be in-focus. As those verbs gain descriptors (or arrive as extension tools), the
  persona narrows them correctly with zero change. The list is not wrong; it is ahead of the palette.
- The **narrowing mechanism** (`menu = reachable ∩ persona.granted_tools`) is proven regardless of
  catalog size — a persona can only ever *shrink* the reachable set. The value is real the moment the
  reachable set is larger than the focus (extensions loaded, or the palette grows).
- The **confusion cure** has TWO levers, and this finding clarifies which does the work *today*: on a
  bare node the tool-menu is already small, so the dominant cure is the **identity + pinned grounding**
  (the agent knows who it is and reads the runbook, not the repo) — proven in #2's grounding test.
  Tool-narrowing bites hardest on a node with many extension tools loaded (the observed real symptom).
- **Follow-up (not a #3 blocker):** widening `host_descriptors()` so more host verbs are
  palette-visible is its own scope (a descriptor per verb-family) — recorded here so a future reader
  knows the persona lists are ready for it. Until then, the per-persona menu tests assert narrowing
  over the **genuinely-reachable** palette tools (a persona's in-list palette tool present, an
  out-of-list one absent), which is the honest, non-drifting assertion.

## Risks & hard problems

- **The lists are a maintenance liability** — every new verb must be triaged into personas or
  silently missing. Mitigation: the session adds a checklist line to `HOW-TO-CODE.md` ("new verb
  → which built-in personas?") and the per-persona menu test fails loudly on seed drift; accept
  the cost — curation IS the feature.
- **Destructive-verb exclusion vs "fully manage".** A user may expect `system-manager` to purge
  a workspace. The identity text says it hands those to the human; the picker description says
  so too. If real demand appears, a custom persona (or an explicit `dangerous: true` seed field)
  is the escape hatch — decide then, not now.
- **Pin weight** on `rules-author`/`system-manager` (deep extends): the 4-body cap + demote-to-
  catalog rule (#1) is the guard; tune with `agent-close-out` A's token counts.

## Open questions

1. Does `data-analyst` get `ingest.write` by default (analysts sometimes backfill) or is that a
   custom-persona opt-in? Proposal: keep it (listed above) — it's member-tier and ws-walled.
2. Picker UX for grant gaps: block activation until granted, or activate + degrade with the
   named error at run start? Proposal: picker offers the grant batch, allows activation
   regardless; the run error is honest (#1).
3. Do surfaces auto-override (Data Studio → `widget-builder`, flows canvas → `flow-author`) at
   invoke time? Deferred in #1 Q3; the seed labels are written so that mapping is obvious when
   it lands.

## Related

- `agent-personas-scope.md` (umbrella), `persona-model-scope.md` (#1 — record/apply/extends),
  `persona-grounding-scope.md` (#2 — the pinned skills), `persona-coding-scope.md` (#4 — the
  eighth persona).
- Area scopes each persona curates: `scope/datasources/`+`scope/query/` (via their skills),
  `scope/flows/`, `scope/genui/genui-scope.md`,
  `scope/frontend/dashboard/render-template-widget.md`, `scope/rules/`, `scope/nav/`,
  `scope/auth-caps/`, `scope/channels/`, `scope/inbox-outbox/`, `scope/system-map/`.
- The verb/cap oracle: `rust/role/gateway/src/session/credentials.rs` (`member_caps()`),
  `rust/crates/host/src/tools/{catalog.rs,descriptor.rs}`.
