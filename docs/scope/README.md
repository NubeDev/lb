# Scope docs

Pre-work briefs: the *ask* for each feature area, written before implementation (see
`../SCOPE-WRITTING.md`). One topic folder per area; one `<name>-scope.md` per ask within it.
A feature reads top-to-bottom across folders: `scope/<topic>/` → `sessions/<topic>/` →
`public/<topic>/`.

## Topics

- `agent/` — the central, workspace-scoped AI agent (S5). `default-agent-wiring-scope.md` finishes
  it: wire a real model into the in-house `default` runtime, route the loop's tool calls through the
  one host MCP bridge so it can call platform tools (`agent.memory.*`/`assets.*`/…) under the wall —
  a fix that also lets the external agent reach host tools — surface the caller's reachable tools to
  the loop, and boot `serve_agent`. Closes the "internal agent has no brain and can't use tools" gap.
  `agent-catalog-scope.md` adds a **manageable catalog of named agent definitions** — `(runtime, model
  endpoint)` presets in two tiers (read-only **built-ins** boot-seeded from a TOML manifest into a
  reserved namespace, the core-skills pattern; workspace-authored **custom** with full CRUD) — shipping
  the in-house **default** + **Open Interpreter** over Z.AI **GLM-4.6/5.1/5.2** by default, plus the
  Settings → Agent catalog manager UI. The library the shipped one-selection `agent.config` picks from.
  **Shipped** (`agent.def.*` verbs, `_lb_agents` seed, `/agent/defs*` routes, the catalog UI;
  `crates/host/src/agent/defs/`) — picking sets the workspace default runtime today, per-workspace
  endpoint consumption is the named follow-up (ai-gateway provider adapter).
  `agent-catalog-test-and-secrets-scope.md` adds a `agent.def.test` **"does it have MCP/ACP/skills
  context" button** (a one-turn invoke with the real run context assembled) + a **DB-sealed per-workspace
  model key** (`lb-secrets` path on the endpoint, resolved secret→env at model-call time; the record
  stays names-only). `active-agent-wiring-scope.md` makes the catalog's active pick the **one implicit
  agent everywhere** — the minimal OpenAI-compatible `Provider` adapter (the root unblock; the in-house
  runtime stops being a de-facto mock), per-workspace model resolution off the active definition
  (consumed by rules' `ai.complete` and the in-house loop), the channel composer no longer auto-sending
  an explicit `runtime:"default"` that outranks the pick, and the missing `POST /agent/invoke`
  transport so the dashboard AI widget's `agent_invoke` stops being an unknown command.
- `agent-memory/` — durable, access-walled **agent memory** in the MEMORY.md shape: per-fact
  `agent_memory` records (`workspace` + `member:{user}` scopes) with a **derived** compact index
  injected at session start, read/written over caps-checked `agent.memory.*` verbs under the
  derived principal — so an agent remembers/recalls only what its invoking user may see. The
  "learned" half of making the agent smarter (the "taught" half is `skills/core-skills-scope.md`).
- `external-agent/` — a **compile-time-optional** (`external-agent` cargo feature, off by default),
  **swappable** runtime that drives a third-party ACP agent (VT Code default, dirge alternate; any
  [Agent-Client-Protocol](https://agentclientprotocol.com) agent) as a **subprocess** behind a
  host-owned `AgentRuntime` trait. The inverse of `agent-run/` (which makes us an ACP *server*): here
  we are the ACP *client*. The agent's only tools are our caps-checked MCP surface (built-ins off + an
  OS sandbox, fail-closed); its models route through our gateway. Built once against the official Rust
  SDK, so the whole ACP registry is pluggable by config; the default runtime stays the in-house loop.
- `agent-run/` — the agent **run** as a first-class object: a canonical `RunEvent` stream, an ACP
  stdio adapter (Zed/Cursor drive the agent), per-tool-call Allow/Deny/Ask with **durable
  suspend/resume**, and **model-activated skills** (the model picks from a granted catalog). Opens up
  the S5 black-box loop; ideas reviewed from the Awaken framework, its plugin framework rejected.
- `ai-gateway/` — the swappable model-access sidecar (S5).
- `observability/`, `audit/`, `undo/` — the **three cross-cutting projections of the host dispatch
  chokepoint** (README §6.5/§6.6), scoped together as the S10 retrofit: `observability/` (structured
  logs + distributed traces + metrics, emitted everywhere with a `trace_id` that survives the routed
  hop — `observability-scope.md` is the **emit** half; `telemetry-console-scope.md` is the **consumer**
  half: a FIFO-capped SurrealDB sink (reusable `lb-store::capped` ring), a gated workspace-walled
  `telemetry.query`/`tail` read surface, and an in-browser console with first-class filters that also
  reads the `audit/` ledger lane — the self-contained, no-external-Grafana-required view), `audit/` (an
  immutable, hash-chained, workspace-walled ledger of every allow/deny — generalizes
  §6.14's model-call audit), and `undo/` (a reversible-command journal whose hard line is *reverse
  state, compensate motion*). See "The shared seam" in `observability/observability-scope.md`.
- `auth-caps/` — the capability grammar, token, and grant delegation; plus `edge-trust-scope.md` (node
  enrollment/cert + mTLS + token-on-the-bus), `authz-grants-scope.md` (durable roles/grants/teams —
  restricted user/team access), `admin-crud-scope.md` (the destructive half — workspace/user/team/
  member delete·disable·remove·rename + dev-store user CRUD), and `api-keys-scope.md` (machine
  principals — appliance/cli/api/agent keys as a non-human `Subject` over the same grant model,
  a hashed bearer secret verified per request for instant revoke, lazy expiry, and an admin tab), and
  `access-console-scope.md` (the **Access console** — the access-first evolution of the `/admin` UI:
  an overview of who-can-do-what, resolved effective caps per subject with provenance, a guided
  no-widening capability picker, a force-re-mint/end-sessions lever for the freshness asymmetry, and
  `roles.delete` — closes the `resolve_caps`/`invalidate`/`roles.delete` backend gaps; not a new page),
  and `global-identity-scope.md` (the **Slack model** the README §7/§6.6 name but the code never built:
  a global, hub-authoritative identity in a system directory linked to workspaces by a `membership`
  record, login resolving to a person's workspaces, and a real workspace switcher — promotes the
  stated design to implementation; the gap surfaced in the access-console session).
- `bus/` — the Zenoh message bus (motion).
- `coding-workflow/` — the S6 worked example: issue → triage → approval → job → outbox.
- `rules/` — the embedded **rules/processing engine** (`lb-rules`), ported from `rubix-cube`: a
  sandboxed `rhai` cage + a lazy `Grid` + a verb library (`rules-engine-scope.md`, data via
  `data.query`/`series.*`/`federation.query`, `ai.*` via the AI-gateway, `emit`/`alert` via inbox/outbox).
  Exposed as `rules.*` MCP verbs. `rules-ai-wiring-scope.md` closes the **`ai.*`-not-hooked-up** gap: the
  data half is wired, but the production `rules.run` bridge hardcodes a `DisabledModel` — the scope binds
  the rule engine's model seam to the real agent (`RuleModel` over `ModelAccess`), resolving the
  workspace's selected model from the agent-catalog `agent.config` pick. **Chaining rules into a DAG is now `flows/`** — the standalone
  `chains.*` surface is retired (`flows/chains-retirement-scope.md`); `rule-chains-scope.md` stays as
  lineage (the `rubix-cube` workflow-DAG port history), not a shipping surface.
- `datasources/` — the native (Tier-2) **`federation` extension** (`datasources-scope.md`): embeds
  DataFusion + connectors to query external SQL sources (MySQL, PostgreSQL/TimescaleDB, …) under `net:*`
  + a mediated secret, surfaced as the read-first, workspace-pinned `federation.query` MCP verb (+
  `datasource.*` admin CRUD and a `federation.mirror` `lb-jobs` batch). SurrealDB stays the authority;
  external DBs are federated sources reached through the gated extension, never a second authority.
  Also **`page-chaining-scope.md`** (parent) + its slices: one **keyset cursor** paging contract (opaque
  `cursor` + `limit` → `{rows, next_cursor}`, additive on the existing read verbs, no new cap) so large
  timeseries load a fast page at a time — **SurrealDB pages the state plane** (index-backed, O(page)),
  DataFusion pages only by predicate **pushdown** to the real source, and anything that must load at
  dashboard speed is **mirrored** into the series plane and keyset-paged there; a chart **downsamples**
  (time-bucket min/max/avg) rather than paging raw points. Offset paging and DataFusion-as-primary-pager
  rejected. Decomposed into `page-cursor-scope.md` (A: the cursor codec + keyset primitive),
  `series-paging-scope.md` (B: native `series.read` rows fast path), `series-decimation-scope.md`
  (C: chart bucket downsampling), `federation-paging-scope.md` (D: external pushdown + mirror routing),
  and `page-chaining-ui-scope.md` (E: the data-console table + dashboard viz callers).
- `control-engine/` — the native (Tier-2) **`control-engine` extension** (scope co-located with the
  extension at `rust/extensions/control-engine/docs/control-engine-scope.md` — it is **100% an
  extension**, so its docs live with it; the core stays CE-ignorant, CI-enforced):
  bridges Control Engine (CE) instances into a workspace as a caps-gated MCP surface (`ce.*`, mirroring
  the `ce-client-rust` `ControlEngine` trait) — a local CE over `localhost` REST/WS, and remote CEs on
  **appliance** LB nodes reached by **routed MCP over Zenoh** (symmetric nodes, no CE-on-Zenoh codec).
  The visual editor is the vendored `@nube/ce-wiresheet` package, mounted as the extension's federated
  `[ui]` page and re-pointed onto the MCP bridge (browser→CE only through the host). MCP-only so agents
  and the CLI drive CE identically to the UI. Live COV rides the generic `extensions/extension-watch-scope.md`
  primitive (`ce.watch`), the only — and generic — core addition it implies.
- `core/`, `crate-layout/`, `extensions/`, `mcp/`, `node-roles/`, `registry/`, `secrets/`,
  `store/`, `tags/`, `tenancy/` — the spine and platform surfaces. `core/` also holds
  `resource-verbs-scope.md` (the **cross-cutting verb convention**: `<resource>.list|get|create|update|delete|watch`
  + a runnable `.start|stop|status|restart|logs` trait, so reminders/jobs/flows/extensions/channels/agent-runs
  all speak one grammar the palette and `lb` CLI render mechanically; renames the outliers
  `channel_list → channel.list`, `installed → extension.list` behind a one-release alias). `extensions/` also holds
  `lifecycle-management-scope.md` (the full start·stop·enable·disable·upload·install·delete lifecycle
  exposed over the gateway, not Tauri-only) and `ui-federation-scope.md` (mount an extension's OWN
  pages inside the shell — module federation for trusted publishers, iframe/Web Component sandbox for
  untrusted, host-mediated MCP bridge; the deferred counterpart to the admin console), and
  `proof-panel-scope.md` (one self-contained **Tier-1 WASM** reference extension — a real MCP tool +
  a federated page reading real series through the bridge — proving the basics end-to-end with no
  placeholders; the wasm sibling of the native `fleet-monitor`), and `host-callback-scope.md` (the
  **forever-ABI** addition that lets a WASM **guest** call host MCP tools — inbox/outbox/db/other tools —
  under its delegated `caller ∩ grant` authority, the symmetric backend dual of the page bridge; without
  it a guest is a one-way box that can't reach the platform), and `reference-extensions-scope.md`
  (five **native-first** reference extensions — markdown doc-store+PDF, todo, MQTT bridge, Timescale
  connector, Zenoh appliance gateway — plus the four platform fixes they need: the **native**
  host-callback transport, a **`net:*`** capability family for owned external sockets/DB/mesh, a generic
  per-extension **`kv.*`** store, and a binary-blob asset path; the doctrine that a native Tier-2
  extension is the sanctioned escape hatch that may own external resources without breaking rule 2),
  and `ext-sdk-scope.md` (the **extension SDK** — `lb-devkit` + `devkit.*` MCP verbs + a built-in
  Extension Studio wizard that **generate** a fresh extension (wasm|native backend + shadcn/Tailwind
  federated page), **build** a folder via the local toolchain as a durable job with a live log stream, and
  **publish** it through the unchanged signed-`Artifact` path; build is a gated **local-only** capability
  behind one `Toolchain` trait), and `extension-watch-scope.md` (the **generic live-feed primitive for
  extensions**: an extension marks a `[[tools]] kind="watch"` and the host relays it as SSE over a
  host-allocated workspace subject — closing the asymmetry where only core tools could stream; the WIT
  ABI stays frozen, streaming rides the bus, and the routed cross-node relay is free; `control-engine`'s
  `ce.watch` is its first tenant).
- `flows/` — the visual **node-graph flow engine** (`flows-scope.md`), the **one DAG engine** (the
  earlier `chains` engine is retired — `flows/chains-retirement-scope.md`): a node-red-style canvas
  over the shipped plane, not a new engine. Promotes the `rubix-cube` rule-DAG step into a typed
  `Node` (`Trigger | Tool | Rhai | Subflow | Sink`), each carrying a **descriptor** (ports + a
  config **JSON-Schema** the React Flow editor renders a form from). **Extensions contribute
  backend node types** via an additive `[[node]]` block in `extension.toml` — **identical for
  WASM and native**, executed through the existing `caller ∩ install-grant` callback (an `mqtt`
  extension ships an "MQTT publish" node). A run is a durable `lb-jobs` `flow-run` job
  (suspend/resume = pause→edit-downstream→resume); triggers `manual|cron|event|inject|boot`;
  enable/disable + `start_on_boot` + `placement`; **dashboard↔flow** binding (`flows.inject` in,
  bus-subject out); shared via the grant model; graph edits undo for free. Rejected adopting
  `open-rmf/crossflow` (in-process Bevy-ECS state breaks rules 1/4, bypasses the cap wall).
- `files/`, `skills/`, `document-store/` — shared workspace assets (S4). `skills/` also holds
  `core-skills-scope.md`: the **two-tier skill catalog** — developer-authored **core skills**
  (the `docs/skills/*/SKILL.md` corpus, embedded in the node and seeded at boot as immutable
  `skill:core.<name>@<node-version>` records, user-write-rejected) alongside the shipped
  user tier (full CRUD incl. a new `assets.deprecate_skill`), both behind the same grant gate,
  surfaced to agents as one granted catalog (name+description in context, bodies on demand). `document-store/` now
  holds `document-store-scope.md` (the ask): a **reusable markdown document store** on the shipped
  S4 asset/file substrate — markdown body + **images/attachments** (the SurrealDB file store §6.12
  finally lands) + a queryable **link graph** (doc→doc links, doc→asset embeds), shared to a
  **user/team/workspace**, undo-journaled save, CRUD over the additive `assets.*` verbs, **reusable
  by extensions** (host-callback ABI) and the doc-site authoring side. Public/anonymous serving is a
  deferred slice with its own threat model.
- `host-tools/` — built-in, cross-platform `host.*` MCP introspection verbs for facts about the node a
  call runs on: **networking** (`host.net.info`/`host.net.reach`), **timezone** (`host.time.now`/
  `host.time.zones`), **files** (`host.fs.stat`/`host.fs.list` — node-filesystem **metadata**, *not*
  the workspace doc assets in `files/`). Read-only, one cap per verb, no shell-out; OS differences
  isolated behind a per-folder `platform`/`path` seam so the verb files carry no `cfg!(windows)`.
- `git-sync/` — periodic, **AI-free** auto-commit-and-push (`autocommit-scope.md`): a `reminder`
  cron (`Action::McpTool`) fires a `git-sync` `lb-jobs` job that calls a new `git.*` MCP verb family
  (`git.commit_push`/`git.status`) backed by a ported `lb-gh` crate (the `gh`/`git` CLI wrapper).
  `systemd` supervises the node so the reactor ticks — it is **not** the scheduler (the cron is a
  record, symmetric edge↔cloud). The folder-of-verbs sibling of `host-tools/` for CLI-backed tools.
- `genui/` — **agent-authored generative UI** (`genui-scope.md`): one reusable `@nube/genui` package —
  a versioned, A2UI-*shaped* IR (surfaces, flat id-referenced component maps, JSON-Pointer data
  bindings, typed action events; Google A2UI v0.9 patterns adopted, dependency rejected) rendered by
  our own shell-token-themed catalog, with **emission formats as authoring-time adapters** (Thesys
  OpenUI Lang via `@openuidev/lang-core` in v1; an A2UI adapter deferred until a real consumer speaks
  it) — the agent's output is **parsed/normalized once at accept and the typed IR is what persists**,
  so the render path carries no parser. The `view:"genui"` **dashboard widget** is the first tenant:
  the workspace agent designs a widget from a prompt (skill-guided data discovery over
  `flows.*`/`store.query`/`series.*`), streams a live preview over the shipped RunEvent SSE, and
  persists a normal v2/v3 cell whose steady-state data flows through the existing `sources[]`
  bindings — the agent authors, it never serves. Sandboxed-view tier per
  `channels/channels-rich-responses-scope.md` (which reserves the channel as second tenant), with a
  concrete in-process promotion checklist; zero new verbs/caps/tables.
- `workspace/` — the workspace session boundary plus the node-level workspace directory and admin
  lifecycle: list/create in the switcher, archive/rename/purge in admin, with workspace data always
  selected from the signed token.
- `channels/` — the collaboration channel surface: durable inbox-backed history, bus motion, channel
  registry, SSE stream, and presence. Also `channels-query-charts-scope.md`: in-channel SQL queries
  (via `federation.query`) whose results post back as durable items and auto-plot a chart; and
  `channels-command-palette-scope.md`: the `/` + `@` command surface (catalog-driven, capability-
  filtered MCP tools — the menu *is* the permission model) that composes those queries; and
  `channels-agent-scope.md`: ask an agent in a channel — a host worker spawns a durable agent **run**
  (via the shipped `agent.invoke`/`AgentRuntime` seam), streams its work live over the agent-run SSE,
  and posts the final answer back as a durable item (in-house runtime now, external once #3 ships).
  And `channels-rich-responses-scope.md`: a command/tool/agent answers with a **rich, typed response**
  (chart/table/stat/form/control, or an AI-generated sandboxed UI) by reusing the **shipped v2 widget
  contract** un-gridded onto the channel — the `render:{view,source|data,options}` cell shape mounted
  through the dashboard's `WidgetView`/`views/*` renderers + host-mediated bridge, leashed to the viewer's
  grant. Generative UI (JSX `template`, future A2UI/JSON-render) is one more sandboxed `view`, not a base
  layer; forms/wizards are the palette arg-rail over a versioned `x-lb` widget enum.
- `inbox-outbox/` — the normalized inbox (S2) and the transactional must-deliver **outbox**
  (`outbox-scope.md`, the S6 driver).
- `ingest/` — a generic buffered read/write surface for high-volume external data; the cloud-side
  ingest buffer (the read-side analog of the outbox). Stays domain-free — IoT is one caller (S9).
- `ros/` — the native (Tier-2) **`ros` driver extension** — it is **100% an extension**, so ALL of
  its docs live with it (nothing in this central tree beyond this pointer), exactly like
  `control-engine`. Authoritative scope: `rust/extensions/ros/docs/ros-scope.md`. Manages a fleet of
  ROS (Rubix) REST
  appliances as caps-gated resources — CRUD over the `connection → network → device → point` tree
  (`ros|network|device|point.list|get|create|update|delete`), a **reusable poller**
  (`Poller/Source/Sink`) that appends point present-values to `series` via `ingest.write` with poll
  enable/disable AND-gated at every tree level, and a must-deliver `point.write` staged through the
  outbox — plus a federated shadcn/Tailwind-v4 page. The canonical "IoT is one caller" bridge that
  keeps ROS vocabulary out of core (vendors `rust-ros`, ported to async).
- `jobs/` — the SurrealDB-native durable job queue / resumable session (S5). Also
  `job-control-scope.md` (the **observe/control surface** — `job.list|get|cancel|retry|watch`,
  owner-routed through the owning service's chokepoint so callers can see/stop/recover durable work
  without a raw `jobs.*` table API; the runnable-trait member of `core/resource-verbs-scope.md`).
- `reminders/` — a durable, workspace-scoped **scheduled trigger that fires an action**
  (`reminders-scope.md`): a `reminder:{id}` record with a cron schedule + optional `max_runs` +
  `enabled` switch, fired by a `react_to_reminders` durable scan (the same altitude as the S6
  relay/approval reactors) that enqueues one `lb-jobs` job per firing. Three v1 action kinds —
  **channel post** (inbox), **MCP tool call** (any capability, under a stored principal re-checked
  at fire time), and **must-deliver effect** (outbox). Cron is the storage format; the UI authors
  it with a best-in-class React cron-builder. The single-action sibling of a rule chain.
  Also `reminders-rich-responses-scope.md`: reminders as the **first tenant** of the channel
  rich-responses contract — `/remind` is a backend-declared form (cron-builder + action `select`) that
  calls `reminder.create`, and `/reminders` is an interactive `render:{view:"table", source:reminder.list}`
  response with per-row pause/run-now/delete controls, all rendered by the shipped widget views over the
  viewer-grant-leashed bridge (no reminders-specific channel UI); adds two `x-lb` widgets (`cron`, static
  `select`) and a small `reminder.fire` run-now verb.
- `prefs/` — per-(workspace,user) preferences + localization: language (en/es), timezone, date/number
  display style, and a backend unit-conversion layer (metric/imperial). Canonical data in, localized
  presentation out, exposed as `format.*`/`convert.*` MCP tools so thin clients don't re-implement it.
  Phase 2 (MF1 message catalogs + per-recipient server-side localization) is scoped in
  `i18n-catalogs-scope.md`.
- `nav/` — the **nav builder** (`nav-builder-scope.md`): user-/team-authored navigation menus. A `nav`
  is a workspace asset cloned from the `dashboard` pattern (slug id, `owner`, `visibility`, ordered
  `items[]`), shared to teams via the shipped `share` edges; entries link to a **dashboard page**, a
  **system surface** (channels/rules/…), an **extension page** (opaque id, rule 10), or a **dynamic
  tag-group** (dashboards matching a tag facet). `nav.resolve` returns the caller's effective menu —
  pick + tag-expand + **cap-strip** — the menu is a **lens over existing access, never a grant path**;
  the sidebar (`NavRail`) renders it, falling back to the built-in `SURFACES` set.
- `query/` — saved **PRQL** queries (`prql-query-scope.md`): author once in PRQL (or `lang:"raw"`),
  **save as an editable `query:{ws}:{id}` record**, and run against the SurrealDB-native store
  (`store.query`) or a registered datasource (`federation.query`) through one `query.*` MCP family.
  A pure `lb-prql` crate wraps `prqlc`; `query.run` composes the target's existing capability (no
  widening); a rule reuses a saved query via `source("query:<name>")`. No new engine, no second
  authority — PRQL is the authoring layer, SurrealDB stays the one datastore.
- `sync/` — multi-node sync + authority (S3).
- `system-map/` — a framework-level **workspace topology + status console**: two admin-gated read
  verbs (`system.overview` status grid · `system.topology` react-flow wiring) that derive a live,
  workspace-scoped health snapshot of every subsystem (gateway·bus·mcp·store·ingest·inbox·outbox·jobs·
  extensions·registry) from the booted `Node`'s handles + the store, holding nothing durable. The
  **read/visualization** complement of `observability/` (which *emits* telemetry); the `dbview`-shaped
  observer that — unlike an extension — can see the runtime that supervises extensions.
- `cli/` — the **operator CLI** (`operator-cli-scope.md`): `lb`, the terminal twin of the React shell —
  a fourth client (besides browser/Tauri/mobile) of the gateway surface, holding no authority of its own.
  Two modes mirroring symmetric nodes: **remote** (over the gateway, the browser path) and **local**
  (`lb local …` embeds the host, the offline/solo posture), both funneling through the one
  `lb_host::call_tool` chokepoint. A universal `lb call <tool> <json>` escape hatch over `POST /mcp/call`
  plus typed commands for the common operator verbs (`ws`/`members`/`channels`/`inbox`/`outbox`/`ext`/
  `registry`/`system`/`agent`/`store`/`tags`), tables + `-o json`, the workspace/principal header always
  legible, denies surfaced honestly. It is only ever as authorized as the token it presents. v1 auth = the dev-login token; it is the **named first consumer** of
  `auth-caps/api-keys-scope.md` when API keys ship. Adds **no new MCP verbs, capabilities, or tables**;
  retires the `curl + jq` publish flow and folds `lb-pack` into `lb devkit sign` over the `lb-devkit` lib.
- `frontend/` — the React/Tauri UI shell; `collaboration-scope.md` (the real multi-user app),
  `admin-console-scope.md` (the management UI for workspaces·teams·users·members·extensions), and
  `dashboard-scope.md` (the grid-of-widgets dashboard over real series — Phase 1 first-party/seeded,
  with the full asset-sharing authz model; Phase 3 the real edge fleet; the `vision/0003` IoT dashboard
  made buildable), and `dashboard-widgets-scope.md` (Phase 2 — widgets as installed extensions: how a
  widget accesses data through the host-mediated read-only bridge without ever holding the token or
  touching the DB, trust tiers, the `[widget]` manifest); `frontend/dashboard/` now holds the dashboard
  subtopic index plus the widget-focused reconciliation scope — including
  `library-panels-scope.md` (panels as their own `panel:{id}` asset: reused across dashboards via
  `panel_ref` cells, edit-once-propagates with explicit unlink-to-fork, and rendered **standalone**
  on a `/panel/{id}` page; sharing a panel never widens data access), `ui-standards-scope.md` (the cross-cutting UI
  standard: shadcn-first primitives, the Members/NavRail canonical look, and responsive/mobile
  auto-resize — what every surface here must obey), `routing-scope.md` (shareable, deep-linkable
  URLs with typed search-param args — @tanstack/router in hash mode, working in both the Tauri
  desktop webview and the browser; e.g. a dashboard scoped to a date range), `data-console-scope.md` (the workspace
  data console: an admin-gated raw table browser + react-flow graph view, and an ingest/series explorer
  with manual write — the raw exploratory counterpart to the dashboard, for users who aren't good at SQL), and
  `theme-switcher-scope.md` (local shell preferences for light/dark mode and three token-bound accent palettes),
  and `rules-workbench-scope.md` (the rules workbench: a Playground to write/run/save Rhai rules, a
  React Flow chain canvas that colours steps as they settle, and a datasources admin page — first-party
  shell driving the shipped `rules.*`/`flows.*`/`datasource.*` verbs over the gateway, mirroring the
  dashboard pattern; the federation extension stays headless), and `rules-editor-ux-scope.md` (a guided,
  explorable authoring surface extending that Playground: a searchable function palette mirroring the
  registered Rhai verbs, click-to-load examples, and a datasource/schema/series data explorer — all
  click-to-insert, frontend-only over the shipped verbs, with the `store.schema` reader extracted to a
  shared `lib/schema` consumed by both the dashboard SQL builder and the rules explorer), and
  `graphics-canvas-scope.md` (the **free-form graphics surface** — Niagara-style plant graphics /
  floor plans / mimic pages / 3D buildings, a **100% UI extension** (control-engine precedent: no
  new core verbs/tables/WIT; docs co-locate with the extension once scaffolded): a declarative,
  dimension-agnostic scene document stored via the shipped asset/document verbs, rendered by **one
  engine — three.js via `@react-three/fiber`** (flat plant graphics = orthographic top-down camera;
  3D = the same document with a perspective camera — never built twice), drawn by hand (drei
  gizmos/controls) and **drawn by the AI agent** through the same shipped tools (skill-guided
  read-modify-save, validate-and-placeholder on LLM sloppiness); new equipment ships as **symbol
  packs (GLTF/SVG assets — data, not code)**; React Flow, Konva/Pixi, Babylon, tldraw, and the
  Awaken A2UI crate evaluated and rejected, their patterns kept), and
  `widget-kit-scope.md` (make widgets genuinely reusable across the whole system: a declarative per-field
  presentation vocabulary — `label`/`description`/`hide`/`order` — that both the request form and the
  response table honor through one resolver; extract the input widgets + registry out of the palette/
  dashboard/reminders feature folders into a common `lib/widgets/` library; and version the federation mount
  context with an input `value`/`onValue` channel + `defineWidget` so extensions can author form widgets,
  not just read-only tiles — additive over the shipped v2 widget contract, no new verb/cap/datastore).
  `frontend/dashboard/viz/` holds the
  **Grafana-compatible visualization** slice (the ask): adopt Grafana's panel/`fieldConfig`/transformation/
  datasource model and dashboard JSON so charts gain the full standard option surface, render units/dates/
  numbers through `prefs/` user-prefs, query any datasource (not just native SurrealDB), and import/export
  Grafana dashboard JSON — one scope file per part, additive over the shipped v2 widget contract.
  `dashboard-query-cache-scope.md` (a **client-only caching / call-de-dup layer** — adopt
  `@tanstack/react-query`, scoped to the dashboard route so the cache lives for the visit and clears on
  leave: collapses the 2–3× `viz.query` per draft panel, the twice-fetched source-picker bundle, and the
  per-cell series/flow reads to one shared call each; no host/verb/cap changes).
- `testing/`, `debugging/` — the standards every session follows.

See `../STAGES.md` for which stage each area lands in and `../STATUS.md` for what has shipped.
