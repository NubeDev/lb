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
