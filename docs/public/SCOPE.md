# Project scope (as built)

The trimmed source of truth for what exists now. The full architecture spec is the root
`README.md`; the staged plan is `../STAGES.md`; live status is `../STATUS.md`.

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
