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
  `agent-close-out-scope.md` **finishes the topic** — four honesty-at-a-seam slices closing the
  `public/agent/agent.md` "Not yet" list: real token accounting on `Turn` (provider `usage`, not the
  content-length estimate), per-workspace loop policy (`max_steps`/`max_run_tokens` on `agent.config`,
  node-clamped), run progress as ws-scoped bus motion + completion via the outbox (cross-node
  `agent.watch`), and the signed token on routed edge→hub invokes (hub verifies, never trusts).
  Fallback chains / the served OpenAI face / the curated tool menu stay deferred to their owning topics.
  `agent-context-basket-scope.md` (**shipped**) gives the dock an **Ask | Tools** toggle mounting the
  SHARED channel `CommandPalette` (no second palette), and a **context basket**: gather durable
  channel items (a query result, a rich response, a note) via a per-row paperclip and the next ask
  carries their ids (`AgentPayload.context_items` — refs, not bodies); the worker resolves + fences
  them into the run's goal ws/channel-scoped with hard caps (`channel/context_items.rs`, the sibling
  of the page-context fence).
  `agent-loop-hardening-scope.md` adopts the best transferable ideas from a survey of three OSS Rust
  agent runtimes (zeroclaw, carapace, hermes-rs): turn-group-preserving transcript compaction +
  in-loop context-overflow recovery, a stuck-loop detector (repeat / ping-pong / no-progress →
  warn/block/break) + a graceful ceiling exit, the dangling-tool-call invariant (a dead turn never
  persists a proposed call without its result), a transient/model-recoverable/fatal error taxonomy
  on structured provider errors, and an `emits_external` exfiltration taint declared on tool
  descriptors (guard filters the advertised menu AND the dispatch). Zero new verbs/tables; composes
  with `agent-close-out`; survey ideas owned elsewhere routed to `agent-memory`/`jobs`/`ai-gateway`.
- `agent-personas/` — **user-selectable agent focus**: a persona = `{granted_tools,
  grounding_skills, identity}` as pure data (rule 10), picked per workspace (`agent.config.
  active_persona`) or per invoke — narrowing the run's advertised menu/catalog/prompt, NEVER the
  capability wall (effective = persona ∩ agent ∩ caller). Fixes the observed "agent confused by
  the whole surface" symptom. Umbrella `agent-personas-scope.md` + four sub-scopes (+ #5 correction):
  `persona-model-scope.md` (the record, two tiers, `agent.persona.*` CRUD, `extends`, run-assembly
  application on both runtimes — absorbs acp-driver's unbuilt `granted_tools`/`persona_skill`),
  `persona-grounding-scope.md` (seed the FULL `docs/skills` corpus + promote `docs/testing/`
  runbooks to skills + author `core.mcp`/`core.acp`/`core.extension-authoring` — the agent learns
  the platform from docs, not the codebase), `persona-catalog-scope.md` (eight built-ins as a
  `personas.toml` seed with exact verb lists: data-analyst, flow-author, widget-builder,
  rules-author via `extends`, workspace-admin, channels-operator, system-manager,
  extension-builder; destructive verbs excluded from all), `persona-coding-scope.md` (the
  extension-builder posture — "100% coding, never on its own": devkit surface only, Ask-gated
  publish/install via the shipped Part-2 policy, in-house-runtime-only until the external-agent
  capability wall ships), and `persona-session-scope.md` (#5, post-ship correction: the workspace
  enables a roster (`enabled_personas`), each run applies ONE persona — context-suggested from the
  page via `Persona.surfaces` (client-matched, rule 10) with a sticky per-tab pin sent as the
  per-invoke `persona` arg; defaults = `Prefs.agent_persona` axis (member → ws-default fold);
  union-of-N rejected, `extends` records stay the composition path; zero new verbs).
- `app/` — the **React Native mobile app** (iOS/Android): a thin RN shell that is the fourth
  client of the gateway (login → many workspaces → REST + SSE; **zenoh-ts rejected** — it would
  expose a second, unmediated server surface beside the gateway), plus **federated app extensions**
  via Re.Pack 5 + Module Federation 2 — an additive `[app]` manifest block beside `[ui]`, JS-only
  remotes published through the unchanged signed-`Artifact` path, mounted as React components over
  the same `ctx`/`bridge` contract as the web. `app-shell-scope.md` (host + transport + session),
  `app-extensions-scope.md` (the model + two reference exts: `proof-panel-app` pairing the wasm
  `proof-panel`, and pure-app `channel-chat` over channels + the in-channel agent),
  `app-sdk-scope.md` (`@nube/app-sdk` — the authored contract source + shared verb map, aligning
  with the `panel-kit` promotion toward one shared panel/widget SDK). Source workshop: `app/`.
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
  `agent-ext-authoring-scope.md` makes the external agent an **extension author** (any tier — wasm
  Rust tools, native sidecars, federated UI pages): a stdio MCP shim bridges the subprocess onto the
  caps-checked tool surface (run-scoped token per `agent-key-lifecycle`), unlocks
  `builtin.extension-builder` on external runtimes (Ask-gated publish preserved), and upgrades the
  devkit `ui` template to shadcn/recharts on the shell theme.
- `agent-run/` — the agent **run** as a first-class object: a canonical `RunEvent` stream, an ACP
  stdio adapter (Zed/Cursor drive the agent), per-tool-call Allow/Deny/Ask with **durable
  suspend/resume**, and **model-activated skills** (the model picks from a granted catalog). Opens up
  the S5 black-box loop; ideas reviewed from the Awaken framework, its plugin framework rejected.
- `ai-gateway/` — the swappable model-access sidecar (S5).
- `embeddings/` — the doc→vector pipeline: `Provider::embed` on the ai-gateway contract, a
  chunk→embed→HNSW indexer over the document store (reactor + `docs.reindex` job), and a hybrid
  `docs.search` verb (metadata filter + KNN, results re-gated per doc read reach). Vectors are
  derived, rebuildable, never synced; model + dimension pinned per index, migration = explicit job.
- `observability/`, `audit/`, `undo/` — the **three cross-cutting projections of the host dispatch
  chokepoint** (README §6.5/§6.6), scoped together as the S10 retrofit: `observability/` (structured
  logs + distributed traces + metrics, emitted everywhere with a `trace_id` that survives the routed
  hop — `observability-scope.md` is the **emit** half; `telemetry-console-scope.md` is the **consumer**
  half: a FIFO-capped SurrealDB sink (reusable `lb-store::capped` ring), a gated workspace-walled
  `telemetry.query`/`tail` read surface, and an in-browser console with first-class filters that also
  reads the `audit/` ledger lane — the self-contained, no-external-Grafana-required view), `audit/` (an
  immutable, hash-chained, workspace-walled ledger of every allow/deny — generalizes
  §6.14's model-call audit), and `undo/` (a reversible-command journal whose hard line is *reverse
  state, compensate motion* — `undo-scope.md` is the shipped mechanism; `undo-exposure-scope.md` is
  the follow-on slice that makes it product-reachable: role grants + typed gateway routes + the
  shell affordance). See "The shared seam" in `observability/observability-scope.md`.
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
  stated design to implementation; the gap surfaced in the access-console session), and
  `login-hardening-scope.md` (the `POST /login` dev-shim's two leaks a live session found: **no
  credential check** — any body mints a token — and **every login gets an admin-grade cap bundle**
  so a nominal member can add members / self-grant `workspace.delete`; adds a `CredentialCheck` seam
  + role-scoped cap issuance behind the same `mint`/`verify` boundary, restoring README §6.6 RBAC),
  and `access-model-scope.md` (**team-as-the-unit-of-access** + a `dashboard.access_check` preflight
  that walks a dashboard's transitive **dependency closure** — panels, datasources, query verb +
  `net:` endpoint caps, required vars — so "assigned a dashboard" provably means "the queries run";
  a live session found bob assigned a page whose cells still 403'd on a private panel + a missing
  datasource), and `entity-scoped-grants-scope.md` (**row-level reach inside a workspace** — an
  additive `scope` selector on the grant record + `check_scoped`/`scope_filter` at the wall and via
  SDK host-callback, so "a member reaches only *their* records" — a guardian's children, a
  technician's sites — is platform-enforced data instead of N hand-rolled ext filters; first
  consumer: the cc-app childcare product), and `authz-verbs-mcp-dispatch-scope.md` (the
  one-arm routing gap that blocks the above from the native tier: the MCP dispatcher routes
  `authz.*` to `call_authz_tool` but not `grants.*`/`roles.*`/`teams.*` — which that handler
  already implements — so a native extension can *read* the scoped-grant surface over the
  host callback but cannot *mint* a grant; additive, no new verb/cap/WIT). The
  **confirmed wire shapes** an out-of-tree native ext reaches all of the above by live in
  `mcp/ems-provisioning-verb-shapes-scope.md` (answers issue #48: the exact request/reply of
  `rules.save`/`series.latest`/`authz.check_scoped`/`scope_filter`/`grants.assign`, each pinned to a
  green test — no `rules.create`, `series.latest`→`{sample}` not `{value,ts}`). Also see
  `invites-scope.md` (**token onboarding for people
  who don't exist yet** — a durable single-use `invite` record carrying role/team intent + an
  opaque caller payload, delivered via an outbox email target, redeemed on the one pre-auth accept
  route into identity + membership + grants atomically, caps live on first login; the missing
  "self-join link" half of global-identity).
- `admin/setup/` — **setup wizards**: the AI-facing playbook for building a guided, multi-step flow
  in the Setup tab (`setup-wizards-scope.md`). The hard rule it enforces: a wizard is **pure
  orchestration over existing editors/hooks/verbs** — if the surface it guides already exists, the
  wizard **reuses that exact code** (or extracts a shared component), never a fork. Documents the
  generic `StepFlow` framework, the four files a wizard touches, the required **reuse ledger**, and the
  worked lessons (Appearance = provider-coupled editors reused as-is; Ingest = the "it didn't actually
  create it" bug → a step that claims to create must persist before advancing, and the real-gateway
  test must read the effect back).
- `bus/` — the Zenoh message bus (motion). `unified-event-stream-scope.md` adds the **browser
  leg**: one multiplexed SSE connection per app session carrying every live feed as a
  cap-re-checked *subject* (run/channel/series/flows/telemetry/insights), replacing the
  one-`EventSource`-per-feed pattern that saturates the browser's ~6-connection HTTP/1.1
  pool and makes an active agent run "block" navigation (browsers refuse cleartext HTTP/2,
  so the cap is structural on the plain-HTTP posture; verified live —
  `debugging/frontend/agent-dock-blocks-navigation-sse-pool-exhaustion.md`).
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
  `rules-messaging-scope.md` gives a rule body the explicit, caller-gated `inbox`/`outbox`/`channel` rhai
  handles (raise/read/resolve/enqueue/post). `rules-approvals-scope.md` builds on it: a rule
  `inbox.request_approval` raises a `needs:approval` item that stages a **held** outbox effect, and a
  resolution reactor **releases the effect only on `Approved`** (discards on reject) — the "a rule
  proposes, a human disposes" loop, reusing the coding-workflow's `Item`+`Resolution`+reactor mechanism.
  `data-stdlib-scope.md` fills the cage's *compute* gap (the complement to datasources' *fetch*): a
  ~180-function data standard library that registers once in the one rhai cage, so **rules AND the flows
  `rhai` node (via `rules.eval`)** both get it — a `time` handle over the run's injected logical clock
  (no wall-clock; `timestamp()` disabled), JSON + SurrealDB-shape helpers (`thing_id`, `epoch`,
  deep-path get), scalar/array stats (median, percentiles, z-scores, rolling windows, linreg, outliers),
  and a **polars-backed `Frame`** (`g.frame()`, method verbs + in-memory `f.sql("… FROM self")`) in a new
  feature-severable `lb-frame` crate. Pure, deterministic, zero-I/O compute that adds **no new authority**
  — rows still enter only through the gated seams; no new MCP verbs.
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
  rejected. **`sqlite-datasource-demo-scope.md`** makes the sidecar's shipped `sqlite` kind
  first-class (kind select + path-DSN semantics in the Datasources UI) and emits the demo building
  dataset into a SQLite file (`seed.py --sqlite`, lite profile + `make seed-demo-sqlite`) — the
  Docker-free demo source the Data Studio 10x demo toggle points at.
  **`datasource-samples-scope.md`** adds `federation.sample {source, tables?, limit?}` — one bounded,
  AI-prompt-ready snapshot of a source (tables + columns + real foreign keys + `LIMIT 10` rows per
  table) under the existing `federation.query` cap, so an agent writes correct SQL in one round trip
  instead of N+1 `federation.schema` calls with no relationship metadata.
  **`federation-pushdown-scope.md`** makes single-source `federation.query` push the whole validated
  SELECT down to the source engine (the pinned providers' `*-federation` features +
  datafusion-federation) instead of streaming base-table rows into DataFusion and joining in the
  sidecar — the demo JOIN/GROUP BY drops from 3–4 s to engine speed; same verb, caps, and envelope.
  Decomposed into `page-cursor-scope.md` (A: the cursor codec + keyset primitive),
  `series-paging-scope.md` (B: native `series.read` rows fast path), `series-decimation-scope.md`
  (C: chart bucket downsampling), `federation-paging-scope.md` (D: external pushdown + mirror routing),
  and `page-chaining-ui-scope.md` (E: the data-console table + dashboard viz callers).
  **`schema-designer-scope.md`** adds the federation **write plane** + visual schema design: a
  `db_schema:{ws}:{name}` record edited on a React Flow canvas (tables/columns/PK/FK; tabularis
  Apache-2.0 lift, ChartDB UX-reference-only), applied by an Ask-gated `federation.migrate`
  (dry-run-default DDL diff), written per-message by a bounded upsert `federation.write` (the
  flow `tool`-node target), and backfilled by `federation.export` — a checkpointed `lb-jobs`
  batch, the outbound dual of `federation.mirror`. External DBs become sinks, never authority.
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
  `store/`, `tags/`, `tenancy/` — the spine and platform surfaces. `store/` also holds
  `session-concurrency-scope.md` — a **tracking** scope (not a green light) for the global session
  mutex that serializes every query node-wide: measured, 18 concurrent writers each in their OWN
  workspace take 7.0ms = 18 × 0.4ms (zero parallelism). Deliberate — it makes `use_ns` + query one
  critical section and removing it reintroduces a cross-workspace leak — and cheap enough today
  (0.4ms/op) that the recommendation is spike-before-coding, not fix. `store/` also holds
  `online-compaction-scope.md` — bound a **long-running** node's SurrealKV commit log: boot-time
  compaction and the retention-GC reactor shipped, but on an append-only engine runtime evictions
  only append tombstones, so bytes (and next-boot replay) grow until restart (measured: ~65× bloat,
  13–14s boot). Ships `store.status` observability first; the compaction pass itself is a
  spike-decided handle-swap job behind the session mutex, with supervised restart-to-compact as the
  honest fallback. `core/` also holds
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
  behind one `Toolchain` trait), and `ext-out-of-tree-scope.md` (**split the extensions out**:
  every product extension moves to a `lb-extensions` repo — only `federation` stays — against
  three published SDK surfaces (`lb-sdk` WIT/wasm, a new `lb-ext-native` child-side facade with a
  versioned `init` handshake, and `@nube/ext-ui-sdk` as the single source of the page/widget contracts),
  an Artifact v2 that carries the UI bundle, and the previously deferred thin `lb-ext` CLI — the
  publish/trust path unchanged), and `pack-toolchain-publish-scope.md` (the **prerequisite slice** of
  that CLI: `lb-devkit` + `lb-pack` are `publish = false` today, so **no embedder can sign an
  extension artifact** — an embedder builds a `.wasm` but cannot package it into the signed `Artifact`
  the gateway accepts (cc-app's `make dev` dies at `cargo build -p lb-pack`); drop the flag, make the
  packager git-tag/`cargo install`-consumable, and document it in the dev flow — no new trust model),
  and `extension-watch-scope.md` (the **generic live-feed primitive for
  extensions**: an extension marks a `[[tools]] kind="watch"` and the host relays it as SSE over a
  host-allocated workspace subject — closing the asymmetry where only core tools could stream; the WIT
  ABI stays frozen, streaming rides the bus, and the routed cross-node relay is free; `control-engine`'s
  `ce.watch` is its first tenant). The **`extensions/ui/`** subtopic (`ui/README.md` index) owns the two
  extension-UI contracts the theme customizer forces: `theme-inheritance-scope.md` (an extension page/
  widget **re-themes live** with the host when the user changes theme — CSS-var cascade for in-process
  DOM, injected+refreshed vars for the iframe tier, and resolved token values in `ctx.theme` so a JS/
  canvas widget like `echarts-panel` recolors via the v-next `update` hook) and `css-isolation-scope.md`
  (an extension's CSS **never leaks into the host shell** — the shipped `library-css-leaks-global-utilities`
  regression turned into an enforced remote-CSS contract: scoped utilities, aliased tokens, no preflight,
  a build-time guard in `lb devkit`, and a runtime cascade-layer/container fence). `node-roles/` also
  holds `embed-node-scope.md` (**lb as a Rust library**: the boot ritual `main.rs`/the Tauri shell/
  `test_gateway` each hand-copy today becomes ONE `BootConfig` + `NodeBuilder` lib target on the
  `node` package — the sanctioned role-aware layer, §3.1 — with struct config (`from_env()` only at
  binary boundaries), composable subsets (gateway/reactors/federation toggles), real teardown, and
  the three existing embedders refactored onto it as proof; git-dep embedding, deliberately NOT a
  crates.io publish of `lb-host` and NOT a new repo).
- `desktop/` — the Tauri v2 desktop shell as a **shipped executable**. `desktop-packaging-scope.md`
  builds the existing `lazybones-shell` (`ui/src-tauri` — node in-process + window, the `workstation`
  persona) into **plain binaries** (no AppImage/installer) for Linux x86-64 (`tauri build --no-bundle`
  + `--features desktop`; dynamically links webkit2gtk-4.1) and Windows x86-64 (WebView2 is
  OS-provided, exe is standalone), via a GitHub Actions matrix + a real-binary boot smoke. Zero
  product code — toolchain, build wiring, proof. The Tauri command-layer verb gap stays a separate ask.
  `desktop-build-container-scope.md` makes that slice's "real dev box or CI" line **reproducible**: one
  Docker image (`desktop/docker/`) with the webkit2gtk-4.1 toolchain + Rust + pnpm that produces the
  bare ELF from a clean checkout — host-pollution-free, same image dev and CI use, build-only (the
  shipped binary is a windowed app, not a container workload). Linux-x86-64 only; darwin/windows stay
  on their native runners (rejected in the parent scope).
  `desktop-standalone-backend-scope.md` adds the **`full`** build mode — the shell mounts the SSE/HTTP
  gateway in-process on a loopback port + runs the boot seeders, so the packaged binary is a 100%
  standalone node (login/MCP/SSE/agents/flows/rules/insights, no external node).
  `desktop-federation-bundle-scope.md` closes the one hole in `full`: it **bundles the federation
  sidecar** into the standalone build and auto-installs it at boot (with a `net:*` grant for the local
  sqlite convention + the shipped `demo-buildings.db` pre-registered), so datasources register **and**
  query end to end in the `.exe` — the "register-but-can't-test" gap otherwise fixed only by `make dev`.
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
  deferred slice with its own threat model. `files/` also holds `media-scope.md` (**photo-class
  binaries at product volume** — chunked/resumable `upload_begin/chunk/commit` that survives
  cellular, server-side variants (thumb/preview) as a durable job, and a capability-checked
  streaming `GET /media/{id}` with Range/ETag — all on SurrealDB buckets, rule 2 intact; the
  generic binary path under document-store attachments and any feed of daily photos).
  `document-store/` also holds `doc-extraction-scope.md`: the **binary→markdown derivation seam** —
  a per-mime `Extractor` registry (PDF text-layer, XLSX/CSV, HTML; pure, offline, no network) run
  as a `docs.extract` **job** that derives markdown docs from media, with a `derived_from` edge to
  the original, an extraction ledger (`checksum + extractor@version`, idempotent re-runs, in-place
  re-derivation on version bumps so links/embeddings survive), and caller-supplied tags/visibility
  (core knows mimes, never domains). Completes file→doc→vector→search with `embeddings/`.
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
- `calendar/` — workspace / team / user **calendars** with shareable events (`calendar-scope.md`):
  a Gmail-style calendar as one native (Tier-2) extension — RFC 5545 recurring events (canonical
  event + `rrule`-materialized occurrence window, never expanded at read time), per-calendar reach
  via **entity-scoped grants** (team share = one `team:{id}` grant), invites as an attendee
  PARTSTAT/sequence state machine (RFC 5546) with must-deliver outbox notifications, event
  reminders as `lb-jobs` `run_at` jobs (no new scheduler), keyset-paged merged `events.range`,
  a `watch` live feed, and `.ics` import (job) / export via the `icalendar` crate.
- `widgets/` — the **system-wide widget platform** umbrella (`widget-platform-scope.md`): a widget is one
  `{view,source|data,options,action,tools}` envelope, one renderer (`WidgetView`) across dashboards,
  channels, and the app. Maps the four widget sources (built-in views, **tool result-renders** — e.g. the
  reminder widget, ext `[[widget]]` tiles, genui) and the slices that connect them: catalog + save-gate
  (Slice A, `frontend/dashboard/widget-catalog-scope.md`), pin-a-tool-render-to-a-dashboard, result-render
  coverage, channel→widget→dashboard authoring, and extension-capability introspection. The **channel is
  the test-bench** for the whole system.
- `inbox-outbox/` — the normalized inbox (S2) and the transactional must-deliver **outbox**
  (`outbox-scope.md`, the S6 driver); plus `push-target-scope.md` (**push notifications as one more
  outbox `Target`** — per-member `device` registrations (FCM/APNs/WebPush) + a generic opaque
  notification effect fanned out behind one `PushProvider` trait, token-gone auto-eviction, a prefs
  quiet-hours gate; the outbox already owns durability/retry, so this is a target, not a service).
  Also holds `mail-source-scope.md` (**inbound email as a generic producer**, receive-only):
  `mail.source.*` CRUD with credentials as a **secrets path** (never values), a durable poll job
  per source (IMAP v1 behind one `MailFetch` trait; UID cursor, Message-ID ledger, resumable,
  node-claimed) running as a narrow per-source api-key principal, normalizing each message to the
  existing surfaces — raw `.eml` as media, body as a markdown doc, attachments through
  `docs.extract` — plus an arrival bus event; routing/tagging policy stays caller-side (rules),
  sender allowlist/quarantine ships v1. Sending stays the outbox's job.
- `ingest/` — a generic buffered read/write surface for high-volume external data; the cloud-side
  ingest buffer (the read-side analog of the outbox). Stays domain-free — IoT is one caller (S9).
  Also holds `webhooks-scope.md` — a first-class inbound-HTTP surface (keyed like an API key,
  emitting an ingest `Sample`, wrapped by a generic flow `webhook` source node; no provider nodes),
  and `series-sample-cap-scope.md` — a **per-series FIFO sample cap** (`max_samples` on the retention
  policy, evict-oldest-by-`ts`), the **missing GC driver** (`run_gc` is called only by tests and the
  on-demand verb — nothing ticks it at boot, so shipped time-based retention never actually runs),
  and a **safe default bound** (today: no policy = keep forever). Measured ~700 bytes/sample →
  50 series @ 1/sec ≈ 3GB/day; the existing series *cardinality* cap bounds series COUNT, not samples
  each. All three needed together or a disc still fills,
  and `drain-backpressure-scope.md` — **shipped 2026-07-15**: `ingest.write` drained the WHOLE
  workspace backlog inside the caller's call (one sample → 18.5s against a 4.6k-row backlog, and
  self-sustaining — a caller that timed out left the rows staged). Root cause was the **missing
  driver** for the commit worker this scope has always named, so every caller became the worker.
  Fixed by bounding each caller's drain to its own batch (`own_batches`, all four call sites) and
  wiring `spawn_ingest_reactors` — the `outbox` relay's twin — into node boot. Measured
  900.7ms → 66.0ms. The blamed `ORDER BY` re-sort was **disproven** by measurement and left alone.
- `insights/` — a durable, queryable **data-insight record** (`insights-scope.md`): the one
  missing piece over the shipped detect/orchestrate/attention planes — `insight:{ws}:{id}` with
  severity, origin provenance (rule/flow/agent + run), a `dedup_key` with occurrence counting
  (flap suppression + re-open on recurrence), and an `open → acked → resolved` lifecycle.
  Raised from rules via a caller-gated rhai handle (the rules-messaging pattern), from flows
  via a built-in `insight` sink node, or by any principal via `insight.raise|list|get|ack|
  resolve|watch`; entity refs ride the **tag graph** (faceted "spark list" discovery); the AI
  story is the shipped agent dock + a data-only `builtin.insights-analyst` persona — no new
  agent surface. Deliberately **not** an inbox item, an outbox effect, or a channel-per-rule
  (all three rejected in-scope). Three sub-scopes carry the key features:
  `insight-occurrences-scope.md` (the per-insight **transaction log** — one lite, size-capped
  row per raise in a capped ring, lifetime `count` beyond the ring),
  `insight-subscriptions-scope.md` (a member **subscribes a channel** to all / a rule / an
  identity / a tag facet / a severity floor, delivered under the subscriber's stored
  reminders-pattern principal), and `insight-notify-scope.md` (the **anti-spam digest
  ladder** — noisy keys decay immediate → hourly → daily → weekly → monthly summaries and climb
  back when quiet; first-occurrence / severity-escalation / re-open always break through; ack
  suppresses; ws policy record + per-sub overrides + per-member kill switch). The fraud and
  HVAC/energy (SkySpark-style, `docker/postgres/seed.py`) verticals build on it as
  config/extensions with zero core branches. `rule-raises-insight-scope.md` builds the **rule
  producer door**: a rule body raises/**acks**/**closes** an insight in one line via a new
  `insight` rhai handle over the existing verbs (no new verb, no new cap), deciding the
  `route:false` read-only-panel suppression and the emit/alert boundary. Index:
  `insights/README.md`.
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
  Also `job-retention-scope.md` (**reactor drain-scan cost + terminal-job retention** — makes
  `lb_jobs::pending` an indexed O(pending) query instead of a full-table walk, and bounds the
  terminal `job`/`flow_run`/`flow_step_output` rows that otherwise grow forever and peg a node's CPU
  on the reactor tick — see `debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`).
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
- `release/` — release engineering. `updates-to-core-release-scope.md`: finish and tag the
  `updates-to-core` branch (five platform-gap features) — the reviewed gap list (relay-reactor
  boot wiring, the four i18n surface fixes riding the existing prefs MF1 en/es catalog engine),
  what is already done (invite-accept rate-limit), and the tag/publish checklist.
- `reports/` — the **report builder + branded PDF exporter** (`report-builder-scope.md`): a
  notebook-style `report:{ws}:{id}` asset of ordered blocks — markdown (true-A4 TipTap WYSIWYG),
  images (`assets.*` refs), and **existing dashboard panels** (the shipped `panel_ref`/inline-spec
  Cell duality, rendered through the one `WidgetHost` path — extension widgets work with zero
  report-side code) — plus reusable **`brand:{ws}:{id}` profiles** (logo/colors/fonts/header/footer
  + a shared BrandPicker; workspace `ui_branding` stays identity, this is document branding) and a
  **branded PDF** via a pure `lb-render` crate ported from the proven lazybones Typst pipeline
  (pinned `typst =0.15.0`, custom `RenderWorld`, embedded fonts, no external binary). Panels export
  as **client-captured snapshots** under the exporter's caps (server never fetches widget data;
  headless-browser and Rust re-render rejected); `report.export` is a gated bounded-sync verb on a
  binary gateway route. Sharing a report is a lens — embedded panel data re-checks the viewer's
  caps per render. GitHub publishing, sources/RAG, and scheduled/emailed exports are named
  deferrals.
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
  `nav-hide-and-pins-scope.md` extends it with a workspace-admin **hidden-set**
  (`nav.hidden_get/set` — hide e.g. the Dashboard surface from every tier incl. the
  fallback; declutter only, never blocks a route) and per-user **pins**
  (`nav_pref.pinned` — a personal ordered Pinned section atop the rail); hide beats pin.
- `query/` — saved **PRQL** queries (`prql-query-scope.md`): author once in PRQL (or `lang:"raw"`),
  **save as an editable `query:{ws}:{id}` record**, and run against the SurrealDB-native store
  (`store.query`) or a registered datasource (`federation.query`) through one `query.*` MCP family.
  A pure `lb-prql` crate wraps `prqlc`; `query.run` composes the target's existing capability (no
  widening); a rule reuses a saved query via `source("query:<name>")`. No new engine, no second
  authority — PRQL is the authoring layer, SurrealDB stays the one datastore.
- `viz/` — the **backend half of Grafana parity** (`grafana-parity-backend-scope.md`, audited
  2026-07-14 vs Grafana 13.2.0-pre): the additive `Cell`/`Dashboard`/`Variable` model fields a
  Grafana v1 import needs, `lb-viz` transformer tranche 2 + the missing reduce calcs, and the one
  **import pin** (classic v1/schemaVersion 42 accepted, `__inputs` resolved, v2 kind rejected with
  notice) consumed by `dashboard.import` and the downstream converter. Backend-only — the typed
  option shapes/editors/renderers are the downstream consumer's UI scope (rubix-ai
  `frontend/dashboard/viz/grafana-parity-ui-scope.md`).
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
- `clients/` — **external starter client libraries** for the gateway surface
  (`client-libraries-scope.md`): one thin library per language (TypeScript/Node, Python, Go, Rust)
  under repo-root `clients/`, each exposing the same five-method shape — `Client` (base URL + bearer)
  + `login()` + `writeSamples()` + `latestSample()` + `callMcp()`, plus a `signWebhook()` /
  `postWebhook()` helper for the third-party caller path. Deliberately **not** a full SDK: the shape to
  extend, with every other verb reachable through the universal `POST /mcp/call` bridge. No mocks, no
  fake backends — the README recipes hit a real `make cloud` node seeded via the real write paths. Adds
  **no new MCP verbs, capabilities, routes, or tables**; the four folders live outside both the core
  `rust/Cargo.toml` workspace and the root `pnpm-workspace.yaml` so a change here cannot break the
  core build.
- `frontend/` — the React/Tauri UI shell; `minimal-shell-scope.md` (the **publishable minimal
  host** for 100%-extension UIs — only the host-side contract: auth screens + workspace pick,
  `ext.list` discovery, full-screen scoped mount of a *configured* ext page, SSE + theme-token
  provider, PWA defaults; retires "vendor the whole shell" as the only embedder option — the
  rubix-ai compromise — and gives mobile-first products like cc-app a stand);
  `agent-dock-scope.md` (the persistent
  `@nube/panel` right-dock AI panel — open on every page, survives navigation, durable
  channel-backed session history with new-session, always the active catalog agent,
  page-context injected, answers streamed with live progress over the run-event SSE),
  `collaboration-scope.md` (the real multi-user app),
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
  `query-builder/` (the **query-builder 10x** subtopic — a Tabularis-grade drag-and-connect visual JOIN
  builder + a schema-aware CodeMirror editor + a standalone `/t/$ws/query` workbench view that also opens as
  a Data Studio pane; UI-only, extends the shipped `SqlBuilderQuery`/`emitSql` seam, no backend; plus
  `tabularis-harvest.md` — what else to take from Tabularis), and
  `theme-switcher-scope.md` (local shell preferences for light/dark mode and three token-bound accent palettes),
  and its successor `theme-customizer-scope.md` (the ported shadcn-store Customizer: a preset library +
  radius + import + custom colors that write the project's **base** design tokens so every existing
  chart/panel re-themes live, persisted **per member** via `prefs` with an admin-set **per-workspace
  default** — member override wins), and *its* successor `theme-appearance-scope.md` (the full look-and-feel:
  one-click **look packs** — code editor, professional, retro, modern dashboard, liquid glass — plus font
  tokens, surface treatments (translucency/blur/elevation/gradients), a motion.dev-backed motion system with
  an off switch, a wider tone palette, the radius-coverage and color-picker fixes, and the widened `ctx.theme`
  live re-theme signal for extensions), and its own successor `shell-chrome-layout-scope.md` (two more
  Layout-tab appearance axes on the same `ui_theme` blob — a **header style** rendered as a shadcn
  `Breadcrumb` vs today's icon-chip band, and a **navigation mode** that moves the sidebar into a
  top `Menubar` with dropdowns; frontend-only, two data axes + two sibling renderers, no backend),
  and `workspace-branding-scope.md` (admin-owned workspace **identity**
  — logo, favicon, site/login heading — via workspace-default prefs + `assets.*`, including the narrow
  read-only pre-auth `/public/branding/{ws}` seam that brands the login page before any token exists),
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
  not just read-only tiles — additive over the shipped v2 widget contract, no new verb/cap/datastore), and
  `system-catalog-scope.md` (grow `@nube/source-picker` into the one **workspace system catalog**: the
  model/loader seam gains local-schema/channels/insights/inbox loaders and a second UI skin — the browsable
  click-to-insert explorer tree extracted from the rules panel's `DataExplorer` — so rules, Data Studio,
  dashboards, and extension UIs all browse "what exists in this workspace" through one package; enumerate +
  pick only, shipped verbs only, honest per-section deny), and
  `data-studio-10x-scope.md` (the **Data Studio 10x** ask, follow-on to the shipped
  `data-studio-scope.md` v2/v3: swap flexlayout-react for **Dockview** as the dock engine; open the
  app's own pages — Flows/Rules/Data/Datasources/Ingest — **as panes** inside the studio (the real
  routed view components, one persisted per-member arrangement, an `AppPage` embedded mode); rework
  the builder into a **query-first → visual viz-gallery → options-drawer** flow; and an honest
  **seeded-demo-data** preview toggle (real records via the `iot_demo` seed + `docker/postgres/seed.py`,
  never client-fabricated frames — rule 9)), and
   `webhooks-admin-scope.md` (the **Webhooks admin page adopts the `AppPage` shell** — a frontend-only
   restyle/UX slice over the shipped `webhook.*` verbs: the page migrates off the legacy `AdminPanel`
   onto the same canonical shell Dashboards/Rules use, the wizard upgrades to the surface discipline, and
   the file splits one-component-per-file during the move. The first admin-tab migration; the other five
   tabs follow under `admin-console-scope.md`), and
   `query-builder-common-scope.md` (make the **Query Builder common**: a LOCAL TABLE source
   (SurrealDB, `store.query`) gets the interactive Builder⇄Code editor today; an external DATASOURCE
   (`federation.query` — postgres/timescale/sqlite) gets only a raw-SQL textarea. Lift the deferral
   recorded in `dashboard/viz/datasource-binding-scope.md` — its prerequisite (`federation.schema
   {source, table?}`) has shipped. One shared `SqlBuilderQuery` state, N dialect emitters behind a
   `SqlDialect` seam (`toSurrealQL.ts` stays one impl; add a standard-SQL emitter for federation);
   the same `SqlQueryEditor` for both, fed by `federation.schema` for federation dropdowns. The wire
   shape (`federation.query {source, sql}`) is unchanged — pure UI + a TS emitter module, no new
   verb/cap/table).
   `frontend/dashboard/viz/` holds the
  **Grafana-compatible visualization** slice (the ask): adopt Grafana's panel/`fieldConfig`/transformation/
  datasource model and dashboard JSON so charts gain the full standard option surface, render units/dates/
  numbers through `prefs/` user-prefs, query any datasource (not just native SurrealDB), and import/export
  Grafana dashboard JSON — one scope file per part, additive over the shipped v2 widget contract.
  `dashboard-query-cache-scope.md` (a **client-only caching / call-de-dup layer** — adopt
  `@tanstack/react-query`, scoped to the dashboard route so the cache lives for the visit and clears on
  leave: collapses the 2–3× `viz.query` per draft panel, the twice-fetched source-picker bundle, and the
  per-cell series/flow reads to one shared call each; no host/verb/cap changes).
- `deploy/` — shipping a node to a target host. `fly-deploy-scope.md` (the ask): a
  one-command **Fly.io** deploy — a single Fly Machine running a Caddy-fronted Lazybones
  `node` (the `cloud` posture) with an embedded SurrealDB store on a persistent volume and
  the **federation sidecar in SQLite mode** (the shipped `demo-buildings.db`
  pre-registered) — **no bundled/hosted Postgres** (rule 2). The load-bearing decision is
  **reuse**: the Dockerfile, Caddyfile, entrypoint, config template, and `.dockerignore`
  live once in `deploy/common/` and are shared verbatim across local Docker, GitHub CI,
  and Fly, so the image is identical everywhere; `deploy/fly/` is a thin driver that only
  *references* them. Adds **no** MCP verb/cap/route/table and touches no core crate —
  toolchain + config + docs only. Adapts `/home/user/code/rust/dev-pulse/FLY.md`, dropping
  its bundled-PG and OAuth steps. Assets: `deploy/common/`, `deploy/fly/`.
  `rubixd-rartifacts-scope.md` (the ask): the **fleet package plane** — `rartifacts`, a
  REST artifact server (signed multi-arch binaries, docker image archives, bundle
  manifests; semver + channels, content-addressed blobs), and `rubixd`, a per-machine
  agent that reconciles **bundle YAMLs** (packages + named instances, `needs` ordering,
  `${secret:...}` refs) through two backends — systemd (`service-manager`, versioned
  release dirs + `current` symlink) and Docker (`bollard`, labeled containers) — with
  health-gated **automatic rollback** and multi-instance installs. Both are new
  out-of-tree binaries (a `rubix-fleet` repo); they reuse lb's Ed25519/SHA-256 artifact
  envelope and atomic-install conventions but touch no lb crate. Installs product hosts
  (`rubix-ai`, `ems`) and companions (TimescaleDB); extensions still publish through the
  running node's gated API. **rartifacts is itself built ON lb** — a product host
  embedding `lb-node` with all package logic in a native (Tier-2) `rartifacts`
  extension (`pkg.*` MCP tools, ext-owned content-addressed blob dir) and its console
  as the extension's **federated shadcn/Tailwind UI** on the minimal shell; agents/
  publishers are **api-key principals** (revoke an agent = revoke its key, instant),
  packages are `public` (anonymous download, no token) or `private`, and rubixd —
  which stays a **standalone lb-free daemon** with a small embedded Bootstrap UI —
  speaks only plain host-mounted REST, with **unlimited `[[remote]]`** rartifacts
  connections. Both services bootstrap via a **boot-generated, one-time-UI-claimable
  admin token** (shared `fleet-auth`) that doubles as the REST bearer. The umbrella
  decomposes into per-slice scopes + AI coding-session roadmaps in `deploy/rubixd/`
  (8 slices: core, token-auth, systemd, rollback-health, docker, bundles, UI,
  local-publish) and `deploy/rartifacts/` (5 slices: host+ext core, identity/claim,
  publish, resolve, federated UI).
  `containerize-scope.md` (the ask): official **container images for both fleet
  services**, so rartifacts runs as an ordinary cloud workload (**AWS** ECS/EC2) and
  rubixd can run on **docker-only** hosts. Reuses `fly-deploy-scope.md`'s one-image/
  many-drivers mechanism (`rubix-fleet:deploy/common/` + the repo-root `.dockerignore`
  finding) in the `rubix-fleet` repo. The load-bearing decision is **posture**:
  rartifacts containerizes cleanly (env-driven, `0.0.0.0:9410` already its default, one
  `/data` volume for store + blobs), but rubixd's job is driving the **host** — so the
  image mounts `/var/run/docker.sock` (slice-5 docker backend fully live) and the
  **systemd backend degrades to a typed `BackendUnavailable`**, with the systemd unit
  staying the blessed path for mixed hosts. Posture is **probed, never branched** (rule 1:
  no `if container {}`). Also ratifies the **fleet-wide health contract**: one open
  `GET /health` per service — **never `/healthz`**, no `/livez`/`/readyz` — returning
  `200 {"status":"ok",…}` / `503 {"status":"degraded",…}` (the readiness split is the
  status code; connection-refused is the liveness signal), reading in-memory state only and
  **never blocking on a dependency**; a *backend* being unavailable is **not** degraded.
  Product hosts (`rubix-ai`/`ems-node`) have no health route today, so bundles gate on
  `tcp:` until they adopt it — no fleet-plane change when they do. Adds no MCP
  verb/cap/route/table and no product code — one env seam (`RUBIXD_BIND_ADDR`), assets, and
  docs. **All questions decided** (§Decisions, each with a reopen trigger): `debian:
  bookworm-slim`, GHCR + build-only CI, EC2+EBS (not Fargate/EFS — an embedded store wants
  a block device), `amd64`+`arm64` only (**armv7 never** — bare binaries serve it), pinned
  toolchain + committed `Cargo.lock`. Lands in **three waves** (prereqs now → rartifacts
  image with its slice 1 → rubixd image after rubixd slice 5, severable). Assets:
  `rubix-fleet:deploy/`.
  `deploy/rubixd/armv7-scope.md` (the ask): make the **armv7 (Raspberry-Pi-class) target
  real**. `make cross` + `rust-toolchain.toml` have advertised `armv7-unknown-linux-gnueabihf`
  since day one, but it **did not build** — three stacked failures: no C cross-toolchain
  (dies in `psm`), then no **libclang** for RocksDB's bindgen, then RocksDB autodetecting
  `__uint128_t` on the **64-bit host** and compiling for a **32-bit target** where it does
  not exist (`rocksdb/util/fastrange.h`). Fix: build in a container extending lb's
  `docker/build/` cross image (real Debian GCC cross-toolchains — never zig, never the
  `cross` tool) + libclang + `-UHAVE_UINT128_EXTENSION` scoped **armv7-only** via cc-rs's
  per-target `CXXFLAGS_<triple>` (bare `CXXFLAGS` would pessimise the correct 64-bit
  targets). Verified: real `ELF 32-bit … ARM, EABI5` in ~3 min; aarch64/x86_64 unaffected.
  The **CI armv7 gate is load-bearing** — the only 32-bit target is the only one that
  breaks, and it breaks silently while the 64-bit ones stay green. Accepted and documented
  costs: ~26 MB (22 MB of `.text` is RocksDB's C++, post-strip) and a `libstdc++.so.6`
  runtime dep. No `cfg(target_arch)` anywhere — the arch difference lives in build inputs
  only (rule 1). This is also what makes containerize's "armv7 images: never" honest.
  Assets: `rubix-fleet:deploy/common/Dockerfile.cross`, `docs/DEPLOY.md`,
  `.github/workflows/ci.yml`, `Makefile` `cross-*`.
  `deploy/health-route-scope.md` (the ask, issue #72): the **gateway `/health` route** every
  embedder (`rubix-ai`, `ems-node`, `rartifacts`) inherits — one unauthenticated `GET /health` on
  the gateway port (outside the auth wall, like `/login`), implementing the fleet contract
  `containerize-scope.md` already ratified (`200 {"status":"ok","version":…,"detail":…}` serving /
  `503 {"status":"degraded",…}` alive-but-not-serving; `/health` never `/healthz`; one route, no
  `/livez`/`/readyz`). Reads **in-memory state only** — no store query, no disk I/O, no network call
  (a health check that can block is one that lies); the 503 path is an honest `HealthGate` seam a
  FUTURE in-process monitor flips, not a faked store-down detection. Always on when
  `GatewayMode::Addr`; no `BootConfig` field. Closes the recorded `fly-deploy`/`containerize`
  concession (probing `GET /` instead) and lets product-host bundles flip `tcp:` → `http:` in their
  own repos with no fleet-plane change.
- `testing/`, `debugging/` — the standards every session follows.

See `../STAGES.md` for which stage each area lands in and `../STATUS.md` for what has shipped.
