//! The **dev-login** credential map — the ONE non-real piece of the session (collaboration scope,
//! Non-goals: "no IdP yet; the token path is real even if the credential check starts as a dev-login").
//!
//! It maps a `(user, workspace)` login request to the claim set the gateway then mints into a real
//! signed token. There is no password DB here — a real credential check / IdP plugs in *here*, behind
//! the same `mint`/`verify` seam, without touching any route. The granted caps are the full member
//! set for the collaboration surfaces (channels, members, inbox, outbox, workspace directory) so the
//! demo principal can exercise every wired verb; a narrower dev principal is built by the tests to
//! prove the deny path.

use lb_auth::{Claims, Role};

/// The capability strings a dev member is granted — every collaboration verb's gate. Channel pub/sub
/// over `*` (post/read/list/create any channel) plus the MCP verb caps the new services check.
fn member_caps() -> Vec<String> {
    [
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:members.list:call",
        "mcp:members.add:call",
        "mcp:inbox.list:call",
        "mcp:inbox.resolve:call",
        "mcp:outbox.status:call",
        // proof-workflow-sim scope: the two durable-primitive WRITE verbs a guest drives over the host
        // callback to PRODUCE motion — create an inbox item, stage an outbox effect. Member-level (the
        // author/actor is host-forced to the principal's sub; the gateway re-checks each server-side).
        // `proof.simulate` calls these inside `caller ∩ install-grant`. (These generic inbox/outbox
        // verbs also back the flow `approval` gate + `sink` outbox node — rules-workflow-convergence.)
        "mcp:inbox.record:call",
        "mcp:outbox.enqueue:call",
        // insights scope (umbrella + sub-scopes): the durable insight surface. Member-level for
        // the read/act verbs (list/get/watch/ack/resolve/occurrences) + the producer raise; sub
        // CRUD is member-owned (the owner is host-stamped from the caller); policy.get is member-
        // read, policy.set is admin (the dev login doubles as admin). The gateway re-checks each
        // cap server-side per verb (the deny-per-verb test). The matcher + ladder + digest
        // reactor are pure / reactor-driven (no cap of their own — they run under the sub's stored
        // principal + the system reactor authority).
        "mcp:insight.raise:call",
        "mcp:insight.get:call",
        "mcp:insight.list:call",
        "mcp:insight.ack:call",
        "mcp:insight.resolve:call",
        "mcp:insight.occurrences:call",
        "mcp:insight.sub.create:call",
        "mcp:insight.sub.list:call",
        "mcp:insight.sub.get:call",
        "mcp:insight.sub.delete:call",
        "mcp:insight.sub.mute:call",
        "mcp:insight.policy.get:call",
        "mcp:insight.policy.set:call",
        "mcp:workspace.list:call",
        "mcp:workspace.create:call",
        // admin-crud: the dev principal is a workspace admin so the console can exercise every
        // destructive verb. The gateway re-checks each on the server — the UI cap-gate is only a
        // convenience (admin-console scope). `workspace.purge` is the higher hard-delete ceiling.
        "mcp:workspace.delete:call",
        "mcp:workspace.purge:call",
        "mcp:user.manage:call",
        "mcp:user.disable:call",
        "mcp:teams.manage:call",
        "mcp:teams.list:call",
        "mcp:grants.assign:call",
        "mcp:grants.list:call",
        "mcp:roles.define:call",
        "mcp:roles.list:call",
        // access-console scope: the three verbs that close the access-graph gaps — resolved effective
        // caps WITH provenance (read), the live-token revoke lever, and roles.delete cascade. Admin-
        // only; the gateway re-checks each server-side.
        "mcp:authz.resolve:call",
        "mcp:authz.revoke-tokens:call",
        "mcp:roles.manage:call",
        // global-identity scope: the global identity directory + per-workspace membership roster
        // verbs. The dev admin manages identities and the workspace roster; the gateway re-checks
        // each server-side. The People tab reads `membership.list`, the switcher reads
        // `identity.workspaces`.
        "mcp:identity.manage:call",
        "mcp:members.manage:call",
        // admin-console slice 4: the extensions console lifecycle verbs, so the dev admin can list +
        // enable/disable/uninstall extensions from the browser. The gateway re-checks each on the
        // server; the UI cap-gate (showing the Extensions section) is convenience.
        "mcp:ext.list:call",
        "mcp:ext.disable:call",
        "mcp:ext.uninstall:call",
        // native-tier resilience: re-arm a sidecar's exhausted restart budget and force a fresh child
        // from the Extensions console (the Reset button), recovering a permanently-exhausted sidecar
        // without bouncing the node. Distinct from restart (bounded); the host re-checks server-side.
        "mcp:native.reset:call",
        // admin-console: publish (upload) a signed extension artifact over POST /extensions. The host
        // verb verify-before-stores; the gateway re-checks this cap server-side.
        "mcp:ext.publish:call",
        // extension SDK / Studio: local-only by deployment convention. The gateway dev node grants
        // these so the built-in Studio can scaffold/build/inspect through the universal MCP bridge.
        "mcp:devkit.templates:call",
        "mcp:devkit.scaffold:call",
        "mcp:devkit.inspect:call",
        "mcp:devkit.build:call",
        // Studio "open existing" folder picker: `devkit.root` anchors the browse at the devkit root,
        // `host.fs.list` walks it one level at a time (so the user browses to an extension instead of
        // typing a path). Read-only metadata; the host re-checks both server-side.
        "mcp:devkit.root:call",
        "mcp:host.fs.list:call",
        // Publishing a native devkit build reuses `ext.publish` plus the existing native install
        // gate before any child process is supervised.
        "mcp:native.install:call",
        // datasources scope: `federation.query`/`datasource.test` dispatch to the federation sidecar
        // via `call_sidecar`, which gates on `mcp:native.call:call`. The dev admin (which installs
        // natives) must also be able to CALL them, else the Datasources page's Test/query path 500s
        // with an opaque sidecar deny. The host re-checks; a token without it is refused.
        "mcp:native.call:call",
        // data-console (Ingest page): the S8 ingest/series verbs, surfaced over the gateway. These
        // are **member-level** — any member may explore + manually write their own series (the
        // producer is the authenticated principal, un-spoofable).
        "mcp:ingest.write:call",
        "mcp:series.read:call",
        "mcp:series.latest:call",
        "mcp:series.find:call",
        "mcp:series.list:call",
        // graphics-canvas (thecrew) live values: the SSE live-value verb the scene bridge subscribes
        // through (`bridge.watch("series.watch")` → the shipped `GET /series/{s}/stream`). Member-
        // level like the other series reads — the SSE route authorizes on `series.read` too, so this
        // cap gates the bridge dispatch; a token without it is refused. Without it a live scene falls
        // back to `series.latest` polling only.
        "mcp:series.watch:call",
        // document-store (assets.*) doc verbs — the browser's shared-doc surface, reached over the
        // `POST /mcp/call` bridge. Member-level: any member may create/read/list their OWN docs
        // (gate-3 author-ownership decides which specific doc). These are what the thecrew graphics
        // extension persists a scene through (`scene:` docs) and what the scene picker lists. Without
        // the exact per-verb cap the page bridge is refused server-side (thecrew finding 3: the member
        // set carried `store:doc/*:write`/`mcp:*.write` wildcards but NOT these verb caps, so a live
        // scene save/load 403'd). The gateway re-checks each server-side (the deny test).
        "mcp:assets.get_doc:call",
        "mcp:assets.put_doc:call",
        "mcp:assets.list_docs:call",
        // skills / core-skills scope: the skill catalog + lifecycle verbs the browser/agent reach over
        // the `POST /mcp/call` bridge. `list_skills` is the one agent-facing catalog (id+latest+
        // description+tier), `load_skill` pulls a granted body, `grant_skill`/`revoke_skill` adopt/drop
        // a skill for the workspace (admin acts), `deprecate_skill` soft-hides a user skill. None of the
        // `mcp:*.<verb>:call` wildcards below cover these single-segment verb names, so each is granted
        // explicitly. `put_skill`/`deprecate_skill` still reject the reserved `core.*` namespace regardless.
        "mcp:assets.list_skills:call",
        "mcp:assets.load_skill:call",
        "mcp:assets.put_skill:call",
        "mcp:assets.grant_skill:call",
        "mcp:assets.revoke_skill:call",
        "mcp:assets.deprecate_skill:call",
        // bus pub/sub (widget-config-vars "Platform fix") — member-level generic workspace-walled
        // motion. `bus.publish` (fire-and-forget) + `bus.watch` (subscribe). The subject is walled to
        // `ws/{id}/ext/{subject}` host-side from the token; a reserved prefix / cross-ws subject is refused.
        "mcp:bus.publish:call",
        "mcp:bus.watch:call",
        // command-palette catalog (channels-command-palette scope): the `/` palette's read. Member-
        // level — every UI-capable principal holds it; `tools.catalog` leaks only the tool *shapes*
        // the caller may already run (a denied tool is absent), never data. Without it the UI has no
        // palette at all.
        "mcp:tools.catalog:call",
        // channel query charts: a viewer's per-item plot override (get/set). Member-level — the same
        // audience that may read a channel (`bus:chan/*:sub`, held above) may save how THEY plot a
        // query result; the verb re-checks the channel `sub` gate.
        "mcp:channel.chart_pref.get:call",
        "mcp:channel.chart_pref.set:call",
        // rules-messaging scope: the channel read/write MCP surface a rule/agent/UI reaches over the
        // one contract. `post`/`list`/`delete` are already covered by the `mcp:*.<verb>:call`
        // wildcards below; `history` and `edit` have no such wildcard, so they're granted explicitly
        // here. Each verb still re-runs the `bus:chan/{cid}:{Pub|Sub}` gate (held above) inside the
        // host fn — this only opens the outer MCP door.
        "mcp:channel.history:call",
        "mcp:channel.edit:call",
        // host-callback scope: the proof-panel guest's own backend tool `proof.derive`, reachable over
        // the live `POST /mcp/call` bridge. The dev member may run it; the guest's INNER callbacks
        // (series.latest/ingest.write) authorize against `caller ∩ install-grant` — both held here.
        "mcp:proof-panel.proof.derive:call",
        // proof-workflow-sim scope: the guest sim tool the page's "Run workflow simulation" card
        // invokes over the live bridge. Its INNER callbacks (inbox.record/list/resolve,
        // outbox.enqueue/status) authorize against `caller ∩ install-grant` — all held here.
        "mcp:proof-panel.proof.simulate:call",
        // tag a series entity (member-level): the discovery edges `series.find` intersects. A member
        // may tag their own series; the test gateway's `/_seed/series` route uses this real write path.
        "mcp:tags.add:call",
        // find/enumerate over the tag graph (member-level read): the dashboard variable Query source,
        // the nav `tag-group`/`template-group` fan-out (reusable-pages scope), and any tag-driven
        // discovery. Paired with `tags.add` above — a member who may tag may also find by tag.
        "mcp:tags.find:call",
        // data-console (Data page, the DB browser): the raw-store lens verbs. **ADMIN-ONLY** by
        // decision — they relax the per-record membership gate (gate 3): a raw scan answers "every
        // record in the workspace". The dev principal is a workspace admin (it holds the destructive
        // verbs above), so it carries them; a true member role must NOT. The gateway re-checks each
        // server-side, and a deny-test asserts a token without the cap is refused (data-console risk).
        "mcp:store.tables:call",
        "mcp:store.scan:call",
        "mcp:store.graph:call",
        // widget-builder Slice A (the "direct SurrealDB" widget source): the read-only SQL verbs the
        // `/store/query` + `/store/schema` routes check, and a widget cell reaches over the bridge.
        // `store.query` is a parse-allowlisted, bounded, workspace-walled SELECT (the parse gate +
        // wall + row-cap are the boundary; the cap grant is convenience). `store.schema` feeds the
        // visual SQL builder's dropdowns. The gateway re-checks each server-side; a token without the
        // cap is refused (the deny test). Granted here like the other raw-store lenses.
        "mcp:store.query:call",
        "mcp:store.schema:call",
        // system-map scope: the two read verbs the `/system/*` routes check. Admin-only by grant
        // convention — a system snapshot reads across every subsystem of the workspace (like the
        // `store.*` lens), so the cap rides the workspace-admin role, NOT the member set. The gateway
        // re-checks server-side; a token without the cap is refused (system_verb_without_cap_denied).
        "mcp:system.overview:call",
        "mcp:system.topology:call",
        // system-map subsystem detail: the per-subsystem detail verb a no-page card (gateway/bus/mcp)
        // drills into. Admin-only by the same convention — it reads one subsystem's full live state.
        "mcp:system.subsystem:call",
        // tool-catalog scope: the reachable MCP tool catalog (host-native + extension, with
        // descriptions) behind the MCP service page, and the ACP adapter's static facts behind the ACP
        // service page. Admin-only by the same convention — the catalog reads across the workspace's
        // whole tool surface. The gateway re-checks server-side; a token without the cap is refused.
        "mcp:system.tools:call",
        "mcp:system.acp:call",
        // dashboard scope (the grid-of-widgets surface): the five `dashboard.*` verbs the dashboard
        // routes check. Member-level — any member may build/share their own dashboards over real
        // series (gate 3 / ownership still decides which *specific* dashboard they read/edit). The
        // gateway re-checks each cap server-side; a token without them is refused per verb.
        "mcp:dashboard.get:call",
        "mcp:dashboard.list:call",
        "mcp:dashboard.save:call",
        "mcp:dashboard.delete:call",
        "mcp:dashboard.share:call",
        // widget-catalog scope (Slice A): the widget palette read. Member-level — every member may
        // read the palette (it grants knowledge, not access; the write stays gated on dashboard.save).
        // LOAD-BEARING: the `mcp:*.{get,list,write,create,update,delete,post}:call` wildcards below do
        // NOT match `.catalog`, so without this line every member call is denied (same trap as
        // `tools.catalog`, individually listed above). The gateway re-checks it server-side.
        "mcp:dashboard.catalog:call",
        // widget-platform scope (Slice B): pin a tool result-render to a dashboard. Member-level — any
        // member may pin an `x-lb-render` envelope to their OWN dashboards (gate-3 owner-only-update
        // still decides which existing dashboard they may pin into). The same `.catalog` wildcard trap:
        // the wildcards below do NOT match `.pin`, so without this line every member's pin is denied. The
        // verb mints a cell host-side and persists through the Slice A validation chain; the gateway
        // re-checks this cap server-side.
        "mcp:dashboard.pin:call",
        // library-panels scope (panels as their own reusable + standalone asset): the six `panel.*`
        // verbs the panel routes check. Member-level like dashboards — any member may build/share their
        // own panels (gate 3 / ownership still decides which *specific* panel they read/edit); the
        // panel is a LENS, its `sources[]` re-checked under the viewer's caps at render. The gateway
        // re-checks each cap server-side; a token without them is refused per verb.
        "mcp:panel.get:call",
        "mcp:panel.list:call",
        "mcp:panel.save:call",
        "mcp:panel.delete:call",
        "mcp:panel.share:call",
        "mcp:panel.usage:call",
        // nav scope (the user-/team-authored navigation menu): the `nav.*` verbs the nav routes check.
        // The reads (`get`/`list`/`resolve`) are member-level — every member resolves their own menu
        // and curates their own pick (`nav.pref.*` gate on `nav.resolve`); the writes
        // (`save`/`delete`/`share`/`set_default`) are admin-ish (the dev principal is a ws-admin). The
        // nav grants NOTHING — `nav.resolve` is a pure lens over the caps above; the gateway re-checks
        // every page verb on click regardless. A token without a given verb is refused per verb.
        "mcp:nav.get:call",
        "mcp:nav.list:call",
        "mcp:nav.save:call",
        "mcp:nav.delete:call",
        "mcp:nav.share:call",
        "mcp:nav.resolve:call",
        // data-studio scope v2 (the multi-pane workbench): the member-owned per-surface ui-layout
        // record (`ui_layout:[ws, user, surface]`). Member-level — every member persists their OWN
        // workbench arrangement; the verb keys the record to the token `sub`, so this grants nothing
        // over any other user's layout.
        "mcp:layout.get:call",
        "mcp:layout.set:call",
        // viz Phase 3 (backend-resolved panel data): `viz.query(panel) -> { frames }` is THE render
        // path every panel now reads through (usePanelData). Member-level like `dashboard.get` — it
        // dispatches each target under `caller ∩ grant` (composing the target tool's own cap), so a
        // token still cannot read a target it lacks. A token without it is refused per verb.
        "mcp:viz.query:call",
        // widget-builder scope (the tool-driven widget builder): the four `template.*` verbs the
        // builder reaches over the `POST /mcp/call` bridge to persist/load durable scripted-view
        // (Plot/D3/JSX) snippets. Member-level — any member may author their own templates
        // (author-ownership decides which *specific* template they may update/delete). A token without
        // a given verb is refused server-side per verb (the deny-per-verb test).
        "mcp:template.save:call",
        "mcp:template.get:call",
        "mcp:template.list:call",
        "mcp:template.delete:call",
        // rules-workbench scope (the Playground · datasources admin): the shipped `rules.*` /
        // `datasource.*` verbs the rules/datasources gateway routes check. Member-level — any member
        // may author/run their own rules and (as admin) register datasources over real series
        // (workspace wall + per-source `caps::check` inside a run still decide what a rule may
        // actually read). The gateway re-checks each cap server-side; a token without a given verb is
        // refused per verb (the deny-per-verb test). The DAG surface is `flows.*` (chains retired —
        // chains-retirement scope).
        "mcp:rules.run:call",
        // `rules.eval` — the flow-node rule entry (message envelope in, findings out; rules-workflow-
        // convergence scope). A member authoring a `rhai`/`rule` flow node dispatches it under their own
        // token, so the member grant carries it beside `rules.run`; a token without it is refused per verb.
        "mcp:rules.eval:call",
        "mcp:rules.save:call",
        "mcp:rules.get:call",
        "mcp:rules.list:call",
        "mcp:rules.delete:call",
        // flows (flows-canvas + dashboard-binding scopes, Wave 3) — the shipped `flows.*` typed-node
        // engine verbs the flows gateway routes check. Member-level — any member may author/run their
        // own flows (workspace wall + the no-widening run gate still decide what a run may do). The
        // gateway re-checks each cap server-side; a token without a given verb is refused per verb.
        "mcp:flows.save:call",
        "mcp:flows.get:call",
        "mcp:flows.list:call",
        "mcp:flows.delete:call",
        "mcp:flows.nodes:call",
        "mcp:flows.run:call",
        "mcp:flows.resume:call",
        "mcp:flows.suspend:call",
        "mcp:flows.cancel:call",
        "mcp:flows.patch_run:call",
        "mcp:flows.runs.get:call",
        "mcp:flows.runs.list:call",
        "mcp:flows.watch:call",
        "mcp:flows.node.get:call",
        "mcp:flows.node.update:call",
        "mcp:flows.node_state:call",
        "mcp:flows.enable:call",
        "mcp:flows.inject:call",
        // prefs (user-prefs scope): reading/writing your OWN presentation settings is member-level —
        // as member-level as building your own dashboard or reading your own flow. `prefs.get`/
        // `prefs.resolve`/`prefs.set` force the target to the caller's own `sub` (structural, beyond
        // the cap — a caller can never name another user), so granting them to a member widens
        // nothing; it just lets a member RENDER their own screen (the viz layer resolves the viewer's
        // prefs to localize a timestamp/quantity via `format.datetime`/`format.quantity`, which are
        // grant-free but need the resolved axes). Without this a member could build a dashboard but
        // not resolve the prefs to format it (flow-ts-display scope). `prefs.set_default` stays
        // admin-only (NOT granted here) — it writes the WORKSPACE default, not a personal record.
        "mcp:prefs.get:call",
        "mcp:prefs.resolve:call",
        "mcp:prefs.set:call",
        // i18n catalogs (i18n-catalogs scope, prefs Phase 2). `message.render` (render a catalog
        // message for the CALLER) + `prefs.catalog` (read the merged override-over-builtin map for
        // the caller's own workspace) are member-level — a member must render/read to localize their
        // own screen, mirroring `prefs.resolve`. `message.render_recipient` is the fan-out grant the
        // outbox/inbox producer holds to render FOR ANOTHER recipient (producing content on their
        // behalf, like `prefs.get(other)`) — the dev principal carries it so the collaboration UI can
        // exercise per-recipient rendering. `message.set_catalog` writes a WORKSPACE override, an
        // ADMIN act beside `prefs.set_default`; granted here because the dev login doubles as admin.
        "mcp:message.render:call",
        "mcp:message.render_recipient:call",
        "mcp:prefs.catalog:call",
        "mcp:message.set_catalog:call",
        // telemetry console (telemetry-console scope): the read grant the Telemetry page's
        // `telemetry.query`/`trace` (and the SSE `telemetry.tail`) gate on. Member-level — the read
        // surface is HARD-filtered to the caller's workspace server-side, so a member sees only their
        // own ring (the cross-tenant operator console is a SEPARATE, higher capability, not granted
        // here). Note: `audit.query` is deliberately NOT granted, so the dev session exercises the
        // console's labelled "audit unavailable / needs-grant" lane (the scope's degraded path).
        "mcp:telemetry.read:call",
        "mcp:datasource.add:call",
        "mcp:datasource.remove:call",
        "mcp:datasource.list:call",
        "mcp:datasource.test:call",
        "mcp:federation.query:call",
        // `datasource.add` mediates the DSN into lb-secrets under the caller's authority — the host
        // requires this secret-write grant alongside `mcp:datasource.add:call` (federation/add.rs).
        // Member-level for the dev login so the datasources admin page's Add actually persists.
        "secret:federation/*:write",
        // rules-workbench: the `rules.*` verbs add a defense-in-depth Store-surface check
        // (`store:rule:*`) BELOW the MCP gate — unlike dashboard, which gates on MCP + the S4 edges
        // only. The dev member needs the store grants so the Playground save/get/list/delete actually
        // persist over the live gateway (mirrors `store:doc/*` above). The DAG engine is `flows.*`,
        // whose store surface is `store:flow:*` (granted with the flows caps above).
        "store:rule:read",
        "store:rule:write",
        // agent-run scope Part 2: the per-tool-call human gate. `agent.decide` first-settles a
        // suspended tool call (member-level — the same authority that resolves the surfaced inbox
        // item). `agent.policy.set` edits the ws Allow/Deny/Ask policy — an ADMIN act (who-may-run-
        // what), so it rides the workspace-admin role the dev principal already holds, NOT the bare
        // member set. The gateway re-checks each cap server-side (a token without it is refused).
        "mcp:agent.decide:call",
        "mcp:agent.policy.set:call",
        // channels-agent + run-lifecycle #5: the in-channel agent. `mcp:agent.invoke:call` is the run's
        // own gate — it also makes the `/agent.invoke` command APPEAR in the `/` palette catalog (the
        // catalog gates each tool on `authorize_tool(principal, ws, <name>)`; naming the descriptor
        // `agent.invoke` reuses that gate with zero special-casing). A member with it sees + can run the
        // agent; one without simply doesn't see the command (absent, not greyed). `mcp:agent.runtimes:call`
        // is a DISTINCT read cap for the runtime-picker dropdown (`agent.runtimes` — list-only, no
        // mutation), so the picker loads for a normal member. Both member-level; the host re-checks each.
        "mcp:agent.invoke:call",
        "mcp:agent.runtimes:call",
        // agent-config scope: the per-workspace default-runtime + model-endpoint record. `get` is
        // member-level (a member reads it to render the Settings/Agent surface and to know which
        // runtime an invoke will use); `set` writes the WORKSPACE default — an ADMIN act beside
        // `prefs.set_default`/`agent.policy.set`, granted here because the dev login doubles as admin.
        // The host re-checks each cap server-side (a token without `set` is refused).
        "mcp:agent.config.get:call",
        "mcp:agent.config.set:call",
        // agent-catalog scope: the definition catalog. `list`/`get` are member-level (the picker
        // reads them); create/update/delete are admin (custom definitions only — built-ins are
        // read-only regardless of caps). The dev login doubles as admin, so it holds all five; the
        // host re-checks each server-side.
        "mcp:agent.def.list:call",
        "mcp:agent.def.get:call",
        "mcp:agent.def.create:call",
        "mcp:agent.def.update:call",
        "mcp:agent.def.delete:call",
        // agent-personas scope #1: the persona catalog. `list`/`get` are member-level (the Settings
        // picker + the run-assembly resolver read them); create/update/delete are admin (custom
        // personas only — built-ins are read-only regardless of caps). The dev login doubles as admin,
        // so it holds all five; the host re-checks each server-side. None of the `mcp:*.<verb>:call`
        // wildcards below cover the two-segment `agent.persona.<verb>` names, so each is listed.
        "mcp:agent.persona.list:call",
        "mcp:agent.persona.get:call",
        // agent-personas #1 Settings surface: `resolve` returns the extends-unioned effective persona
        // for the read-only "effective tools" view (member-level read); `agent.policy.get` reads the
        // Allow/Ask/Deny policy so the pane can round-trip it (member read; `set` stays admin, above).
        "mcp:agent.persona.resolve:call",
        "mcp:agent.policy.get:call",
        "mcp:agent.persona.create:call",
        "mcp:agent.persona.update:call",
        "mcp:agent.persona.delete:call",
        // agent-catalog test-and-secrets scope: the context-proving diagnostic. Its OWN admin-tier cap
        // (distinct from the read-ish `agent.def.list`) because the test SPENDS model budget — "who may
        // spend model budget" is a distinct authority. The dev login doubles as admin; the host
        // re-checks it server-side. Setting a sealed MODEL KEY for a definition rides the shipped
        // secrets surface — `mcp:secret.set:call` + `secret:agent/*:write` below (the key value flows
        // ONLY through `secret.set`, sealed; the record stores just the path, names-only).
        "mcp:agent.def.test:call",
        "mcp:secret.set:call",
        "secret:agent/*:write",
        // agent-memory scope: the durable memory verbs. `list`/`get` are member-level reads; `set`/
        // `delete` are member-level writes on the caller's OWN scope (the target scope is derived from
        // the principal, never an argument — the member wall). A write to the SHARED `workspace` scope
        // ALSO needs the distinct `store:agent_memory/workspace:write` cap below, so an admin decides
        // whether every member's agent may write shared memory. The dev login doubles as admin, so it
        // holds it. The host re-checks each server-side (a token without a verb is refused).
        "mcp:agent.memory.list:call",
        "mcp:agent.memory.get:call",
        "mcp:agent.memory.set:call",
        "mcp:agent.memory.delete:call",
        "store:agent_memory/workspace:write",
        // reminders-tenant scope: `reminder.fire` is the gated run-now verb (a "fire now" control).
        // Member-level — the same authority that creates/updates a reminder may fire one now; the
        // firing still re-checks the ACTION's own cap under the stored principal (no escalation).
        // Granted explicitly here because the `mcp:*.<verb>:call` wildcards below do NOT cover `fire`
        // (there is no `mcp:*.fire:call`), so without this line the run-now control would be denied.
        "mcp:reminder.fire:call",
        // agent-run scope Part 3: `agent.watch` gates the live `RunEvent` SSE feed (`GET
        // /runs/{job}/stream`). Read-only on the run; checked inside `watch_run` (a `403` before any
        // stream body). Member-level — observing a run is not an admin act.
        "mcp:agent.watch:call",
        // agent-dock run controls: `agent.control` gates STOP / PAUSE / RESUME on a run (`POST
        // /runs/{job}/{cancel|pause|resume}`), checked inside `stop_run`/`pause_run`/`resume_run`
        // (opaque `403` on deny). DISTINCT from `agent.watch` — watching a run never implies authority
        // to control it. Member-level: a member driving a run may stop/pause/resume their own run
        // (the workspace wall still isolates it; a ws-B caller can't reach a ws-A run).
        "mcp:agent.control:call",
        // agent-run scope Part 5: model-activated skills. `skill.activate` is a LOOP-INTERNAL tool
        // (the loop intercepts the model's proposed call and loads the body under the S4 grant gate),
        // so the dev principal does not strictly need this to drive the loop. It is granted for the
        // catalog/activation surface symmetry and so a future direct-MCP route is reachable; the S4
        // skill GRANT remains the real wall (an ungranted skill is denied regardless of this cap).
        "mcp:skill.activate:call",
        // files/skills scope: the shared-asset surface caps the doc/skill routes check directly
        // (`authorize_doc`/`authorize_skill` gate on `store:doc/{id}` / `store:skill/{id}`, NOT an
        // MCP verb). The dev member may put/get/share/link their docs and manage skills; gate 3
        // (membership/ownership) still decides which *specific* asset they may read. `add_team_member`
        // is gated by `store:doc/*:write` (an admin act at S4), so the dev admin can populate teams.
        "store:doc/*:read",
        "store:doc/*:write",
        // Skill surface: `**` (recursive tail), NOT `*`. A core skill id contains a `.`
        // (`core.lb-cli`), and the caps grammar splits a resource on BOTH `/` and `.`, so `skill/*`
        // (one segment) would NOT cover `skill/core.lb-cli` — a real admin could neither grant nor
        // load a core skill (core-skills scope). `skill/**` covers dotted core ids AND flat user ids.
        "store:skill/**:read",
        "store:skill/**:write",
        // api-keys scope: the management verb gate, plus the built-in role cap bundles. The dev admin
        // HOLDS the read-only/read-write cap sets so the no-widening guard lets it mint keys under
        // either built-in role (a key created by this admin never widens beyond it). The write role's
        // caps are action-named (not `*.*`) so a data key can never reach `apikey.manage`.
        "mcp:apikey.manage:call",
        // webhooks scope: the management verb gate + the secret-write cap `signature` mode needs
        // to store its shared secret in `lb-secrets`. The dev admin HOLDS `mcp:ingest.write:call`
        // (the cap a webhook's inbound principal resolves to) so the no-widening guard lets it mint
        // hooks under either mode. `secret:webhook/*:write` is re-checked by `lb_secrets::set_with`
        // during create/rotate (the same gate `bearer` mode's linked apikey path traverses via
        // `apikey_create`'s own grants). The public `POST /hooks/{ws}/{id}` route does NOT take a
        // session token — these caps gate the ADMIN surface (`/admin/webhooks/*`) only.
        "mcp:webhook.manage:call",
        "mcp:ingest.write:call",
        "secret:webhook/*:write",
        "store:*:read",
        "store:*:write",
        "mcp:*.get:call",
        "mcp:*.list:call",
        "mcp:*.write:call",
        "mcp:*.create:call",
        "mcp:*.update:call",
        "mcp:*.delete:call",
        "mcp:*.post:call",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Build the claim set for `user` logging in to `workspace`, valid for `ttl` seconds from `now`.
/// Real signed claims — only the *credential check* (here, "any user, any workspace") is the
/// dev-login stand-in. The workspace becomes the token's hard wall (§7).
pub fn dev_claims(user: &str, workspace: &str, now: u64, ttl: u64) -> Claims {
    Claims {
        sub: user.to_string(),
        ws: workspace.to_string(),
        role: Role::Member,
        caps: member_caps(),
        iat: now,
        exp: now.saturating_add(ttl),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Datasources page drives `federation.query`/`datasource.test`, which dispatch to the
    /// federation sidecar via `call_sidecar` (gated `mcp:native.call:call`) after mediating the DSN
    /// (`secret:federation/*:write` for Add). A dev login missing ANY of these silently 500s the live
    /// page even though the route-level cap is present — the end-to-end gap this guards against.
    /// widget-catalog scope (Slice A): `dashboard.catalog` is a member-level read, but the
    /// `mcp:*.<verb>:call` wildcards do NOT cover `.catalog` (there is no `mcp:*.catalog:call`), so a
    /// member token gets the cap ONLY if it is individually listed. Without it every member's palette
    /// read is denied — the verb is dead on arrival. This guards the load-bearing grant line (the same
    /// trap `tools.catalog` sits behind, asserted alongside it here).
    /// widget-platform scope (Slice B): `dashboard.pin` is a member-level write with the SAME trap — the
    /// `.pin` suffix is not matched by the `mcp:*.{get,list,write,create,update,delete,post}:call`
    /// wildcards, so without this line every member's pin is denied. Asserted alongside the catalog cap.
    #[test]
    fn dev_login_carries_the_widget_catalog_read() {
        let caps = member_caps();
        for needed in [
            "mcp:dashboard.catalog:call",
            "mcp:dashboard.pin:call",
            "mcp:tools.catalog:call",
        ] {
            assert!(
                caps.iter().any(|c| c == needed),
                "member set must grant {needed} — the `.catalog`/`.pin` wildcards don't cover it"
            );
        }
    }

    #[test]
    fn dev_login_carries_the_full_datasources_chain() {
        let caps = member_caps();
        for needed in [
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
            "mcp:federation.query:call",
            "mcp:native.call:call",
            "secret:federation/*:write",
        ] {
            assert!(
                caps.iter().any(|c| c == needed),
                "dev login must grant {needed} for the Datasources page to work end to end"
            );
        }
    }
}
