# Scope docs

Pre-work briefs: the *ask* for each feature area, written before implementation (see
`../SCOPE-WRITTING.md`). One topic folder per area; one `<name>-scope.md` per ask within it.
A feature reads top-to-bottom across folders: `scope/<topic>/` → `sessions/<topic>/` →
`public/<topic>/`.

## Topics

- `agent/` — the central, workspace-scoped AI agent (S5).
- `agent-run/` — the agent **run** as a first-class object: a canonical `RunEvent` stream, an ACP
  stdio adapter (Zed/Cursor drive the agent), per-tool-call Allow/Deny/Ask with **durable
  suspend/resume**, and **model-activated skills** (the model picks from a granted catalog). Opens up
  the S5 black-box loop; ideas reviewed from the Awaken framework, its plugin framework rejected.
- `ai-gateway/` — the swappable model-access sidecar (S5).
- `observability/`, `audit/`, `undo/` — the **three cross-cutting projections of the host dispatch
  chokepoint** (README §6.5/§6.6), scoped together as the S10 retrofit: `observability/` (structured
  logs + distributed traces + metrics, emitted everywhere with a `trace_id` that survives the routed
  hop), `audit/` (an immutable, hash-chained, workspace-walled ledger of every allow/deny — generalizes
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
  `data.query`/`series.*`/`federation.query`, `ai.*` via the AI-gateway, `emit`/`alert` via inbox/outbox),
  plus **rule chains** — a rule DAG whose every step is an `lb-jobs` job, cron via the S6 reactor, event
  via `bus.watch` (`rule-chains-scope.md`). Exposed as `rules.*` / `chains.*` MCP verbs.
- `datasources/` — the native (Tier-2) **`federation` extension** (`datasources-scope.md`): embeds
  DataFusion + connectors to query external SQL sources (MySQL, PostgreSQL/TimescaleDB, …) under `net:*`
  + a mediated secret, surfaced as the read-first, workspace-pinned `federation.query` MCP verb (+
  `datasource.*` admin CRUD and a `federation.mirror` `lb-jobs` batch). SurrealDB stays the authority;
  external DBs are federated sources reached through the gated extension, never a second authority.
- `core/`, `crate-layout/`, `extensions/`, `mcp/`, `node-roles/`, `registry/`, `secrets/`,
  `store/`, `tags/`, `tenancy/` — the spine and platform surfaces. `extensions/` also holds
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
  behind one `Toolchain` trait).
- `flows/` — the visual **node-graph flow engine** (`flows-scope.md`): a node-red-style canvas
  over the shipped plane, not a new engine. Promotes the `chains` rule-DAG `Step` into a typed
  `Node` (`Trigger | Tool | Rhai | Subflow | Sink`), each carrying a **descriptor** (ports + a
  config **JSON-Schema** the React Flow editor renders a form from). **Extensions contribute
  backend node types** via an additive `[[node]]` block in `extension.toml` — **identical for
  WASM and native**, executed through the existing `caller ∩ install-grant` callback (an `mqtt`
  extension ships an "MQTT publish" node). A run is a durable `lb-jobs` `flow-run` job
  (suspend/resume = pause→edit-downstream→resume); triggers `manual|cron|event|inject|boot`;
  enable/disable + `start_on_boot` + `placement`; **dashboard↔flow** binding (`flows.inject` in,
  bus-subject out); shared via the grant model; graph edits undo for free. Rejected adopting
  `open-rmf/crossflow` (in-process Bevy-ECS state breaks rules 1/4, bypasses the cap wall).
- `files/`, `skills/`, `document-store/` — shared workspace assets (S4). `document-store/` now
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
- `workspace/` — the workspace session boundary plus the node-level workspace directory and admin
  lifecycle: list/create in the switcher, archive/rename/purge in admin, with workspace data always
  selected from the signed token.
- `channels/` — the collaboration channel surface: durable inbox-backed history, bus motion, channel
  registry, SSE stream, and presence. Also `channels-query-charts-scope.md`: in-channel SQL queries
  (via `federation.query`) whose results post back as durable items and auto-plot a chart; and
  `channels-command-palette-scope.md`: the `/` + `@` command surface (catalog-driven, capability-
  filtered MCP tools — the menu *is* the permission model) that composes those queries.
- `inbox-outbox/` — the normalized inbox (S2) and the transactional must-deliver **outbox**
  (`outbox-scope.md`, the S6 driver).
- `ingest/` — a generic buffered read/write surface for high-volume external data; the cloud-side
  ingest buffer (the read-side analog of the outbox). Stays domain-free — IoT is one caller (S9).
- `jobs/` — the SurrealDB-native durable job queue / resumable session (S5).
- `reminders/` — a durable, workspace-scoped **scheduled trigger that fires an action**
  (`reminders-scope.md`): a `reminder:{id}` record with a cron schedule + optional `max_runs` +
  `enabled` switch, fired by a `react_to_reminders` durable scan (the same altitude as the S6
  relay/approval reactors) that enqueues one `lb-jobs` job per firing. Three v1 action kinds —
  **channel post** (inbox), **MCP tool call** (any capability, under a stored principal re-checked
  at fire time), and **must-deliver effect** (outbox). Cron is the storage format; the UI authors
  it with a best-in-class React cron-builder. The single-action sibling of a rule chain.
- `prefs/` — per-(workspace,user) preferences + localization: language (en/es), timezone, date/number
  display style, and a backend unit-conversion layer (metric/imperial). Canonical data in, localized
  presentation out, exposed as `format.*`/`convert.*` MCP tools so thin clients don't re-implement it.
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
  subtopic index plus the widget-focused reconciliation scope, `ui-standards-scope.md` (the cross-cutting UI
  standard: shadcn-first primitives, the Members/NavRail canonical look, and responsive/mobile
  auto-resize — what every surface here must obey), `routing-scope.md` (shareable, deep-linkable
  URLs with typed search-param args — @tanstack/router in hash mode, working in both the Tauri
  desktop webview and the browser; e.g. a dashboard scoped to a date range), `data-console-scope.md` (the workspace
  data console: an admin-gated raw table browser + react-flow graph view, and an ingest/series explorer
  with manual write — the raw exploratory counterpart to the dashboard, for users who aren't good at SQL), and
  `theme-switcher-scope.md` (local shell preferences for light/dark mode and three token-bound accent palettes),
  and `rules-workbench-scope.md` (the rules workbench: a Playground to write/run/save Rhai rules, a
  React Flow chain canvas that colours steps as they settle, and a datasources admin page — first-party
  shell driving the shipped `rules.*`/`chains.*`/`datasource.*` verbs over the gateway, mirroring the
  dashboard pattern; the federation extension stays headless), and `rules-editor-ux-scope.md` (a guided,
  explorable authoring surface extending that Playground: a searchable function palette mirroring the
  registered Rhai verbs, click-to-load examples, and a datasource/schema/series data explorer — all
  click-to-insert, frontend-only over the shipped verbs, with the `store.schema` reader extracted to a
  shared `lib/schema` consumed by both the dashboard SQL builder and the rules explorer).
  `frontend/dashboard/viz/` holds the
  **Grafana-compatible visualization** slice (the ask): adopt Grafana's panel/`fieldConfig`/transformation/
  datasource model and dashboard JSON so charts gain the full standard option surface, render units/dates/
  numbers through `prefs/` user-prefs, query any datasource (not just native SurrealDB), and import/export
  Grafana dashboard JSON — one scope file per part, additive over the shipped v2 widget contract.
- `testing/`, `debugging/` — the standards every session follows.

See `../STAGES.md` for which stage each area lands in and `../STATUS.md` for what has shipped.
