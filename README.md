# lazybones


# Core Stack Scope

**A reusable, extensible backend + frontend platform.**
Status: architecture scope (no implementation detail). Intended as the basis for writing a coding scope.
Where the build actually is right now: see [`docs/STATUS.md`](docs/STATUS.md).

---

## 1. Purpose

A single reusable platform that other projects build on by writing **extensions**. The same platform can become a chat/Slack clone, a coding IDE, a Node-RED-style flow tool, a document/PDF store, an offline file server, an email client, a coding-agent workplace, and so on — because all of that is delivered as extensions on a common core.

The core provides: identity, a multi-model datastore, a real-time message bus, an extension runtime, an extension distribution server, a permission/capability system, a shared UI shell, a cloud AI gateway, and durable workflow primitives. Everything else is an extension.

A cloud hub may host a **shared AI gateway** and one or more **central AI agents** for a workspace. Edge users can route to them through the same MCP + bus layer when online, while still keeping local/offline operation for local tools and cached work. The shared workspace surface is: skills, docs, extensions, channels, inbox/outbox items, AI gateway access, and remote workflow sessions.

---

## 2. Goals & non-goals

**Goals**
- One reusable core; product features arrive as extensions.
- **One stack**: the same Rust crates run everywhere. "Edge" and "cloud" are *roles*, not separate codebases.
- Local-first and fully offline-capable on a single node; syncs to a central server so teams can collaborate.
- Runs on Windows, Raspberry Pi, cloud servers, desktop, mobile, and browser.
- Extensible at every layer: data, messaging, jobs, UI, and AI tools.
- Share workspace assets across edge and cloud: skills, docs, extensions, a cloud AI gateway, central AI agents, and durable remote workflow sessions.

**Non-goals (for v1)**
- General multi-master database replication (use the authority model in §6.8 instead).
- An `org` tier above workspaces (defer until an enterprise-grid use case appears — see §7).
- A separate microservice mesh; the node is a modular monolith assembled from crates.

---

## 3. Core principles

1. **Symmetric nodes.** One node binary built from shared crates. Cloud and edge differ only by configuration (which roles are enabled) and by data authority — never by separate code. No core crate may contain `if cloud { … }` branches; role-awareness lives in config and in two thin layers only (the entry/UI layer and the role/deployment layer).
2. **One datastore.** SurrealDB is the only persistence layer, on every node. No SQLite, no Postgres, no separate blob service. Any library that insists on its own datastore is a smell — wrap SurrealDB or choose a SurrealDB-native approach.
3. **State vs motion.** SurrealDB holds *state*; Zenoh moves *messages*. Don't use the database as a message bus or the bus as a database.
4. **Stateless extensions.** An extension instance holds no durable state. All persistent state lives in SurrealDB or on the bus, so any instance can be killed and recreated (this is what makes hot-reload safe).
5. **Capability-first security.** Nothing — not the filesystem, network, secrets, the database, or another extension — is reachable except through a host-mediated capability check. The capability system is the actual product.
6. **Workspace is the hard wall.** Every key in the system is scoped by workspace (= tenant). Isolation is checked first; capabilities operate within it.
7. **MCP is the universal contract.** Extension capabilities are expressed as MCP tools, so AI agents, the UI, and other extensions all call them the same way.

---

## 4. Architecture overview

A node is a vertical stack:

- **UI / entry layer** — how clients reach the node (local UI on edge; gateway + bootstrap on cloud).
- **Host + MCP server** — the kernel and the tool contract.
- **AI gateway** — cloud-hosted model/provider routing, streaming, quotas, audit, and shared AI access.
- **Extension runtime** — WASM components and native sidecars.
- **Platform crates** — auth, capabilities, tags, inbox/outbox, jobs, secrets, sync, extension loader.
- **Bus** — Zenoh.
- **Store** — embedded SurrealDB.

The middle four layers are identical crates on every node. Only the top (UI/entry) and the per-layer configuration differ between edge and cloud.

AI/workflow products sit above this stack as extensions. For example, a coding-agent workplace is not a special core mode: it is an extension that uses shared docs, GitHub inbox items, approval inbox items, outbox delivery, jobs, channels, MCP tools, and a central AI agent.

The shared AI gateway is not the same thing as an agent. The gateway brokers access to model providers, workspace keys, model policy, request streaming, usage limits, audit logs, and optional context services. Agents and workflow extensions call through it; they do not each own provider credentials or model routing.

---

## 5. The node model (edge & cloud as roles)

There is one node. A node plays one or more roles, chosen by config:

- **Edge role** — runs on a user's device (desktop, Pi, mobile). Single user, possibly many workspaces. Embeds SurrealDB and a Zenoh peer that connects up to a hub. Works fully offline.
- **Cloud hub role** — the central server. Runs a Zenoh router, hosts many workspaces, is the identity authority for shared data, hosts the shared AI gateway, and hosts the extension registry.
- **Solo** — an edge node with no hub; its own authority. The "single tenant" case is just N=1.

The same binary covers all three; configuration selects the role(s). See §8 for the precise shared-vs-different split.

---

## 6. Core stack components

### 6.1 Data store — SurrealDB

The single source of truth on every node, embedded in-process. Used multi-model: relational/SQL, document, graph (membership and relations), vector search (HNSW), time-series (complex record-ID ranges + table views for rollups), full-text search, and file storage (buckets).

- **Tenancy mapping:** workspace = namespace; environments/sub-divisions = databases beneath it.
- **Files:** via `DEFINE BUCKET`; backed by local disk on edge, and S3/GCS/Azure on cloud for scale. (Note: file support is recent and currently experimental — validate for heavy blob workloads.)
- **Change data capture:** change feeds power sync and external integration; LIVE queries provide real-time push (treat LIVE notifications as ephemeral — persist them for durable delivery).

### 6.2 Event bus — Zenoh

Rust-native and embeddable: the host process *is* a bus peer (no separate broker per node). Provides pub/sub, queryables (request/reply and RPC), and liveliness tokens (presence and extension health).

- **Channels** = key expressions with wildcards, namespaced per workspace (`ws/{id}/...`).
- **Topology:** edge nodes run in peer mode and connect up to cloud nodes in router mode; peers can also talk directly on a LAN.
- **Durability:** Zenoh is not a durable log. Anything that *must* be delivered goes through the outbox (§6.10), not raw pub/sub. Classify every message: fire-and-forget, must-deliver, or must-replay.

### 6.3 Extension runtime

Two tiers, both running on every node:

- **Tier 1 (default): WASM components** — wasmtime, WASI 0.2 / Component Model, WIT-defined interfaces. Sandboxed, capability-gated, portable, the safe default. Resource caps via wasmtime fuel/epoch limits, applied per workspace.
- **Tier 2 (escape hatch): native sidecar processes** — for extensions needing full OS access or native libraries (language servers, media tooling, GPU). Supervised; communicate over Zenoh or a local socket.

**Placement** is declared per extension: `local-only`, `cloud-only`, or `either`. An `either` WASM component can migrate between nodes. **Hot-reload/unload** is first-class: for WASM, instantiate the new component, atomically swap routing, drain and drop the old; for native, blue-green the process. Liveliness tokens drive supervision and restart.

### 6.4 Extension registry & distribution

A **central registry** on the cloud hub: a catalog plus signed, versioned artifacts (artifacts stored via SurrealDB buckets, metadata in SurrealDB).

- **Visibility classes:** *public* (global catalog, any workspace can install) and *private* (in a single workspace's namespace, only that workspace can see/install). Visibility is independent of trust.
- **Distribution:** a node pulls an artifact on demand, verifies its signature, caches it locally, and instantiates it through the runtime. This pull-verify-cache path *is* the hot-reload path; rollback is pulling the previous version. Once cached, an edge runs offline.
- **Trust:** every extension, public or private, runs as a per-workspace instance with only the capabilities its workspace admin granted at install. "Public" means discoverable, never "more privileged."
- The registry itself is a platform extension exposing `install` / `list` / `update` as MCP tools.

### 6.5 MCP / tool layer

Every node runs an MCP server (rmcp) that exposes the tools of the extensions hosted *on that node*. Because the bus spans nodes, a tool call routes to whichever node hosts the target extension (via Zenoh queryables), so clients see one unified tool namespace. The same MCP contract serves three callers: AI agents, the UI, and peer extensions.

The cloud hub can also host the shared AI gateway and workspace-level AI agents as MCP-capable actors. Edge users call those agents through the same routed tool namespace, so a laptop UI can use a cloud-hosted coding/research/review agent without embedding the model locally. Capability checks still happen workspace-first: the gateway and central agent only see the docs, channels, secrets, tools, and extensions granted to that workspace and request.

### 6.6 Identity, auth & capabilities

- **Identity:** users are global identities (one person, many workspaces), stored in a system directory on the hub. The cloud hub is the authority for shared identity/team/workspace data; edge nodes verify tokens offline using the public key and operate on cached identity.
- **Auth:** issue API keys; OIDC for human login; tokens are JWTs carrying a workspace claim and scopes.
- **Roles:** super-admin → workspace-admin → members (RBAC).
- **Capabilities:** one identity resolves to scopes that project onto *all three* enforcement surfaces — SurrealDB record/namespace access, Zenoh key-expression permissions, and MCP tool grants. Keep these as projections of one scope model, not three independent systems. **Enforcement order: workspace isolation first (hard boundary), then capability checks within it.**

### 6.7 Secrets

Capability-mediated envelope encryption. A master key (env/KMS) wraps **per-workspace** data keys, which wrap **per-extension** secrets. Extensions never touch the store; they request a secret and the host returns it only if the grant allows. OS keychain (`keyring`) on desktop; encrypted at rest in SurrealDB on the server.

### 6.8 Sync (edge ↔ cloud)

No general multi-master replication. Partition data by authority:

- **Node-local data** — owned by one node, never conflicts, never synced.
- **Shared workspace/identity data** — cloud-authoritative; edges hold a read-cache; edge-originated writes queue through the outbox to the hub and merge cleanly because they are append-style.
- **Real-time collaborative content** (e.g. live co-editing) — a per-extension concern using CRDTs (Automerge/Yjs) inside that extension, not a platform feature.

Mechanism: SurrealDB change feeds → outbox → Zenoh → idempotent apply, last-writer-wins on the rare contested shared record. The sync logic is one reusable crate; direction and authority are config.

### 6.9 Jobs

A **SurrealDB-native** durable job queue (no separate datastore). Jobs are records; workers claim atomically via a conditional `UPDATE` in a transaction; LIVE queries give instant pickup; an indexed `run_at` field plus record-range scans handle scheduling and delayed jobs; retries/backoff/cron are fields and queries. May be implemented either as a custom `apalis-surrealdb` backend (to keep apalis's worker/middleware/cron ergonomics) or as a thin native queue. Either way: jobs persist in SurrealDB on every node.

Remote workflow sessions are a first-class use of jobs. A workflow extension can create a long-running job such as "triage this GitHub issue", "draft a scope doc", "run the coding agent", "review this document", or "prepare a release". The job owns durable session state, streams progress to channels, writes attention items to inbox, and queues external effects through outbox.

### 6.10 Inbox / outbox

Generic, source-agnostic, built in-house.

- **Inbox:** a normalized item model (source, type, payload, tags, read-state, timestamps) that any extension writes to and any UI/extension reads or subscribes to. Email, CI, chat, etc. all deposit into the same shape.
- **Outbox:** the transactional-outbox pattern — write the domain change and an outbox row in one SurrealDB transaction, then a relay publishes to Zenoh. This is the durability backstop for anything that must be delivered.

Examples:

- A GitHub issue extension writes an inbox item tagged `source:github`, `repo:{repo}`, and `needs:triage`.
- A coding workflow extension turns that inbox item into a scope doc and posts a summary to a channel.
- A human approval appears as an inbox item for the responsible user or team.
- Once approved, the workflow writes an outbox row to start a remote coding-agent session, comment on GitHub, send a message, or sync the result to the hub.

### 6.11 Tags

A cross-cutting tagging service: **key:value tags with search**, usable on any entity (records, files, messages, inbox items, extensions, jobs). Modeled as graph edges (`entity ->tagged-> tag:{key,value}`), giving both traversal and indexed/full-text search. Supports exact `key:value`, key-only, value search, and faceted combinations. Reused as the labels in the time-series model.

### 6.12 Files

Through SurrealDB buckets (§6.1). Application code always talks to the bucket/file API; the physical backing is config (local disk on edge, S3-compatible on cloud). Includes file sharing (bucket permissions scope who can put/get/list).

Docs and skills are shared workspace assets built on the same store/capability model. A document can be private to a user, shared with a team, linked into a channel, attached to an inbox item, or made available to an AI workflow session. Skills are versioned assets that a central or local AI agent can load only when granted by the workspace.

### 6.13 Frontend

One React + Tailwind + shadcn/ui codebase, delivered two ways:

- **Edge:** bundled in a **Tauri v2** shell; the UI talks to the local host directly (no SSE). Includes a **workspace switcher** (Slack-style).
- **Cloud:** served to remote browsers via an **SSE gateway** (server→browser push) plus **HTTP POST** for commands; also serves a minimal embedded **bootstrap UI** (first-run only, mints the first super-admin token, then locks).
- **Admin section:** part of the same app, role-gated; surfaces admin-scoped MCP tools (cloud administration is itself a privileged platform extension).
- **Extension UIs:** module federation for trusted first-party extensions; Web Components or iframes for untrusted ones. Design tokens exposed so extension UIs look native.

### 6.14 Shared AI gateway

The cloud hub exposes a shared AI gateway for workspaces that want central model access. It is the controlled path between agents/workflow extensions and external or local model providers.

Responsibilities:

- **Provider routing:** OpenAI-compatible APIs, local model servers, enterprise providers, or future plugin providers behind one workspace policy.
- **Credential isolation:** provider keys are secrets owned by the workspace or hub, never copied into extensions or edge clients.
- **Streaming:** token/event streams route back to edge UIs, channels, jobs, and workflow sessions.
- **Policy and quotas:** allowed models, max cost, rate limits, data-retention mode, local-only requirements, and team/user budgets.
- **Audit:** every model call records actor, workspace, tool/workflow source, input references, output destination, token/cost metadata, and approval checkpoint if required.
- **Context mediation:** optional retrieval over docs, skills, messages, and files, always filtered through workspace/team/channel capabilities.

The gateway can be implemented as a privileged platform extension on the cloud hub, but the contract should be treated as a core shared service because many extensions will depend on it.

### 6.15 Optional: data/AI plane (SpiceAI)

An optional, pluggable sidebar: SpiceAI (Rust, federated SQL + vector/text search + LLM gateway). Strictly opt-in; too heavy for a minimal Pi profile; can itself be packaged as an extension. Its LLM gateway can double as the AI-tools provider.

SpiceAI may be one implementation behind the shared AI gateway, but the gateway concept should not depend on SpiceAI being enabled.

### 6.16 Shared AI agents & workflow extensions

AI agents are workspace-scoped service actors. They can run locally on an edge node, centrally on the cloud hub, or as a mixture of both. A central cloud agent is useful when teams want shared model access through the AI gateway, heavier compute, shared skills, shared document context, and remote workflow sessions that continue after an edge device disconnects.

The core does not hard-code a "coding agent" product. Instead, extensions compose the primitives:

- **Skills:** reusable instructions/tools stored as workspace assets.
- **Docs/files:** scope docs, specs, PDFs, reference material, and generated outputs.
- **Channels/messages:** live collaboration, progress streams, mentions, and decisions.
- **Inbox:** attention, approvals, external events, assignments, and review requests.
- **Outbox:** durable external effects: GitHub comments, emails, webhooks, sync publishes, and workflow starts.
- **Jobs:** long-running workflow sessions with resumable state.
- **AI gateway:** shared model/provider access, streaming, quotas, and audit.
- **MCP tools:** the common action surface for humans, AI agents, UI, and extensions.

Example coding workflow:

1. A GitHub issue extension receives an issue and writes it to the workspace inbox.
2. A workflow extension asks the central AI agent to read the issue, related docs, and channel context.
3. The AI drafts a scope doc and shares it with the relevant team.
4. The workflow creates an approval inbox item for a user or team.
5. On approval, the workflow starts a remote coding session as a durable job.
6. The coding agent calls models through the shared AI gateway, uses granted MCP tools and skills, streams progress to a channel, and writes durable external actions through outbox.
7. Results are saved back as docs, messages, files, GitHub updates, or follow-up inbox items.

---

## 7. Tenancy & entity model

**Workspace = tenant = the isolation boundary.** One workspace maps to: one SurrealDB namespace + one `ws/{id}/**` bus prefix + its own per-workspace secrets. This is the hard wall and also the "org" level for v1.

Inside a workspace, three entities are **siblings linked by a membership graph** — not a strict nesting:

- **Members** — global users who joined this workspace (membership records reference the global identity).
- **Teams** — named groups of members for roles, mentions, and permissions; flat and overlapping.
- **Channels** — communication/collaboration spaces with a member subset; each maps to a bus subject (`ws/{id}/chan/{cid}/**`) with messages persisted to SurrealDB.

Channels are not under users; teams and channels do not contain each other. A user belongs to many workspaces and switches between them in the UI. An `org` tier above workspace is deferred until enterprise-grid nesting is needed; if added later, it becomes the new outer isolation boundary with workspaces as subdivisions.

AI agents fit this model as workspace-scoped actors, not global super-users. A central AI agent may be hosted on the cloud hub and may call models through the shared AI gateway, but its effective access is still the current workspace, team/channel membership, and granted capabilities. It can be mentioned in a channel, assigned an inbox item, or invoked by a workflow job just like any other actor with tools.

---

## 8. What's shared vs role-specific

**Identical crates and behavior on every node:** host, extension runtime (WASM + native), MCP server, auth, capabilities, tags, inbox/outbox, jobs, secrets, sync, extension loader, SDK; SurrealDB as the only store; Zenoh as the bus; the workspace/membership model; the same UI codebase.

**Configured per role (same crate, different config):**

| Dimension | Edge node | Cloud hub |
|---|---|---|
| Zenoh mode | Peer (connects up to hub) | Router (accepts peers, routes) |
| Serves | One user, this device | Many users across workspaces |
| Workspaces held | The user's workspaces + personal | All hosted workspaces |
| UI delivery | Bundled in Tauri, local IPC | Bootstrap UI + serves React over SSE/HTTP |
| Data authority | Local data; cache of shared data | Authority for shared identity/teams/workspace data |
| SurrealDB role | Embedded; local + workspace cache | Many namespaces; buckets backed by S3 |
| Extension placement | Local-placed (filesystem, IDE, GPU) | Cloud-placed + platform ext (admin, billing) |
| AI gateway | Optional local provider only | Shared model/provider gateway, quotas, audit, streaming |
| Registry | Client — pulls, verifies, caches | Host — catalog + signed artifacts; public + private |
| Identity/auth | Verifies tokens offline | Issues tokens, OIDC, RBAC authority |
| Offline | Full offline operation | Always-on hub |

---

## 9. Crate / workspace layout (high level)

One Cargo workspace.

- **Core crates (compiled into every node):** `host`, `bus` (Zenoh wrapper), `store` (SurrealDB wrapper), `runtime` (wasmtime + native sidecar supervisor), `mcp` (rmcp), `auth`, `caps`, `tags`, `inbox`, `jobs`, `secrets`, `sync`, `ext-loader`, `prefs` (per-user/workspace preferences + localization/unit-conversion, canonical data in & localized presentation out).
- **SDK crate (extension authors depend on this):** WIT bindings, capability traits, host-function interface. This is the public API surface — version it deliberately; breaking it breaks every extension.
- **Role-only crates:** `gateway` (SSE/HTTP, cloud), `ai-gateway` (cloud model/provider gateway), `registry-host` (cloud), `bootstrap-ui` (cloud).
- **`node` binary:** wires the crates together and reads config to select roles.
- **Frontend workspace:** the React/Tauri app (separate from the Rust workspace).
- **Extensions:** shipped as separate WASM/native artifacts — never dynamically-linked Rust (Rust has no stable ABI; the WASM component boundary is the stable plugin ABI).

---

## 10. Platform targets

Windows, Raspberry Pi (ARM), cloud Linux, desktop (macOS/Windows/Linux via Tauri), mobile (SurrealDB embeds on mobile via WASM; SQLite only as a last-resort fallback if the embed proves impractical), and browser (thin client over SSE/HTTP; SurrealDB can also run in-browser via WASM).

Define a **minimal profile** (embedded SurrealDB + Zenoh peer + wasmtime + local extensions — fits a Pi) and a **full profile** (adds router, multi-workspace hosting, registry host, SpiceAI). Enforce the split in config, not in code.

---

## 11. Key risks & hard problems

These are the parts most likely to be underestimated; the coding scope should give each real attention.

1. **The capability model** — the actual core product. Must be expressive, safe, and understandable, spanning the database, the bus, secrets, and peer calls.
2. **SDK / WIT versioning** — a forever commitment once extensions depend on it.
3. **Edge↔cloud shared-data sync** — kept tractable only by the authority partition in §6.8; avoid general multi-master.
4. **Resource fairness / noisy neighbor** — per-workspace wasmtime fuel/epoch caps and query limits; design so a single shared SurrealDB instance can later be split into per-workspace instances by config.
5. **Platform-extension blast radius** — cross-workspace extensions are the isolation risk; keep that set near-empty (admin, billing) and heavily audited.
6. **Unified scope model** — one identity projecting onto SurrealDB + Zenoh + MCP permissions, kept in sync.
7. **SurrealDB maturity edges** — file/bucket support is experimental; there is no native apalis backend (build one or build a native queue). Validate both early.
8. **AI gateway privacy/cost boundary** — model calls can leak context or spend money quickly. The gateway needs explicit provider policy, retention settings, quotas, audit logs, and local-only escape hatches.

---

## 12. Suggested build sequence

1. **Spine first, one binary:** `auth` + embedded SurrealDB + embedded Zenoh + one Tier-1 WASM extension exposed as an MCP tool, with a capability check working end to end.
2. **SSE gateway**, then prove **hot-reload** by swapping that extension live.
3. **Ship one collaboration app fully** — the Slack clone is the right first target (channels = bus subjects, teams/members = membership graph, presence = liveliness, sync = router, inbox = generic inbox) and never forces the native tier.
4. **Add shared workspace assets** — docs/files, skills, extension install records, team/channel sharing, and capability-checked reads.
5. **Shared AI gateway** — central provider routing, workspace secrets, streaming, quotas, audit logging, and local-only policy.
6. **Central AI agent + remote workflow jobs** — host an AI agent on the cloud hub, let edge users invoke it over MCP, and prove a workflow session can continue remotely.
7. **Coding workflow extension** — GitHub issue inbox item → AI scope doc → user/team approval inbox item → remote coding job → outbox updates back to GitHub/channel/docs.
8. **Extension registry** (public + private, signing, pull/verify/cache).
9. **Native (Tier 2) tier** — prove it with an IDE-style extension (language servers).
10. **Optional SpiceAI plane** and additional example apps.

---

## 13. Open decisions to resolve in the coding scope

- Job queue: custom `apalis-surrealdb` backend vs a from-scratch SurrealDB-native queue.
- The **extension manifest** format — the contract declaring placement, required capabilities, and public/private visibility. Everything downstream keys off this; specify it first.
- Capability grammar and the token shape (how tenant claim + role + capabilities encode and get checked workspace-first).
- Blob strategy at scale — when to move SurrealDB buckets from local disk to an S3-compatible backend, and which one.
- Single shared SurrealDB instance vs per-workspace instances, and the trigger for switching.
- Super-admin key custody for the bootstrap flow (show-once, rotation, recovery).
- Federation: whether a user's workspaces may live on different hubs (multiple upstream connections) or are assumed co-located on one hub for v1.
- Shared AI gateway: provider abstraction, model routing policy, quota/cost model, streaming protocol, audit schema, prompt/context retention, and whether each workspace brings its own keys or uses hub-managed keys.
- Central AI agent policy: per-workspace agent instance vs shared hub pool, model/provider configuration, audit logging, and whether edge nodes can require local-only execution for sensitive workflows.
- Remote workflow session schema: job state shape, transcript storage, approval checkpoints, retry semantics, cancellation, and how workflow extensions expose progress to channels/inbox.
