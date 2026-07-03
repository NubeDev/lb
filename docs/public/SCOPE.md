# Project scope (as built)

The trimmed source of truth for what exists now. The full architecture spec is the root
`README.md`; the staged plan is `../STAGES.md`; live status is `../STATUS.md`.

## Shipped (post-S8 — the in-house default agent, finished: model door + shared tool wall + boot)

The always-registered `"default"` runtime is now a working, platform-native agent. Four wiring seams,
no new subsystems:

- **Model door:** the node builds the in-house `ModelAccess` from config (`LB_AGENT_MODEL_*`, the
  `ModelEndpoint` shape — provider/model/api-key-env **NAME**/base-url) as `AiGateway<Provider>` and
  installs it via `Node::install_runtimes`. No model → `UnconfiguredModel` (the honest empty state).
  Config-only, symmetric (no `if cloud`). Key is an env NAME via the secrets path, never logged.
- **Shared tool wall (the load-bearing fix):** the loop's proposed calls
  (`agent/step.rs::run_calls`) route through the ONE host MCP bridge `call_tool` (the `POST /mcp/call`
  entry) instead of registry-only `lb_mcp::call` — so the loop reaches **host-native** verbs
  (`agent.memory.*`/`assets.*`/`series.*`/…) AND extension tools under the identical `authorize_tool` +
  caps wall (`agent ∩ caller`). `skill.activate` stays the loop-internal built-in. The same dispatch
  serves the **external** agent (one path, both fronts platform-capable).
- **Tool menu = reachable catalog:** the loop's `AllowedTool` list is the caller's reachable
  `tools.catalog` (registry + host-native descriptors ∩ grants) — the wall re-checks, so it's a hint,
  not a widening.
- **Boot:** `node/src/agent.rs` builds the registry (default + external entries when the feature is on)
  and calls `serve_agent`, after the gateway key install — closing the serve-wiring TODO.

**No new MCP verbs.** Provider adapter is deferred (ai-gateway scope) — only `MockProvider` exists; the
seam + config + unconfigured→configured **swap** are shipped and proven for real against it. Deny +
workspace-isolation + external parity + offline-resume all tested (rule 9). See
`agent/agent.md` ("The finished in-house default"), `../scope/agent/default-agent-wiring-scope.md`,
`../sessions/agent/default-agent-wiring-session.md`, `../skills/agent/SKILL.md`.

## Shipped (post-S10 — frontend: GenUI — AI-authored dashboard widgets over one generative-UI layer)

One reusable package `@nube/genui` (a versioned, A2UI-*shaped* IR + a trusted catalog renderer + the
OpenUI-Lang authoring adapter — the ONE new external dep `@openuidev/lang-core`) with the
`view:"genui"` dashboard widget as its first tenant. The workspace agent designs a widget from a
natural-language prompt (`agent.invoke` under `skill:core.genui-widget` → run stream → live Lang
preview → **accept parses/normalizes/validates once** → the typed IR persists on the cell); it renders
live with **no model in the render path** — data flows through ordinary v3 `sources[]` via the shipped
`usePanelData`. Two package strata (render vs authoring) on two entries so a viewer never bundles the
parser. **A2UI patterns adopted, Google's packages rejected** (no A2UI adapter in v1). **Trust tier:
in-process** — the scope's "iframe" decision was amended (the shipped `WidgetIframe` sandbox can't host
React; genui widgets are admin-authored so `dashboard.save` is the trust gate; the catalog IR is
trusted data satisfying the 5 promotion-checklist items, CI-tested). The **one** host change is a
validation branch inside `dashboard.save` for `view:"genui"` cells (IR `v` known, ≤8 KB, every
component in the generated catalog JSON) — no new verb/cap/table, giving headless MCP authors the same
loud rejection. A generated skill (`gen:skill` renders the catalog block + the host's
`genui_catalog.json`; CI freshness gate). Tests: package unit ×42, host ×8 (incl. capability-deny +
workspace-isolation), gateway integration ×4 (save→reload→render-without-adapter, save-time rejection,
empty-source v3 round-trip). Deferred with named triggers: the A2UI JSONL adapter, the channel tenant,
IR patch-line refine. See [`genui/genui.md`](genui/genui.md).

## Shipped (post-S10 — frontend: graphics-canvas phases 1–2 — the `thecrew` extension)

The proven playground lifted into a real, publishable **100% UI extension** (`rust/extensions/thecrew/`)
with **zero core additions** — a pure consumer of shipped `assets.*` + `series.*` verbs. Ships a zero-tool
wasm32-wasip2 component (the loader + registry publish path need component bytes; there is no UI-only tier),
a federated `remoteEntry.js` exporting `mountPage` (the full graphics page: palette + canvas + rail + scene
picker/save) and `mountWidget` (a read-only dashboard scene cell, narrower grant — never saves), one build,
React externalised + three.js bundled. Scenes are workspace docs (`assets.get_doc`/`put_doc`/`list_docs`,
`content_type: json`, `scene:` id-prefix + `scene` tag); live values ride one ValueSource multiplexer
(`series.latest` backfill + `series.watch`/poll) under the **viewer's** grant — denied series render a
shape's no-access state, a denied save surfaces the deny. The one declared playground fake (`simulator.ts`)
is **deleted** (rule 9). Saves are last-writer-wins (a client read-before-write conflict prompt is the
interim; a generic `document-store/` revision check is the named ask). Findings recorded, none built:
`list_docs` returns no tags (→ prefix discovery), the member cap set lacks `mcp:assets.put_doc:call` (a real
save needs the install grant), `series.watch` SSE isn't in the gateway-vitest harness (backfill proven live,
watch via stub/Playwright). Real-gateway tests: round-trip, backfill, capability-deny ×2, workspace-isolation,
widget no-access. Parent-scope phases 3–5 (AI drawing + skill, symbol packs, 3D) not built. See
[`frontend/graphics-canvas.md`](frontend/graphics-canvas.md).

## Shipped (post-S10 — frontend: Widget Kit Phase 1 — field presentation + a reusable widget library)

A declarative per-field **presentation** vocabulary (`label`/`description`/`hide`/`order`) authored on the
descriptor and resolved by ONE `resolveFieldPresentation` + `humanize`, applied on BOTH the request form
(`x-lb`) and the response table (`fieldConfig`, with a new additive `hide`) — so `maxRuns` reads "Max Runs"
in both and a header can't drift from a form label. The input widgets + registry + `CronBuilder` + the
presentation/table core were extracted into `ui/src/lib/widgets/` (registry = the public API) as a
behavior-preserving move + re-export shim. One shared table column-model backs both the read-only
`TablePanel` and the row-controlled `ResponseTable`. The `/reminders` table now reads author labels, hides
`principalSub`/`ts`, and no longer dumps `action` as a raw blob (its descriptor declares a `fieldConfig` that
rides the existing rich_result envelope — **no new verb/cap/table/WIT**). `hide` is presentation, **not**
security (a hidden field still crossed the bridge under the viewer's grant). Phase 2 (view-renderer move +
the ext input-widget value channel + `defineWidget`) is a named follow-up. See
[`frontend/widget-kit.md`](frontend/widget-kit.md).

## Shipped (post-S10 — operator CLI `lb`, v1 slice: the terminal twin of the shell)

A fourth client of the node gateway — the terminal twin of the React shell — holding **no authority
of its own**, denied exactly when the server denies (`cli/cli.md`, `../skills/lb-cli/SKILL.md`):

- **One binary, two postures** (`rust/role/cli/`, crate `lb-cli`, binary `lb`): *remote* (a reqwest
  client over the gateway's `POST /mcp/call`) and *local* (an embedded `Node::boot()` + a minted
  `dev_claims`-shaped `Principal`, offline). One `Transport` trait, both ending at
  `lb_host::call_tool`. Mode is a config/flag choice (`--local`), never an `if cloud` branch.
- **v1 commands**: `lb login`/`whoami` (dev-login token, per-workspace, `0600`, never logged); the
  universal `lb call <tool> '{json}'`; one typed `lb inbox list <channel>`; `lb devkit sign` +
  `lb ext publish` (the `make publish-ext` / `lb-pack` fold — same `lb-devkit` signing, so artifacts
  verify by construction; `lb-pack` kept as a shim); `lb local …` (the offline/edge posture).
- **Pure client, zero new surface**: no new MCP verbs, capabilities, tables, or enforcement paths.
  `-w` is a **credential selector** (an unstored workspace is a loud error), never a workspace
  override — the token's ws always reaches the server (`/mcp/call` carries no ws in the body). Every
  command prints a `ws/user/role/mode` header (stderr) + shaped body (stdout); a deny renders
  `DENIED  mcp:<tool>:call` and exits non-zero — never a fabricated success.
- One host-crate touch: `lb_auth::claims_unverified` (a client-side header read; not an authz path —
  the server re-verifies every request). New deps: `clap`/`tabled`/`dirs` (CLI ergonomics only).
- **Tested** in-process against a real gateway on a real socket, seeded via the real write path (no
  mocks): capability-deny + workspace-isolation + offline, remote AND local; the `devkit sign` →
  `verify_artifact` round-trip; config `0600` + "no command emits the token" (`rust/role/cli/tests/`).

## Shipped (post-S10 — global identity / many-workspaces, the Slack model)

One **global identity** per person belonging to many workspaces, linked by a per-workspace
**membership** roster — the prerequisite that makes the Access console, teams, and the workspace
switcher behave the way README §7/§6.6 already specify (`auth-caps/auth-caps.md`):

- **Identity directory** — `identity:{sub}` = `{sub, display_name?, created_ts}` in a reserved
  system namespace `_lb_identity` (mirroring `_lb_workflow_directory`). Hub-writable,
  resolution-read-only, **no tenant data, no credential** (dev-login behind the seam; OIDC additive
  later). `sub` stays the human handle (`user:ada`), globally unique — `Subject::User(sub)` grants
  unchanged. `lb_authz::identity` raw verbs + host `identity` service (`create/get/list/workspaces`,
  each gated `mcp:identity.manage:call`).
- **Membership roster** — `membership:{sub}` = `{sub, joined_ts}` in the workspace namespace; the
  single source of truth for "who is in this workspace" (no `role_hint`). `lb_authz::membership` raw
  verbs + host `membership` service (`add/remove/list`, gated `mcp:members.manage:call`). `add`
  system-grants `role:member`; `remove` tombstones + **composes** `revoke_subject` + `token_revoke_mark`
  (clean exit — live token refused next verify); `list` is the **effective** roster (membership ∪ legacy
  `user:*` rows, lazy migration).
- **Login + bootstrap** — login resolves identity → memberships → the existing `(sub, ws, caps)` token;
  an effective member mints, an empty workspace bootstraps the requester as `workspace-admin`
  (decision #3), a non-member of a workspace that has members is refused (decision #4).
  `create_workspace` auto-memberships the creator + grants `workspace-admin`.
- **Surfaces** — gateway routes (`/admin/identities*`, `/admin/members*`) + `http.ts` entries; the
  Access console "People" tab re-points `user_list` → `membership.list` (decision #9); the workspace
  switcher resolves through `identity.workspaces`. The token/cap grammar is unchanged.

**Tests (real store/gateway, no mocks):** host `identity_membership_test` 7 (deny-per-verb ·
ws-isolation across store+MCP · one-identity-in-N-workspaces · login/zero-memberships ·
leave-is-a-clean-exit · legacy-migration no-access-change · removed-tombstone-replays-idempotently) +
gateway `identity_routes_test` 5 (forged-call-denied · create+add+list+workspaces · login
bootstrap+refuse · clean-exit live-token-refused) + UI `Membership.gateway.test` 4 + the re-pointed
`PeopleAdmin`/`DocView` suites green. Scope + session: `../scope/auth-caps/global-identity-scope.md`,
`../sessions/auth-caps/global-identity-session.md`.

## Shipped (post-S8 — reminders: a scheduled trigger that fires one action)

A durable, workspace-scoped **reminder** — a `reminder:{id}` record + a dedicated durable scan over
the shipped `lb-jobs` + outbox, **not** a new scheduler (`reminders/reminders.md`):

- **The record (`lb-reminders`).** A 5-field cron `schedule` (the storage format), `max_runs`/`runs`
  (one-shot/counted/recurring), an `enabled` on/off switch, a `next_attempt_ts`, a stored
  `principal_sub`, and an `action` tagged union — **one** action: channel post / MCP tool call /
  must-deliver outbox effect. The store crate holds no authorization (like `lb_inbox`/`lb_outbox`);
  the one new mechanical piece is cron "next after T" on the **injected logical clock** via `croner`
  (`find_next_occurrence` on a supplied time — deterministic, never wall-clock).
- **CRUD `reminder.*`** — `create`/`update`/`delete`/`get`/`list`, one tool + capability + file each
  (`mcp:reminder.<verb>:call`), bounded single-record writes → **synchronous, not jobs** (the firing
  is the job). `update` covers pause/resume (`enabled`) and reschedule (re-anchoring `next_attempt_ts`
  to the next future slot — no backfill on resume).
- **The reactor (`react_to_reminders`).** A dedicated scan in its own file (same altitude/cadence as
  `react_to_approvals`/`relay_outbox`): finds every enabled+due reminder, enqueues ONE
  `kind="reminder-fire"` `lb-jobs` job per firing (**deterministic per-firing id** on
  `(reminder_id, scheduled_ts)` → idempotent, one instant → one job → one effect), dispatches the
  action against its real seam, and advances (`max_runs` countdown to `Done`, else next future slot).
  Missed-firing policy is **fire-once-then-skip-to-next** (no backfill storm after an outage).
- **The security model — principal capture at fire time.** The record stores `principal_sub`, not a
  frozen cap set; the firing **re-resolves** caps from the durable grant store and re-checks the
  action's own capability (`bus:chan/{c}:pub` / `mcp:{tool}:call` under the live schema /
  `mcp:outbox.enqueue:call`). A grant revoked after create turns the firing into a **logged deny**
  with no effect; the reminder is left scheduled, the existing job id keeps a re-scan from re-firing.
- **UI** — a `react-js-cron` visual cron builder (antd scoped to its own subtree, never the global
  Tailwind/shadcn theme), an action editor, and a `reminder.*` api client over the host-mediated
  `POST /mcp/call` bridge (ws + principal from the token).

**Tests (real store/bus/jobs/inbox/outbox, no mocks):** `lb-reminders` cron-math units +
`reminders_mcp_test.rs` (CRUD round-trip, **deny-per-verb**, **ws-isolation** at list/get, bad-cron =
`BadInput`) + `reminders_reactor_test.rs` (each action kind against its real seam, **idempotent**
re-scan, `max_runs`→`Done`, disable/resume, **offline catch-up exactly-once**, **deny at firing** on a
revoked grant, **ws-isolation** across store + reactor, multi-day cron on the injected clock). See
`reminders/reminders.md` + `../sessions/reminders/reminders-session.md`.

## Shipped (core crate — `lb-prefs`: canonical-in / localized-out preferences, units & formatting)

The boundary that renders canonical data per a principal's resolved preferences (domain data stays
canonical — UTC instants, base units, neutral enums — never a formatted string).

- **`lb-prefs` crate** — the closed axis set (8 dimensions, 29 units), nullable `user_prefs:[ws,user]`
  + `workspace_prefs:[ws]` SCHEMAFULL records, the pure resolution fold (request → user → ws-default →
  built-in, each axis independent), `uom`-backed conversion (affine °C↔°F; cross-dimension rejected),
  and locale/tz formatting (separators + date order + 12/24h from the axes; tz over a UTC instant incl.
  DST via `chrono-tz`).
- **MCP surface** — gated `prefs.get/set/resolve/set_default` (OWN self-scoped; `set_default` admin)
  + a **grant-free** utility tier `format.datetime/number/quantity` + `convert.unit` (pure math, no
  tenant data). Gateway routes mirror them 1:1. Closed vocabulary generated to the client
  (`ui/src/lib/prefs/dimensions.generated.ts`).
- **i18n catalogs (Phase 2, shipped):** a hand-written **ICU MF1 subset** parser/renderer in
  `lb-prefs` (plural `one`/`other`+`=0`/`=1`, `select`, typed date/number/quantity placeholders routed
  to `format::*`, one nest, `#`); built-in en/es `.mf` catalogs compiled in + **generated to the client**
  (`catalog.generated.ts`, byte-identity drift-tested; `intl-messageformat` cross-checked host==client);
  a sparse per-workspace override `message_catalog:[ws, locale]` (per-key merge, LWW, offline-idempotent);
  and 3 MCP verbs — `message.render` (member for self, `message.render_recipient` grant to fan out for
  another), `prefs.catalog` (member), `message.set_catalog` (admin) — routes 1:1, + the "catalog changed"
  hint. **Server-side per-recipient fan-out**: one event → N distinct renders (per member's prefs).
- **Settings UI (shipped 2026-07-02):** a dedicated **Settings** nav surface — a **Preferences** tab
  over all eight axes (`prefs.set` own + admin `prefs.set_default`, unit picker generated from the
  server vocabulary) and an **Agent** tab (below). See `prefs/prefs.md` → "Settings UI".
- **Deferred (named):** the `icu4x` swap-in (localized names + full-CLDR plural engine + en/es CLDR
  slice) behind the same signatures; the **pre-auth bootstrap-locale** + RTL UI (the settings *editor*
  is shipped).

See `prefs/prefs.md`, `../scope/prefs/{user-prefs,i18n-catalogs}-scope.md`,
`../sessions/prefs/{lb-prefs,i18n-catalogs}-session.md`.

## Shipped (per-workspace agent config — the workspace's default runtime + endpoint)

The `agent.runtimes` read verb *lists* what a node offers; this persists a workspace's **choice**.

- **Record:** `workspace_agent_config:[ws]` (SCHEMAFULL, composite-id, idempotent replay) — nullable
  `default_runtime` (registry-validated on write) + a **names-only** `model_endpoint`.
- **Verbs:** `agent.config.get` (member, `mcp:agent.config.get:call`) + `agent.config.set` (admin,
  `mcp:agent.config.set:call`); gateway `GET|PUT /agent/config` (1:1). Opaque deny; no delete/feed/batch.
- **UI:** the Settings **Agent** tab — a runtime dropdown (from `agent.runtimes`) + endpoint fields,
  editable for an admin, read-only for a member (server is the wall); registry-drift flagged.
- **Follow-up (named):** honor the stored default in `agent.invoke` when `runtime` is omitted.

See `external-agent/external-agent.md` → "Agent config", `../scope/external-agent/agent-config-scope.md`,
`../sessions/external-agent/agent-config-settings-session.md`.

## Shipped (S9+ — frontend: the rules workbench — Playground · chain canvas · datasources admin)

Three cap-gated shell pages over the **already-shipped** `rules.*`/`chains.*`/`datasource.*` host verbs,
mirroring the dashboard surface verb-for-verb — gateway routes + UI api clients + the React surface, no
host work added (`public/frontend/rules-workbench.md`):

- **Playground** — a CodeMirror editor + `rules.run` rendering the typed `RuleOutput` three ways
  (scalar/grid/findings) + log + ms/ai budget; full `rules.*` CRUD rail; honest cage/deny/AI-budget/
  AI-not-configured states (`BadInput` verbatim, `Denied` opaque), never a fake result.
- **Chain canvas** *(RETIRED — superseded by the Flows canvas; `chains` is deleted, `flows` is the one
  DAG engine — `scope/flows/chains-retirement-scope.md`)* — was a React Flow DAG (nodes=steps,
  edges=needs) over `chains.*`; a cyclic edge rendered the host's validation error inline; Run + a
  bounded `chains.runs.get` settle-poll coloured nodes. The DAG capability now lives in **Flows**
  (`flows.*`, live `flows.watch` SSE instead of a poll).
- **Datasources admin** — first-party shell page over `datasource.*` (the federation extension stays
  headless): list (redacted, never the DSN), add (DSN write-only, implied grants shown), test (honest
  green/red probe), remove.
- The gateway re-checks every cap server-side (via `lb_host::call_tool`); workspace + principal from the
  token; per-verb deny + two-session isolation + the DSN-redaction assertion all tested on a real
  in-process gateway. Fixed a shipped host bug along the way (`rules.list`/`chains.list` dropped every
  row by not unwrapping the store envelope).

## In progress (S10 — extensions: SDK / built-in Studio)

The extension SDK is implemented behind `lb-devkit` and the local-only `devkit.*` host bridge:
shared artifact signing with `lb-pack`, root-safe scaffolding under `LB_DEVKIT_ROOT`, real cargo/pnpm
builds behind one `Toolchain` trait, durable build jobs streaming logs over the existing generic bus,
and a built-in Studio page that drives scaffold → build → publish. `POST /extensions` also accepts a
devkit path and signs server-side from `LB_DIR/keys/dev-publisher.key`, reusing the existing
`ext.publish` trust/install path.

Current state: focused devkit/pack tests, host devkit/e2e tests, the focused Studio real-gateway test,
TypeScript, and `pnpm test` are green. The full workspace/gateway gate is blocked by an unrelated
active `agent/run.rs` compile break (`crate::run_events` unresolved) and is not promoted to shipped yet.

## Shipped (S10 — agent-run: a streamable, externally-drivable, interactively-gated run)

The agent run is now a first-class, observable, interruptible, replayable object (details:
`agent-run/agent-run.md`). **Durable typed transcript** (`lb_jobs::TranscriptEvent`, versioned) so
resume *rehydrates* and continues the conversation instead of re-asking from the goal; **one
`RunEvent` vocabulary** (`lb-run-events`) derived from that transcript (live == replay); **`agent.watch`
+ `GET /runs/{job}/stream`** for a snapshot-then-deltas live feed; **per-tool Allow/Deny/Ask** with a
durable **first-settle** `agent_decision` record (via `lb_store::create`, not the last-writer-wins
inbox `Resolution`) settled by `agent.decide`; an **ACP stdio adapter** (`role/acp`/`lb-acp`) with
trusted-session auth and a durable disconnect-mid-permission contract; and **model-activated skills**
gated by the S4 grant. All six parts tested against real store/bus/gateway/spawned-ACP (only the LLM
provider is stubbed). `cargo test --workspace` green except a pre-existing cross-node Zenoh flake.

## Shipped (S10 — telemetry console: in-store capped sink + in-browser viewer)

The platform is now **self-contained for everyday observability** (details:
`observability/observability.md`) — no external monitoring stack needed to see what a tool just did.
A reusable **`lb_store::capped_insert`** primitive keeps the newest *N* rows per a caller-chosen FIFO
key (per-source or global) in one SurrealDB transaction; it is correct under concurrency via the
transaction + a per-key in-process lock + a bounded retry on SurrealDB's retryable conflict (the
concurrency test lands exactly `cap`, deterministically). A **`SurrealCappedLayer`** `tracing`
subscriber (peer to stderr/OTLP, config-selected by `LB_TELEMETRY_SINK`, no `if cloud`) writes the
redacted event schema (`params_digest` only — a planted secret reaches zero rows) to a capped
`telemetry` table and mirrors onto a ws-walled tail subject. A gated **`telemetry:read`** MCP surface
(`telemetry.query`/`trace`/`tail` + node-admin `telemetry.purge`), **hard-filtered to the caller's
workspace server-side**, feeds an in-browser **console** with composable filters (source/actor/level/
outcome/trace_id/text/time), a live tail over `GET /telemetry/stream`, a click-trace→timeline pivot,
and a second **Audit lane** that reads the immutable mutation ledger as a *separate* store (degrades
to a labelled empty state until audit ships — never fake rows). There is no `telemetry.write` (writes
come from the Layer only) and cross-tenant operator reads are a separate, higher capability. Tested
against real store/bus/gateway (capped FIFO + concurrency, deny-per-verb, workspace-isolation,
planted-secret redaction, filter narrowing, trace correlation, and a live SSE row over a real spawned
gateway). The OTLP peer sink + cross-hop trace propagation + metrics (the **emit** half) remain scoped.

## Shipped (S10 — host tools: built-in `host.*` node introspection)

The host now exposes a read-only, backend/agent-facing `host.*` MCP family
(`public/host-tools/host-tools.md`) for structured node-local facts without shelling out:
`host.net.info`, `host.net.reach`, `host.time.now`, `host.time.zones`, `host.fs.stat`, and
`host.fs.list`.

Implementation is host-native: one `host.` arm in `tool_call.rs`, delegating to
`host_tools::call_host_tool`, which mirrors the `agent.*` dispatcher and runs the standard
`authorize_tool(principal, ws, verb)` gate per verb. There is no registry, no new dispatch path, no store
or bus state, and no dedicated UI panel in v1. Existing agents, extension callers, and UI bridge callers
can use the verbs through generic MCP when granted.

Safety boundaries: filesystem verbs return metadata only (no contents, no writes, no workspace doc
assets); `host.fs.list` is one level, sorted, capped at 1000 entries; `host.net.reach` is TCP-only,
single-host/single-port, default 2s with a server hard cap of 5s via `connect_timeout`. DTO shape is
identical across OSes, with interface enumeration isolated in `host_tools/net/platform.rs` and path
normalization in `host_tools/fs/path.rs`.

Tests use real infra and real OS facts: six capability-deny tests, other/empty-workspace gate denial,
cross-platform DTO allow-list checks, a real temp directory for `host.fs.*`, a real loopback
`TcpListener` for `host.net.reach`, bounded-probe assertions, port-range refusal, and leak-nothing
allow-lists.

## Shipped (S10 — dashboard: widget config + a Grafana-style variable system + generic bus pub/sub)

The dashboard gains a Grafana-style variable system + widget settings, on a shared interpolation library
extensions reuse, plus the one new backend API the sinks need (`public/frontend/dashboard.md`):

- **Widget settings.** `Cell.title` (additive serde, no new verb) + a per-cell ⚙ drawer reusing the
  builder fields in edit mode; gated `mcp:dashboard.save:call`.
- **The shared `vars` library** (`ui/src/lib/vars/`, pure TS, federation-shared, `VARS_LIB_V`):
  `interpolate` (the three Grafana syntaxes + format hints + multi-value + unknown-left-literal),
  `interpolateArgs` (deep, type-preserving — generalizes the control `{{value}}`), `resolveBuiltins`
  (pure, token/range-derived `$__from`/`${__user.*}`/`${__workspace}`/…), `extractVarNames`.
- **Variables.** `Dashboard.variables[]` (additive serde) + a variable bar + editor; **selection lives in
  the URL** (`?var-<name>=`), definitions on the record. Query options resolve over the leashed bridge.
- **Interpolation everywhere + ctx.vars.** Every cell call interpolates its args against the resolved
  scope; the widget ctx gains `vars`/`timeRange` (additive v2). Identity is shell-resolved (un-spoofable).
- **Auto-refresh + live.** A URL-synced refresh picker (tab-hidden pause) re-resolves vars + re-runs reads
  (state); `bus.watch`/`series.watch` stream motion; they compose.
- **JSON payload builder.** A JSON template with `${var}`/`{{value}}` slots + a target picker → the leashed
  bridge; a `bus.publish` shows "published", never a fake "delivered".
- **Generic `bus.publish` / `bus.watch`** (the platform fix): workspace-walled, capability-gated subject
  pub/sub (`crates/host/src/bus/`, one verb/file), mirroring `series`/`ingest`. The subject is namespaced
  `ws/{id}/ext/{subject}` host-side from the token; reserved prefixes (`series/`/`channels/`/internal) and
  cross-ws names are refused. `POST /bus/publish` + `GET /bus/stream?subject=&token=` (auth-first 401/403);
  `bus.publish` also over `POST /mcp/call`. Fire-and-forget motion (rule 3) — must-deliver goes via the outbox.

Mandatory tests (real infra, no mocks): capability-deny per verb, two-workspace isolation (a ws-B
`bus.watch`/`bus.publish`/variable query reaches only ws-B; a reserved/cross-ws subject refused),
identity-un-spoofable, the interpolation contract (every syntax/hint/built-in), URL round-trip, auto-refresh
cadence, and a live publish→watch SSE round-trip. See `frontend/dashboard.md`.

## Shipped (S9/S10 — frontend: theme switcher)

The React shell now ships a maintained local theme preference layer (`public/frontend/frontend.md`):
explicit **dark/light** mode plus three token-bound accent palettes (**amber** default, **teal**,
**blue**) selected from a shadcn-style switcher in the sidebar footer. Preferences are validated,
stored in browser/webview `localStorage` (`lb.theme`), applied before React mounts to avoid first-paint
flash, then owned by `ThemeProvider`/`useTheme`.

The theme contract stays CSS-variable based (`bg`/`panel`/`fg`/`muted`/`accent`/`border` and shadcn
aliases), so existing pages, primitives, graphs, and extension UI inherit the palette without
component branches. Accent contrast against the base background clears AA in light and dark. Tests:
`pnpm test` 56/56, `pnpm test:gateway` 110/110, `pnpm build` green, `pnpm lint` 0 errors (legacy
allowlist warnings remain).

## Shipped (S10 — extension UX: a self-contained extension over real Module Federation)

An extension is now **one folder = backend + frontend**, each part optional (`public/extensions/extensions.md`).
The reference `fleet-monitor` (`rust/extensions/fleet-monitor/`) ships **both** halves together:

- **Backend** — a **native Tier-2 sidecar** with its own PID, supervised over `Content-Length` stdio via
  the shared `lb-supervisor` wire types (no ABI drift), exposing the `fleet.summary` MCP tool. Stateless;
  the injected `LB_EXT_WS` identity reaches the child (proven by a real `OsLauncher` e2e test).
- **Frontend (co-located, `<id>/ui/`)** — a **real Vite Module Federation remote**
  (`@originjs/vite-plugin-federation`) exposing `mount(el, ctx, bridge)` and sharing the shell's
  `react`/`react-dom` **singletons** (not a hand-rolled `import()` of a self-bundled ESM — so it renders
  in-process against the *same* React, native-feeling). Real **shadcn/ui + Tailwind** (token-matched to the
  shell). A cap-gated sidebar **page with 3 nested routes** + **2 declared dashboard widgets**.
- **The seam** — the shell is the federation **host** (`ui/vite.config.ts` shares React; `ext-host/federation.ts`
  loads a remote by gateway URL; `ExtHost` mounts the federated container). Data reaches the UI ONLY through
  the host-mediated `bridge.call → POST /mcp/call`, cap + workspace re-checked per call; the page never holds
  the token/DB. The gateway serves the remote at `GET /extensions/{ext}/ui/{*path}` (traversal-guarded).
- **Contract** — the manifest `[widget]` block became **`[[widget]]`** (`widgets: Vec` end to end — manifest →
  `Install` → `ExtRow` → `ext.api`); the **native** install path now persists `[ui]`/`[[widget]]` (shared
  `crates/host/src/ui_decl.rs`, scope-narrowed to the grant) — it silently didn't before. The wrong split
  `ui/extensions/hello-ui` was deleted; the fake dependency was removed from page discovery (real `ext.list`
  only). A pre-existing `cargo build --workspace` failure (`test_gateway_seed` stray-bin) was fixed.

**Tests (real infra, no mock node):** 3 backend + 12 manifest + 2 `ui_decl` + 3 `ext_ui` + **2 native e2e**
(real child spawns + page/2-widgets surface in `ext.list`, scope-narrowed) + **6 extension-UI Vitest** + 20
shell + **50 real-gateway** (incl. the `fleet-monitor` page slot + both widget tiles round-tripping through a
real `Install`). `cargo build/test --workspace` + fmt + FILE-LAYOUT size check green. Capability-deny and
workspace-isolation categories covered (the bridge denies an ungranted tool; the page/widget scope is narrowed
to the grant; the ws comes from the token, never the page). Widget *rendering* in a cell + the untrusted iframe
tier are explicit follow-ups. See `extensions/extensions.md` and
`../sessions/extensions/fleet-monitor-federation-session.md`.

## Shipped (S9+ — data console: DB browser + ingest explorer)

Two workspace-scoped, capability-gated shell pages for non-SQL users, on the shipped S8 data plane
(`public/frontend/data-console.md`):

- **Data page — the admin, READ-ONLY DB browser.** A new, deliberately small generic store-read surface
  (`lb_store::{tables,scan,graph}`, host `dbview` service): `store.tables` (tables + exact counts),
  `store.scan(table, limit, cursor)` (a **hard-capped** id-cursor page of raw rows), `store.graph(table?,
  id?, depth)` (depth-1, fan-out-bounded nodes + real relation edges for react-flow). Over `/store/*`
  gateway routes. UI: table picker → paged row grid (row-expand→JSON) → a **code-split** react-flow
  relation graph (`@xyflow/react`). **The security decision:** these verbs relax the per-record membership
  gate (gate 3) — a raw scan answers "every record in the workspace" — so they are **admin-only**
  (`mcp:store.*:call` to the ws-admin role, never `member_caps`) and **read-only**. The workspace wall and
  the capability still hold hard; a member never sees the Data nav entry.
- **Ingest page — the series explorer.** The S8 `ingest.*`/`series.*` verbs finally reachable over the
  gateway, plus a new small `series.list(prefix)`: list/search series, latest + recent samples (rendered
  by payload type), and a manual `ingest.write` form (`POST /ingest` writes-then-drains so the sample is
  instantly visible; producer = the authenticated principal).
- **The real-gateway test harness.** Both the Rust route tests and the UI tests run against a **real node**
  (`mem://`), seeded with real rows through the real write path — **no fake backend** (CLAUDE §9). The
  slice built the first real-node Vitest harness (`test_gateway` bin + `vitest.gateway.config.ts`,
  `pnpm test:gateway`), the start of retiring the `*.fake.ts` layer. 7 Rust route tests (deny-per-verb +
  ws-isolation) + 7 UI real-gateway tests green.

## Shipped (S8 — data plane: durable store · generic ingest · typed tag graph)

The first stage that **writes to disk**, in three gated slices (`public/{store,ingest,tags}/`):

- **Persistent store + the GO/NO-GO spike (the gate)** — `Store::open(path)` on the pinned **SurrealKV**
  engine alongside `Store::memory()`; both engines compile into every node, the constructor chosen by
  `LB_STORE_PATH` (config, **no code-branch** — symmetric nodes). A permanent hermetic
  **capability-spike matrix** classifies each SurrealDB feature LOAD-BEARING vs DEGRADABLE: all five
  LOAD-BEARING ✓ → GO; `DEFINE BUCKET` ✗ (→ record-as-content), SEARCH/HNSW ✓, materialized view
  defines-but-doesn't-populate, LIVE ✓. A subprocess **crash set** (SIGABRT) proves kill-mid-tx →
  rollback and flush-burst → last-commit-survives. See `store/store.md`.
- **Generic ingest (`lb-ingest`)** — the read-side analog of the outbox. A `Sample{series,producer,ts,
  seq,payload,labels,qos}` firehose lands as a cheap durable **append** to staging, then a commit worker
  drains **one transaction per batch**: UPSERT into `series` on `[series,producer,seq]` + delete-staged
  in the same tx → **atomic + exactly-once on re-drain** (proven across a kill-mid-commit). Dedup
  identity is `(series, producer, seq)` with `producer` = the authenticated principal, so a fleet writes
  one series without collision. Overflow drop-oldest/dead-letter. **No device/sensor/MQTT in core** — a
  producer is a principal. `ingest.write`/`series.read`/`series.latest`/`series.find` MCP verbs. See
  `ingest/ingest.md`.
- **Typed tag graph (`lb-tags`)** — a tag is a shared typed node `tag:[key,value]`; applying it is a
  `(entity,tag,source)` provenance edge (same-source upserts, different sources coexist).
  `tags.add/remove/of/find` (exact/key-only/faceted intersection) + a required per-workspace **tag-node
  cap**. Spike-gated add-ons shipped: **BM25 full-text**, **HNSW vector** (dimension pinned, mismatched
  dims rejected), **per-dimension counts** (per-query, since the materialized view doesn't populate on
  SurrealKV). `series.find` discovery is built on it. See `tags/tags.md`.

Mandatory tests across all three: capability-deny, two-workspace isolation (store + MCP; tags uses the
**identical** `tag:['region','eu']` in both workspaces), offline/restart. 284 workspace tests green.

## Shipped (S7 — frontend: make collaboration real — identity · workspaces · channels · people · inbox/outbox)

The UI became a **real collaboration app over a real session** (the channel's 4-file move, ×5):

- **A real identity session (the keystone)** — the gateway's demo principal is **deleted**. `POST
  /login` mints a signed `lb_auth` token (a dev credential store, the token path real); **every**
  route `verify`s the bearer token and derives the principal — **workspace + caps from the token, not
  the request** (§7). Missing/forged/expired → `401`; ungranted → `403`. SSE authenticates by a
  `?token=` query param. UI: `lib/session/` + `useSession`; `App.tsx`'s hardcoded identity is gone.
- **Workspaces / channels / members / inbox / outbox** — five new host services, each gated and
  workspace-scoped, mirrored 1:1 by a gateway route and a `features/<x>/` view: `channel_registry`
  (list/create + create-on-post), `workspaces` (directory), `members` (list/add over S4 edges),
  `inbox` (the **real** `lb_inbox` queue — replaces the workflow fake on the real path; Approve =
  the S6 gate), `outbox_status` (read-only). **Presence is rendered** (`usePresence` roster).
- **Two real sessions** make the workspace-isolation test real — ws-B sees none of ws-A.
- Tested: `crates/host/tests/collaboration_test.rs` + the gateway suite (session / deny / two-session
  isolation / registry / real inbox / outbox pending→delivered / live SSE) + a Vitest view per
  surface. See `frontend/collaboration.md`, plus the standalone shipped docs
  `workspace/workspace.md` and `channels/channels.md`.

## Shipped (S7 — platform maturity: the outbox egress — real GitHub `Target` + backoff/dead-letter)

The transactional outbox's **outbound** edge, completed and hardened (two of its listed follow-ups):

- **A real GitHub `Target`** — `lb-role-github-target` delivers `create_pr` / `comment` effects to the
  GitHub REST API over `reqwest` (the in-test target was the only stub; the egress counterpart to the
  webhook ingress, `reqwest` in the role crate, never core). `create_pr` is idempotent via GitHub's
  own `422 "already exists"` — a re-delivery is acknowledged, never a second PR. The token is mediated,
  never logged. A permanent mapping fault (unknown action, bad payload) is distinguished from a
  transient transport failure.
- **Backoff + dead-letter** in `lb-outbox` + the host relay (the outbox scope's top open question,
  answered): each `Effect` carries `max_attempts` (default 5) and `next_attempt_ts`; on failure the
  relay applies an exponential, capped backoff, and at the cap moves the effect to a terminal
  `DeadLettered` status (parked, off the schedulable set, readable via `dead_lettered`). The relay
  scans `due` (schedulable AND past the backoff gate), not `pending`.

**11 new** tests green — +2 outbox (backoff gate · dead-letter at the cap) + 9 github-target (5 unit
action-mapping/permanent-error + 4 integration over a real socket: happy 201 · 422-idempotency ·
dead-letter through the adapter · transport-failure-then-recovery). 8 host workflow tests updated to
the new `relay_outbox(.., now)` / `mark_failed(.., now)` signatures and green. No SDK/WIT or
capability-grammar change. See `inbox-outbox/inbox-outbox.md` and
`../sessions/coding-workflow/outbox-egress-session.md`.

## Shipped (S7 — platform maturity: the GitHub webhook-receiver role crate)

The live HTTP ingress for the coding workflow's inbound edge — resolving the explicit follow-up the
`github-bridge` slice left (it shipped `ingest_via_bridge` as a host helper a test/UI drove). New:
**`lb-role-github-webhook`** (beside `lb-role-registry-host`; roles depend on host, never the reverse),
a node that also exposes `POST /webhook`. It adds no authority; two layers guard it, in order:

- **Transport authenticity** — `HMAC-SHA256(secret, raw-body)` against `X-Hub-Signature-256`, compared
  in **constant time** over the **raw bytes** GitHub signed (verifying re-serialized JSON never
  matches). A failure is an opaque `401`; the mediated, crate-private secret is never logged. The legacy
  SHA-1 header is not accepted.
- **Capability + workspace** — a verified delivery calls `ingest_via_bridge` under a fixed
  principal/workspace, so the SAME two gates (`mcp:github-bridge.normalize:call`, then
  `mcp:workflow.ingest_issue:call`) and the workspace wall apply. An authentic-but-ungranted delivery is
  `403`, distinct from the `401` forgery case. Re-delivery is idempotent (one inbox item).

**12 new** tests green — bad-signature (forged / tampered / absent → `401`, ingests nothing),
capability-deny (`403`), workspace-isolation (a ws-A receiver never writes ws-B), idempotent
re-delivery, the happy path over both `tower::oneshot` and a real socket, malformed→`422`, plus the
HMAC verifier units — all through the real `github_bridge_ext.wasm`. `axum`/`hmac` live in the role
crate, never core. No SDK/WIT or capability-grammar change. See `extensions/extensions.md` and
`../sessions/extensions/github-webhook-session.md`.

## Shipped (S7 — platform maturity: the `github-bridge` as an installed wasm artifact)

The S6 `github-bridge` deferral, resolved: the coding workflow's inbound edge ships as an installable,
signed **Tier-1 wasm extension** — the second real extension proving the registry install lifecycle end
to end (the first was `hello`). The shape was decided deliberately (with the user) against two walls:

- **The orchestrator stays a host service.** It drives host-internal seams (`caps::check`, the S5 agent
  loop, durable jobs, the outbox) a sandboxed guest reaches only *through* MCP — so only the inbound
  *normalizer* is packaged, not the workflow engine.
- **A pure-transform bridge, no ABI change.** The stable WIT world imports only `host.log` (no
  host-tool-call import — adding one is a major-bump-class change to the forever ABI, README §11.2). So
  the wasm guest only **normalizes** a raw GitHub webhook → `{ issue_id, payload, ts }` over the existing
  `tool.call` export; the **host** (`ingest_via_bridge`) composes that with `workflow.ingest_issue`. The
  split lands on the trust line: pure transform → sandbox; must-deliver state write → host.

New: `rust/extensions/github-bridge/` (the wasm crate) + `lb_host::ingest_via_bridge` (the host
composition; two independent gates — `mcp:github-bridge.normalize:call`, then
`mcp:workflow.ingest_issue:call`). **7 new + 19 regression** tests green — install-deny, workspace
isolation (the node-global stateless instance is shared; the wall is caps + store — see the debugging
entry), offline-from-cache, rollback, and the transform branches, all through the real
`github_bridge_ext.wasm`. See `extensions/extensions.md` and `../sessions/extensions/github-bridge-session.md`.

## Shipped (S7 — platform maturity: the native Tier-2 supervisor)

The second S7 vertical slice — the **remaining half of the S7 exit gate**: a native OS-process
sidecar is **supervised and restarts cleanly**, beside the wasm tier, under one control plane and one
identity model. Built by composing the supervisor seam with the S4 install + the capability model,
not by forking a second extension system.

- **`lb-supervisor`** — the OS plumbing + supervision policy, behind a `Launcher` seam (the registry's
  `Source` analogue): spawn a child, frame `Content-Length` JSON-RPC over its stdio, handshake
  (`init`), health-poll, cooperative `shutdown` (escalating to a process-group kill), and `restart`
  (kill + relaunch from the spec, bounded by exponential `Backoff` + a `max_restarts` budget). Holds
  no store/auth/identity — the host drives it. See `extensions/extensions.md`.
- **The host `native` service** (beside `agent`/`channel`/`assets`/`workflow`/`registry`) —
  `install_native` (persist the S4 `Install` record → spawn → record status), `stop`/`restart`/
  `status`, and `call_sidecar` (dispatch a child tool, **restart-on-fault** and retry — the
  supervision crash-path). The live `Sidecar` (PID, stdio) lives in a **runtime-only** `SidecarMap`
  keyed `(ws, ext_id)`; the durable truth is the `Install` record + a `native_status` projection
  (lifecycle intent + restart count) in the workspace namespace — so a restart re-derives from the
  record and **loses no durable state** (the stateless-extension guarantee carried into Tier 2).
- **The `[native]` manifest block** — `tier="native"` carries exec/args/target/restart (closing the
  extensions-scope deferral); the host turns it into a supervisor `Spec` and injects the child's
  **scoped identity token** (`requested ∩ admin_approved`, the same intersection the wasm tier grants).
- **Two gates, unchanged** — the **capability** gate (`mcp:native.<verb>:call`, workspace-first, the
  proven host-service gate — **no `process:` grammar change**) and, for a registry-installed sidecar,
  the **signature** gate (`verify_artifact`). A signed `tier="native"` artifact installs through the
  same pull→verify→cache flow a wasm one does (`install_native_from_registry`); a tampered native
  artifact is rejected before the binary touches disk.
- **A reference sidecar** (`echo-sidecar`) — a real host-platform binary (a workspace member, unlike
  the wasm `hello`) speaking the supervisor's wire types verbatim (the child↔host ABI cannot drift).
- **UI** — a `NativeView` + `native.api` client mirroring the verbs, surfacing the **restart count +
  running flag** (the supervision, visible), with a faithful in-memory fake. See `frontend/frontend.md`.

**Exit gate — fully MET.** A native sidecar is supervised and restarts cleanly: a killed child is
respawned (proven with a **real OS process**), resumes answering, and **no durable workspace state is
lost** (a channel message posted before the crash is intact after); install/lifecycle are
capability-gated (no spawn without `mcp:native.install:call`); ws-B can never see or control ws-A's
sidecar (store + MCP + the runtime map); a signed native artifact installs through the registry and a
tampered one is rejected. Posture: process-group isolation + scoped identity + bounded restart;
OS-level hardening (cgroups/seccomp/userns) and a boot reconciler are noted follow-ups.

## Shipped (S7 — platform maturity: the signed extension registry)

The first S7 vertical slice: a node installs an extension from a **signed registry**, runs it
**offline once cached**, and **rolls back** to a prior version — built by composing the S4 install
flow with a new artifact-verification crate, not by re-cutting either.

- **`lb-registry`** — artifact identity + the one new crypto surface. A `digest(manifest, wasm)`
  binds the manifest AND the bytes (SHA-256, length-prefixed framing); `verify_artifact` recomputes
  the digest and Ed25519-verifies the signature against an allow-listed publisher key, returning a
  **`VerifiedArtifact` newtype it alone can mint**. Reuses the `lb_auth` `ed25519-dalek` idiom — no
  second crypto stack. See `registry/registry.md`.
- **The host `registry` service** (beside `agent`/`channel`/`assets`/`workflow`) — `pull` (fetch ·
  **verify** · cache, serving the cache **without a source call** when present — the offline path),
  `install_from_registry` (pull THEN the **existing** S4 `install_extension`; **rollback = the same
  verb with a prior version**), `list`/`resolve` over MCP. The fetch is behind a host-owned `Source`
  trait (the outbox `Target` / agent `ModelAccess` analogue); `cache_artifact` accepts only a
  `VerifiedArtifact`, so **verify-before-cache is a compile-time guarantee**. Cache + catalog are
  SurrealDB records in the workspace namespace (no second datastore; isolation is structural). See
  `registry/registry.md`.
- **Two independent gates** — the **capability** gate (`mcp:registry.<verb>:call`, workspace-first)
  and the **signature** gate (`verify_artifact`). Granted ≠ trusted: a fully-granted caller is still
  refused a tampered/unsigned/untrusted artifact, and a trusted artifact still needs the grant.
- **`lb-role-registry-host` — the real HTTP transport** (S7 follow-up, replacing the in-memory
  source stub): an axum **server** (`router`/`serve`) serving signed artifacts at
  `GET /artifacts/{ext_id}/{version}`, and an **`HttpSource`** client filling the host `Source` seam.
  The server is a **dumb origin** (signs/verifies nothing); the wire is untrusted and the client
  re-verifies on arrival, so a tamper *in transit* is caught by the same gate as a tamper at rest.
  `reqwest`/`axum` live in this role crate, never in core `lb-host` (roles depend on host). See
  `registry/registry.md`.
- **UI** — a `RegistryView` + `registry.api` client mirroring the verbs, with a faithful in-memory
  fake exercising the capability gate, the signature gate, and rollback. See `frontend/frontend.md`.

**Exit gate — first half MET.** An extension installs from the signed registry; runs offline once
cached (zero source calls on the cached path); rolls back to a prior version with **no durable
workspace state lost** (a channel message + job step survive an N→N−1 install through the real wasm);
a tampered/unsigned/foreign-key artifact is rejected before caching, even with the grant. **145 Rust +
22 Vitest + 2 shell tests** pass — incl. capability-deny (each registry verb), workspace-isolation
across **store + MCP** (a ws-B caller sees no ws-A cache/catalog and cannot ride its cache offline),
offline (install succeeds with the source unreachable once cached), rollback/hot-reload (durable state
preserved), and signing/verification (the new crypto surface). The native Tier-2 sidecar — the exit
gate's second half — **shipped** next (above); the S7 exit gate is now fully met. As an S7 follow-up,
the registry's last mocked external became real: `lb-role-registry-host` (server + `HttpSource`) proves
the whole pull·verify·cache path over a real HTTP socket — round-trip, offline-from-cache,
tamper-in-transit rejected, ws isolation, and deny (+5 Rust tests; ~168+26+2 green).

## Shipped (S6 — coding workflow)

A vertical slice composing the S5 agent + jobs with a new must-deliver outbox — the worked example
end to end (vision `0002`), built **entirely from core primitives** (the core never learns "coding
agent").

- **The transactional must-deliver outbox** — a new `lb-outbox` crate: an `Effect` record + raw
  verbs. The pattern: `enqueue` (over the new `lb_store::write_tx`) writes the **domain change AND
  the effect in one transaction** — both commit or neither. `relay_outbox` delivers `pending`
  at-least-once through a host-owned `Target` trait, retrying `failed` rows; the receiver dedups on
  `idempotency_key` (never lost, never double-sent). See `inbox-outbox/inbox-outbox.md`.
- **The inbox resolution facet** — `lb_inbox::Resolution` (approve/reject/defer + actor + ts), a
  sibling record keyed by the item id (the `Item` shape stayed stable). The subject of the approval
  gate. See `inbox-outbox/inbox-outbox.md`.
- **The host `workflow` service** (beside `agent`/`channel`/`assets`) — the orchestrator, holding no
  durable state: `ingest_issue` → `triage` (drives the S5 agent over MCP to draft + `share_doc` a
  scope doc) → `request_approval` → `resolve_approval` → `start_coding_job` (**THE GATE**: starts the
  durable job only on `Approved`, creating nothing otherwise) → `emit_effect` (job step + effect in
  one transaction) → `relay_outbox`. `workflow.*` is reached over the one MCP contract. See
  `coding-workflow/coding-workflow.md`.
- **UI** — a `WorkflowView` + `workflow.api` client mirroring the verbs, with a faithful in-memory
  fake exercising the capability + approval gates and the outbox. See `frontend/frontend.md`.

**Exit gate met.** The full flow runs; the approval **genuinely gates** the job (no job record
before approval; refused with `AwaitingApproval`; a rejected approval starts nothing); every external
effect goes through the outbox with retry. **124 Rust + 18 Vitest + 2 shell tests** pass — incl.
capability-deny (each workflow verb), workspace-isolation across **store + MCP** (a ws-B caller/relay
sees no ws-A state), and offline/sync (an effect survives an outage and is delivered at-least-once,
idempotently — never lost, never double-sent).

## Shipped (S5 — AI core)

A vertical slice through every layer: a central agent, the swappable gateway, and durable jobs.

- **Central AI agent** — a host `agent` service (beside `channel`/`assets`): a workspace-scoped
  actor that **owns the tool-call loop** (ask the gateway for a turn → run each proposed tool call,
  capability-checked → feed results back → repeat, bounded by `MAX_STEPS`). Reached over MCP
  (`mcp:agent.invoke:call`) and over the **routed namespace** — an edge invokes the hub agent via a
  Zenoh queryable (`ws/*/agent/invoke`, the S3 routing seam), `caps::check` on the calling node.
  See `agent/agent.md`.
- **Grant delegation (the intersection)** — the agent acts under `agent ∩ caller`:
  `Principal::derive` mints a strictly **narrower** actor, and `caps::check` gained **gate 2b** (a
  delegated request must match the caller's caps too). An agent can never widen its own access.
  Substrate reads (granted skill + shared doc) run **on the caller's behalf** (caller's identity for
  the S4 membership/grant gate, intersected caps for the capability gate). See `auth-caps/auth-caps.md`.
- **Swappable AI-gateway sidecar** — `lb-role-ai-gateway` behind a stable contract
  (`AiRequest`/`AiResponse`/`ToolCall`): **model access only, no loop** (that's the agent's). A
  `Provider` trait + a deterministic mock (the only external stubbed) + a **replay-safe idempotency
  cache** (same key → cached response, never re-spent). One contract, many implementations.
- **Durable resumable jobs** — a new `lb-jobs` crate: the session is a `job:{id}` record with an
  **append-addressed transcript** + a cursor (no separate datastore, §3.2). Resume continues from
  the cursor; re-applying a persisted step is a no-op. The atomic-claim multi-worker queue is
  deferred (jobs scope). See `../scope/jobs/jobs-scope.md`.
- **UI** — an `AgentView` + `agent.api` client mirroring the verb, with a faithful in-memory fake
  exercising the invoke + grant gates. See `frontend/frontend.md`.

**Exit gate met.** An edge user invokes the central agent; the agent calls the gateway for a model
and a granted MCP tool; a workflow job survives the edge disconnecting and resumes idempotently.
**105 Rust + 14 Vitest + 2 shell tests** pass — incl. capability-deny (invoke gate + the in-loop
intersection), workspace-isolation across **store + MCP**, and offline/sync (interrupted session
resumes from its cursor; a duplicated invocation does not double-apply or re-spend).

## Shipped (S4 — shared workspace assets)

A vertical slice through every layer, building the asset substrate the AI workflows (S5–S6) stand on:

- **Docs as workspace assets** — a new `lb-assets` crate (the store side: `Doc`/`Skill`/relation/
  install models + raw verbs) + a host `assets` service (the auth side). A doc read passes **three
  gates in order**: workspace (namespace) → capability (`store:doc/*:read`, no grammar change) →
  **membership** (owner / shared-team-member / linked-channel-`sub`-grantee — the layer tenancy
  deferred). Sharing is a live graph relation, not a content copy; revoke = delete one edge. See
  `files/files.md`.
- **Content as a record, not a bucket** — `DEFINE BUCKET` isn't in our embedded `kv-mem` build, so
  asset content is stored as a record value behind a bucket-compatible verb (S7 swaps the backend
  by config). See `../debugging/store/define-bucket-unavailable-in-kv-mem-build.md`.
- **Skills as versioned, grant-gated assets** — `skill:{id}@{version}` immutable per version
  (rollback = a prior version); `load_skill` returns the body **only when the workspace granted the
  skill** (`grant:skill/{id}` relation) — the §6.12 "load only when granted" rule. See `skills/skills.md`.
- **Agent memory — durable, access-walled** (2026-07-03) — the workspace agent's learned knowledge
  in the MEMORY.md shape: a SCHEMAFULL `agent_memory` table keyed `{ws, scope, slug}` (scopes
  `workspace` + `member:{user}`, the member scope derived from the principal — never an argument),
  four verbs `agent.memory.list|get|set|delete` (one cap each + a distinct workspace-scope write
  gate), a derived index injected at session start into both runtimes (framed as recalled background),
  bounds (desc ≤ 120, body ≤ 8 KB) + a best-effort secret lint. Memory changes quality, never
  authority. See `agent-memory/agent-memory.md`.
- **Skills — the core (developer) tier** (2026-07-03) — a second tier alongside user skills:
  `docs/skills/*/SKILL.md` embedded at build time, seeded at boot as immutable
  `skill:core.<name>@<node-version>` in a reserved namespace (`_lb_skills`), read-only to users
  (`put`/`deprecate` reject `core.*`), resolved through the SAME grant gate (no bypass). Adds
  `assets.deprecate_skill` (soft delete), `list_skills` tier/description rows, a default grant set at
  workspace creation, and granted-catalog injection into both agent runtimes. See `skills/skills.md`.
- **Extension install records** — `install_extension` persists `granted = requested ∩ admin_approved`
  as an `install:{ext_id}` record (closing the S1 deferral); `installed` reads it back,
  workspace-isolated. See `extensions/extensions.md`.
- **`assets.*` over MCP** — the verbs are reachable through the one MCP contract via a host-native
  bridge (`call_asset_tool`): the MCP gate (`mcp:assets.*:call`, workspace-first) then the verb's
  own store + membership gate. See `mcp/mcp.md`.
- **UI** — a `DocView` + `assets.api` client mirroring the verbs, with a faithful in-memory fake
  exercising the allow/deny paths. See `frontend/frontend.md`.

**Exit gate met.** A doc private to a user can be shared to a team and linked into a channel; a
non-member is denied; a skill loads only when granted. **83 Rust + 11 Vitest + 2 shell tests** pass
— incl. capability-deny (non-member / no-grant) and workspace-isolation across **store + MCP**.

## Shipped (S3 — multi-node / sync / SSE)

A vertical slice through every layer, standing up a second node and the browser path:

- **Node roles as config** — `lb_host::Role` (`Edge | Hub | Solo`) + `Node::boot_as(role)`. Same
  binary, same crates; the only role-derived policy is data authority (§6.8). A second node is just
  a second `boot_as` (two in-process Zenoh peers auto-discover). No `if cloud` anywhere. See
  `sync/sync.md`.
- **Cross-node MCP routing** — the dispatch seam is real: a tool call on the edge routes over a
  Zenoh **queryable** (`mcp/{ext}/call`) to the hosting hub, callers + `authorize` unchanged,
  `caps::check` on the calling node workspace-first. See `mcp/mcp.md`. New bus primitive:
  `declare_queryable`/`query` (`bus/bus.md`).
- **Channel sync (edge↔hub, §6.8 append-style subset)** — `sync_channel` idempotently applies bus
  items into the local store; `replay_history` catches a reconnecting node up. Offline write →
  reconnect → idempotent merge, conflict-free (inbox upserts on `(channel,id)`). See `sync/sync.md`.
- **SSE/HTTP gateway** — `lb-role-gateway` (axum): POST/GET `/channels/{cid}/messages` + SSE
  `/channels/{cid}/stream` (live `message` + `presence`). Every route forwards to a
  capability-checked `lb_host` verb. The browser reaches a real node; only `ui/lib/ipc/invoke.ts`
  swapped transport. See `frontend/frontend.md`.

**Exit gate met.** A second node joins; a cross-node tool call routes and is capability-checked;
channel data syncs edge↔hub with idempotent offline apply; the browser reaches a node over SSE/HTTP
(replacing the S2 in-memory fake) and sees live messages appear. **61 Rust + 8 Vitest + 2 shell
tests** pass — incl. capability-deny, workspace-isolation, and the first offline/sync categories,
all now **across two nodes** and the gateway.

## Shipped (S2 — first app: messaging + UI + hot-reload)

A vertical slice through every layer, on top of the S1 spine:

- **Bus** — Zenoh pub/sub (`publish`/`subscribe`) + **presence** via liveliness tokens, all
  workspace-scoped (`ws/{id}/chan/{cid}/**`). See `bus/bus.md`.
- **Inbox** — a normalized `Item` model persisted to SurrealDB; idempotent on `(channel,id)`,
  ordered by `ts`. See `inbox-outbox/inbox-outbox.md`.
- **Channels (state vs motion)** — the host `channel` service: post → `caps::check` → **persist
  (store) then publish (bus)**; history reads the durable record. The capability chokepoint for
  messaging (`bus:chan/{cid}:{pub|sub}`).
- **Store** — added a workspace-scoped `list` filter verb. See `store/store.md`.
- **UI** — a React + Tailwind **channel view** in a Tauri v2 shell, talking to the in-process
  node over IPC; the api client mirrors the Rust verbs. See `frontend/frontend.md`.
- **Hot-reload** — `reload_extension` swaps a live component (hello v1→v2) with **no durable
  state lost** (the stateless-extension guarantee).

**Exit gate met.** Post a message in the UI and it appears (Vitest); history survives independent
of the bus / a restart (the store keeps it); an extension version swaps live with state intact.
54 Rust tests + 6 Vitest + 2 shell tests pass — incl. the mandatory capability-deny,
workspace-isolation (bus+store+inbox), and hot-reload categories.

## Shipped (S0 + S1 — the spine)

A single Rust binary (`node`) built from a Cargo workspace, running a **solo node** that
proves the capability model end to end:

- **Workspace** (`rust/`) — the §9 crate split: real `host store bus runtime mcp auth caps
  ext-loader`, plus stubs for `tags inbox jobs secrets sync` and placeholder `role/*` crates.
  See `crate-layout/crate-layout.md`.
- **Stable extension ABI** — a WASI 0.2 Component-Model world in `sdk/wit/world.wit`
  (`lazybones:ext/extension@0.1.0`), versioned; host + guest generated from the one file.
- **Capabilities** — `<surface>:<resource>:<action>` grammar; an Ed25519 JWT with a single
  `ws` claim; a two-gate check (workspace-first, then capability) as the single chokepoint.
  See `auth-caps/auth-caps.md`.
- **MCP** — `call → authorize → resolve → dispatch`; authorize before resolve so denials don't
  leak tool existence. See `mcp/mcp.md`.
- **Store** — embedded SurrealDB, workspace = namespace (isolation is structural).
- **Bus** — embedded Zenoh peer + workspace key prefixing (pub/sub at S2).
- **Runtime** — wasmtime component host; loads `extensions/hello` and answers `hello.echo`.
- **CI** — FILE-LAYOUT size check + build (incl. the wasm guest) + test + fmt.

**Exit gates met.** S0: `cargo build` green; CI runs; manifest + capability grammar written as
scope docs. S1: tool call refused without the grant / allowed with it; cross-workspace data is
invisible. 35 tests pass (mandatory capability-deny + workspace-isolation included).

## The four forever decisions (resolved in S0)

| Decision | Choice | Where |
|---|---|---|
| SDK/WIT boundary | WASI 0.2 Component-Model world, semver-versioned | `crate-layout/` |
| Capability grammar + token | `surface:resource:action` + Ed25519 JWT, two-gate check | `auth-caps/` |
| Job queue | thin **native** SurrealDB queue (not apalis); resumable-session subset built at S5 | `../scope/jobs/` |
| Extension manifest | **TOML** `extension.toml`; host grants `requested ∩ approved` | `../scope/extensions/` |

## Not yet built

**Packaging the S6 workflow/github-bridge as installed wasm artifacts** (now that the registry exists
to install them through). For the **native tier** (now shipped): a **boot reconciler** (re-spawn
`lifecycle=started` sidecars from records on boot), **OS-level hardening** (cgroups/seccomp/userns —
the slice ships process-group isolation + scoped identity + bounded restart), a **background
health-poll reactor** (the slice restarts on-demand at the call boundary), the **child→host MCP
callback transport**, and **native artifact platform-target enforcement** remain. For the registry: a
**real HTTP `Source`/`registry-host` server** (the in-memory test
source is the only stub), a **durable publisher-key allow-list** + the admin trust-management flow,
**key rotation/revocation** (needs the hub identity directory), **cache eviction/GC**, the **public
catalog read-only union** (S7 ships per-workspace catalog entries), and **`registry.update`** semantics.
A **real model provider** behind the gateway contract (the S5 mock is the only stub) and **streaming**
agent progress as Zenoh motion remain. The outbox's **real `Target`
adapters** (GitHub HTTP, email, sync), **backoff + dead-letter**, the **multi-relay atomic claim**,
FIFO-per-target ordering, and the LIVE-query relay reactor are deferred (S6 shipped the transactional
enqueue + at-least-once retry + receiver dedup with an in-test target). **Token-on-the-bus** for
routed agent invocations (S5 is in-process co-trust), bus message classification, serve-side
authorization for hub-authoritative routed calls, packaging the workflow/bridge as installed wasm
artifacts, and explicit edge→hub router endpoints (S7) remain. Tracked in `../STATUS.md` and
`../STAGES.md`.
