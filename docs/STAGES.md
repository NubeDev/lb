# STAGES — how we build this, in order

The build plan: what to build, in what order, and why. This is the strategic companion
to `../README.md` §12 (which lists the same arc) — here with the *node posture*, *when UI
joins*, and *exit gate* spelled out for each stage.

> Each stage ships its docs the normal way (scope → session → tests → debugging); see
> `ABOUT-DOCS.md`. A stage is not done until its **exit gate** passes and its docs exist.

---

## Three decisions that shape everything

1. **Build ONE solo node first — there is no "edge phase" then "cloud phase."** Symmetric
   nodes (README §3.1) means edge and cloud are the *same binary*, differing only by config
   and role. Run it **solo** (N=1, its own authority, fully offline) through the early
   stages. "Edge↔cloud" arrives later as a config flip (S3), not a second codebase. If you
   ever write `if cloud {…}`, you've gone wrong.
2. **A thin backend spine first, then backend + UI together.** Not months of backend then
   UI. Prove the spine headless (S1), then bring the UI in at the first visible feature (S2)
   and move them together after. The UI is what validates the realtime/bus contract.
3. **Vertical slices, not horizontal layers.** Build one capability all the way through
   (store → caps → bus → MCP → UI), then the next. Never "finish the store crate" in
   isolation — you'll integrate late and discover the contracts are wrong.

---

## The stages at a glance

| Stage | Goal | Node posture | UI? | Maps to README §12 |
|---|---|---|---|---|
| **S0** | Decisions + workspace skeleton | — | no | (pre-work; §13 decisions) |
| **S1** | The spine: capability check end-to-end | Solo, 1 workspace | no | 1 |
| **S2** | First app: messaging (Slack slice) + UI + hot-reload | Solo | **yes** | 2, 3 |
| **S3** | Multi-node: hub role, edge peer, sync, SSE gateway | Edge + cloud | yes | 3 |
| **S4** | Shared assets: docs/files, skills, team/channel sharing | Edge + cloud | yes | 4 |
| **S5** | AI core: central agent + workflow jobs + AI-gateway sidecar | Edge + cloud | yes | 5 |
| **S6** | Coding workflow extension (the worked example, end to end) | Edge + cloud | yes | 6 |
| **S7** | Platform maturity: registry, native Tier-2, optional SpiceAI | both | yes | 7, 8, 9 |
| **S8** | Data plane: durable on-disk store + generic ingest + tagging | both | yes | 6.1 |
| **S9** | Real collaboration UI: identity, workspaces, channels, messaging, inbox/outbox | both | **yes** | 6.13 |
| **S10** | Cross-cutting retrofit: observability, audit ledger, undo/redo | both | yes | 6.17–6.19 |

> **S0–S7 are MET** (see `STATUS.md`). **S8** is the active stage (the persistent store + ingest + tags
> slices); **S9** finishes the UI from an S2 demo into a real multi-user app. These two are independent
> tracks — S8 deepens the platform (data), S9 finishes the existing surfaces (collaboration) — so they
> can proceed in either order; numbered by when they were scoped, not a hard dependency. **S10** is the
> **cross-cutting retrofit** of three concerns that should have ridden the chokepoint from S1 but were
> missed (observability, audit, undo); scoped 2026-06-27, not yet built.

---

## S0 — Decisions + skeleton

Resolve the *forever* decisions before code locks them in (README §13): the **extension
manifest** format, the **capability grammar + token shape**, the **job-queue** choice
(`apalis-surrealdb` vs native), and the **SDK/WIT** boundary shape. Stand up the Cargo
workspace under `rust/`, the WIT/SDK skeleton, and CI (the FILE-LAYOUT size check + a test
runner). Keep it minimal — but these are the decisions that are expensive to change later.

**Exit gate:** `cargo build` green on an empty workspace; CI runs; the manifest + capability
grammar are written down as scope docs.

## S1 — The spine (headless)

`host` + embedded SurrealDB + embedded Zenoh + `auth` + `caps` + `mcp` + **one trivial WASM
extension** exposed as an MCP tool. The whole point is to prove the **capability model**
(README §11.1 — the actual core product) end-to-end before anything is built on it.

**Exit gate:** a tool call routed through MCP succeeds *with* the grant and is refused
*without* it; a second workspace cannot see the first's data. These are the mandatory
capability-deny and workspace-isolation tests from `scope/testing/testing-scope.md` — they
exist from day one, not retrofitted.

## S2 — First app: messaging

The Slack-clone slice: channels = bus subjects, messages persisted to SurrealDB, presence =
Zenoh liveliness, a generic inbox. Bring in the **React shell in the Tauri local app**
against the local node. Prove **hot-reload** here by swapping the extension live.

Why messaging first: it exercises every core subsystem (bus, store, presence, inbox) without
ever forcing the native tier, and it's visual — so the UI earns its place validating the
state-vs-motion contract.

**Exit gate:** post a message in the UI, see it appear in real time; restart the node and the
history is intact; swap an extension version with no dropped state.

## S3 — Make it multi-node

Flip on the **router role** to make a hub; run a **second node as an edge peer**. Add `sync`
(the §6.8 authority partition, not multi-master) and the **SSE gateway** so a browser can
reach the hub. This is where "edge/cloud" becomes real — as config of one binary.

**Exit gate:** an edge writes while offline, reconnects, and the write merges idempotently;
a browser reaches the hub over SSE/HTTP; cross-workspace isolation still holds across nodes.

## S4 — Shared workspace assets

docs/files via SurrealDB buckets, skills as versioned assets, extension install records,
team/channel sharing, all behind capability-checked reads. This is the substrate the AI
workflows (S5–S6) stand on.

**Exit gate:** a doc private to a user can be shared to a team and linked into a channel;
a non-member is denied; a skill loads only when granted.

## S5 — AI core

Host a **central AI agent** (workspace-scoped actor) on the hub, callable by edge users over
the routed MCP namespace. Add **remote workflow jobs** (durable, resumable sessions) and the
**AI-gateway sidecar** (swappable model-access service — see
`scope/ai-gateway/ai-gateway-scope.md`; the agent owns the tool-call loop, the gateway does
model access).

**Exit gate:** an edge user invokes the central agent; the agent calls the gateway for a
model and a granted MCP tool; a workflow job survives the edge disconnecting and resumes.

## S6 — Coding workflow extension

The worked example end-to-end (`vision/0002-coding-agent-workplace.md`): GitHub issue → inbox
`needs:triage` → agent triages + drafts a scope doc → approval inbox item → on approval a
durable coding job → progress to a channel → external effects through the **outbox**.

**Exit gate:** the full flow runs; the approval genuinely gates the job; every external
effect (PR, comment, notify, sync) goes through the outbox with retry.

## S7 — Platform maturity

The **extension registry** (pull/verify/cache, signing, public + private), then the
**native Tier-2** tier proven with an IDE-style extension (language servers), then the
**optional SpiceAI** plane and further example apps.

**Exit gate:** an extension installs from the signed registry, runs offline once cached, and
rolls back to a prior version; a native sidecar is supervised and restarts cleanly.

## S8 — Data plane: durable store + generic ingest + tagging

The first stage that **writes to disk**. Three slices, in order: (0) swap SurrealDB from the in-memory
`kv-mem` engine to a **persistent embedded backend** (`Store::open(path)`, engine by config) and run a
**day-one capability spike** that classifies each SurrealDB feature LOAD-BEARING vs DEGRADABLE
(`scope/store/persistent-backend-scope.md`); (1) a **generic buffered ingest surface** — the read-side
analog of the outbox — that absorbs high-volume external data into time-series `series` state without a
write storm (`scope/ingest/ingest-scope.md`); (2) **tags** upgraded from key:value strings to a
**typed annotation + relationship graph** (`scope/tags/tags-scope.md`), the discovery layer over
heterogeneous data.

Why now: must-deliver durability, the energy/IoT-style edge→cloud data-collection use cases, and any
real dataset all need on-disk persistence the earlier stages deferred. Stays generic — a "device" is a
principal, protocol bridges are out-of-core extensions; the platform does not become an IoT system.

**Node posture:** both. The **ingest role** is `either` — the hub usually runs the buffer, but a Pi
sub-hub can run its own. **Exit gate:** data survives a node restart (real persistence, crash-consistent);
a fleet of producers writes one series without collision (`(series, producer, seq)` dedup), buffered and
committed exactly-once; tags classify heterogeneous series and faceted/relationship queries return them;
the capability-deny + two-workspace-isolation + offline-replay tests pass on the persistent engine.

**Status (2026-06-27): exit gate MET — all three slices shipped.** (0) `Store::open(path)` on the pinned
**SurrealKV** engine (config by `LB_STORE_PATH`, no code-branch) + the permanent capability-spike matrix
(all LOAD-BEARING ✓ → GO) + the crash set (kill mid-tx → rollback, flush-burst → last commit survives).
(1) `lb-ingest` — durable staging append → one-tx-per-batch commit (UPSERT on `[series,producer,seq]`),
proven exactly-once across a **kill-mid-commit** subprocess test; two-producer collision both survive;
overflow at both QoS. (2) `lb-tags` — typed `tag:[key,value]` nodes + `(entity,tag,source)` provenance
edges; `add`/`remove`/`of`/`find` (exact/key/faceted) + the required per-workspace tag-node cap;
spike-gated add-ons shipped (BM25 full-text ✓, HNSW vector ✓ with pinned dimension, per-dimension counts
✓ — computed per-query since the materialized view does not populate on SurrealKV). `series.find`
discovery wired on tags. See `sessions/{store,ingest,tags}/`. Anti-IoT discipline held (no
device/sensor/MQTT in core).

## S9 — Real collaboration UI

Take the UI from a single-screen S2 demo bolted to fakes to a **real multi-user collaboration app over a
real node** (`scope/frontend/collaboration-scope.md`). Finishes both ends as needed: a **real login →
token → principal session** (replacing the gateway's demo principal — the keystone), then
**workspaces**, a **channel registry** (list/create), **users/teams/members** (surfacing the S4
membership backend), **messaging between real people** with **rendered presence**, and a **real inbox**
(replacing the workflow fake) + **outbox status**. Mostly transport wiring + missing views over verbs
that already exist — except identity, which is genuinely missing on both ends.

**Node posture:** both (browser→gateway and the in-process Tauri shell). **Exit gate:** two **real**
principals in two workspaces — one cannot see the other's channels/inbox/members (the wall, finally
demonstrable end-to-end); messaging between people works live with presence; the real inbox's approval
gate is a UI action; an expired/forged token is rejected. Capability-deny over real routes +
two-session isolation + offline-replay pass.

## S10 — Cross-cutting retrofit: observability, audit, undo

Three concerns that should have ridden the host chokepoint since S1 but were **missed**: there is no
correlated operational telemetry, no durable record of capability decisions, and no platform
reversibility. All three are **projections of the one event the host already mediates** (§6.5 dispatch
+ §6.6 cap check), so they are scoped together and share the existing `write_tx` seam — not three
bolt-ons (`scope/observability/`, `scope/audit/`, `scope/undo/`).

- **Observability** (README §6.17) — `tracing`-based spans/logs/metrics on every node, with a
  `trace_id` that **propagates across the routed Zenoh hop** and into jobs/outbox; secret-safe by
  construction; export to an external collector (the platform emits, it is not the dashboard).
- **Audit** (§6.18) — an immutable, hash-chained, workspace-walled ledger of **every allow and deny**,
  appended at the chokepoint (so it is complete by construction) and as durable as the action it
  records (same-`write_tx`). Generalizes §6.14's model-call audit.
- **Undo** (§6.19) — a before-image reversible-command journal whose hard line is **reverse state,
  compensate motion**: the host *derives* irreversibility from reaching the outbox, so undo can't
  diverge from the world. Single-actor, sync-safe-by-refusal; collaborative/OT undo stays a CRDT
  extension concern.

**Node posture:** both (symmetric emission/append/journal; sink and aggregation are config). **Build
order:** observability first (foundational + cheapest, makes the rest debuggable), then audit
(security-critical, reuses the same capture + `write_tx`), then undo (the most feature-shaped, depends
on the before-image at `write_tx`). Each can ship as its own vertical slice.

**Exit gate:** a single cross-node tool call produces **one** trace spanning the edge→hub hop with no
secret in any span/log/metric; **both** an allow and a deny append a tamper-evident audit entry that
`audit.verify` confirms (and a direct edit is detected); a reversible state mutation undoes/redoes
correctly while an **irreversible** (outbox-effect) action is **refused** by undo and offers its
declared compensation. Capability-deny + two-workspace-isolation + offline/sync pass for all three.

---

## Reuse: the extension server (port from rubix-cube)

The extension **control plane, supervisor, and runtime** do not need to be invented — they
can be **re-authored** from the sibling project's design:
`/home/user/code/rust/rubix-cube/docs/scope/extensions/extension-server-scope.md` (and its
`crates/rubix-ext` + `crates/rubix-hello-ext`). It already covers, in a shape close to ours:

- **Two backend flavours** — a `process` PID supervisor *and* an in-process `wasm`/Wasmtime
  (Component Model, WASI P2, WIT) peer. Maps to our Tier-2 native + Tier-1 WASM (README §6.3).
- **Idempotent control surface** — install/upsert, `start`/`stop`/`restart`/`disable`,
  uninstall, plus a boot reconciler. Maps to our `ext-loader` + runtime supervision.
- **Federated UI loader** — `ExtensionAutoLoader` + `<ExtensionSlot>` + a React-shim
  importmap over shadcn/ui. Maps to our module-federation extension UIs (README §6.13).
- **MCP tool aggregation** and an event-bus seam.

**This is a re-author, not a copy.** Re-base it onto our primitives: **workspace** tenancy
(not rubix's workspace/project split), our **capability/grant** model and API-key principal,
SurrealDB instead of Postgres+Drizzle, and **Zenoh** for the bus/event seam instead of
Redis/broadcast. Treat the rubix phases (E0–E5) as a proven decomposition to mine, slotting
the lift into our **S1** (runtime + one extension), **S2** (federated UI loader), and **S7**
(registry/distribution + native tier).

When this work starts, write it up as a `scope/extensions/` doc via `SCOPE-WRITTING.md`,
recording exactly what is lifted, what is re-authored, and what is dropped.

---

## Cross-cutting rules for every stage

- **Slice vertically; one capability through all layers** before the next.
- **Capability + isolation tests are mandatory from S1** — they are the gate, not an extra.
- **Each stage produces docs** (scope → session → public) and captures any debugging in
  `debugging/` with a regression test (`ABOUT-DOCS.md`).
- **No `if cloud {…}`** — role differences live in config and the two thin layers only.
- **Resist building the registry early** — load extensions from local disk through S6; do
  signed distribution in S7 once you actually need it.
