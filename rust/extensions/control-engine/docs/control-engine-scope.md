# Control-engine scope — the CE bridge extension (local CE + remote appliances)

Status: scope (the ask). Promotes to `public/control-engine/` once shipped.

We want a first-class way for a workspace to **drive Control Engine (CE) instances** from
Lazybones: a native (Tier-2) extension — `control-engine` — that owns a CE's REST/binary-WS
protocol locally and exposes the engine as a **caps-gated MCP surface** (`ce.*`). A CE running
on the same box is reached over `localhost` REST/WS; a **remote CE lives on an *appliance*** —
an LB edge node (a machine principal, per `auth-caps`) that runs this same extension against its
own localhost CE — and the workspace reaches it the LB-native way: **routed MCP over Zenoh**, no
new CE-specific fabric. The visual editor is the existing **`@nube/ce-wiresheet`** React package,
vendored into the repo and mounted as the extension's federated `[ui]` page, re-pointed onto the
MCP bridge so the browser talks to CE only through the host (workspace + caps enforced at the
edge). Because everything is MCP, **agents and the CLI drive CE identically to the UI** (README
§3 rule 7).

## Goals

- One extension, `control-engine`, that turns any reachable CE into a **workspace-scoped,
  caps-gated MCP tool surface** mirroring the `ce-client-rust` `ControlEngine` trait
  (`tree`/`schema`/`patch`/`set_override`/`call_action`/`add_edge`/…/`watch`).
- **Local mode:** bind a CE over `localhost` REST + binary-WS via the `ce-client-rust` client.
- **Appliance mode:** reach a remote CE that lives on another LB node **over Zenoh** using LB's
  existing cross-node MCP routing — the *same* `ce.*` call, routed by the host to the node that
  owns that CE. Symmetric: identical code on every node; the host router picks local-vs-remote
  from an appliance record.
- Mount **`@nube/ce-wiresheet`** as the extension's federated page, driving CE **only** through
  the MCP bridge (no direct browser→CE connection; no raw host proxy).
- Live change-of-value (COV) surfaced as a `ce.watch` live feed (motion on the bus → gateway SSE).
- Keep `@nube/ce-wiresheet` a **vendored workspace package** we can update in-repo, but **every
  edit to it is approval-gated** (it is an upstream Nube library, not our source).

## Non-goals

- **No new CE transport codec.** We do **not** implement `ce-client-rust`'s reserved `zenoh` feature
  (a CE speaking Zenoh natively via `ce-ext-core`). "Over Zenoh" means LB's routed MCP hop to the
  appliance *node*, which then speaks localhost REST/WS to its CE. (Rejected — see Intent.)
- **No raw host reverse-proxy.** We do not add an extension-contributed raw HTTP/WS proxied
  transport; the surface is MCP only. (Rejected — see Intent.)
- **Not the full `ControlEngine` trait in v1.** v1 ships a core verb subset; the remaining
  trait-mirror verbs (`copy`, `restore`, `bulk`, `remove_edge`, `set_layout`, …) are additive
  follow-ups on the same one path.
- **Not appliance enrollment.** Machine-principal appliance keys + edge-trust already exist
  (`auth-caps/api-keys-scope.md`, `edge-trust-scope.md`); we reuse them, we don't re-cut them.
- Not authoring the CE itself, the C++ engine, or its extensions.

## Intent / approach

**One surface, the MCP contract; appliances are just LB nodes.** The extension is a native
Tier-2 sidecar (like `mqtt`/`fleet-monitor`) that holds the long-lived CE connection and serves
`ce.*` tools. It depends on the **`ce-client-rust`** crate, whose `ControlEngine` trait is already
the narrow, transport-replaceable contract for a CE; each `ce.*` tool is a thin, caps-gated map
onto one trait method.

For **remote appliances**, we reuse the platform's single most important property (README §3
rule 1, symmetric nodes): the appliance runs the **same** `control-engine` extension against its
**own** localhost CE. A `ce.*` call carries an `appliance` id; the host resolves it to the owning
node from a workspace-scoped `ce_appliance` record. If that node is *this* node, the sidecar hits
`localhost` CE via `ce-client-rust`; otherwise the host **routes the MCP call over Zenoh** to that node
(the existing `rmcp`-over-Zenoh routed hop, `key-stack` "MCP/tool layer") where the same tool
runs locally. There is **no `if cloud`** and **no CE-on-Zenoh codec** — motion (COV, routed
calls) rides Zenoh; state (the appliance registry, optional COV history) lives in SurrealDB
(README §3 rule 3).

**Why MCP-only over a raw CE proxy (user decision):** the user chose "over MCP … so the AI can
use it." A raw proxy would keep `@nube/ce-wiresheet` byte-for-byte unforked but adds a new,
un-gated-by-default host transport surface and gives agents nothing (they can't drive a raw
REST/WS socket through the cap system). Making `ce.*` the one surface means the wiresheet, the
CLI, the central agent, and other extensions all drive CE through the same gate. The cost —
**re-pointing `@nube/ce-wiresheet`'s transport onto the bridge** — is real, but we own
`NubeIO/ce-wiresheet`, so it is **not a divergent fork**: we cut a generic `EngineTransport`
seam on an **upstream branch** (`lb-transport`, mergeable to `main` — slice S1), vendor a
byte-identical snapshot of that branch to `packages/ce-wiresheet` (S2, **approval-gated**
pin bumps, never in-place edits), and keep the LB-specific `BridgeTransport` in this
extension's own `ui/` folder (S7), injected via the seam.

**Rejected — `ce-zenoh` direct transport:** implement `ce-client-rust`'s `zenoh` feature so the sidecar
speaks Zenoh straight to a CE with `ce-ext-core`'s Zenoh extension. It couples us to a CE-specific
fabric, bypasses LB routing/caps/workspace-walls on the wire, and makes the appliance a bare CE box
instead of a symmetric LB node — losing enrollment, isolation, and the routed-MCP reuse.

**Rejected — raw gated reverse-proxy:** keeps the package unforked but violates rule 7 (a
non-MCP path to a capability) and is dead to agents/CLI.

## 100% extension — the core stays CE-ignorant (hard invariant)

**Non-negotiable: no host/core crate contains the word `ce`/`control-engine` or any CE concept.**
Everything CE-specific lives in `rust/extensions/control-engine/` (the sidecar + manifest) and
`packages/ce-wiresheet` (the UI). The extension is built **entirely on generic platform primitives**
that other extensions already use — verified against the codebase (`mqtt`, `fleet-monitor`,
`proof-panel`):

| CE-specific concern | Lives in | Generic primitive it rides | Core knows? |
|---|---|---|---|
| CE REST/WS protocol, `ce-client-rust` | the sidecar crate | native Tier-2 supervision (`[native]`) | ❌ |
| `ce.*` tools + their caps | `extension.toml` | manifest `[[tools]]` → `mcp:<id>.<tool>:call` (name is the gate) | ❌ |
| Appliance registry `ce_appliance` | extension's own table | generic `store:ce_appliance:*` verbs | ❌ (no host table code) |
| Reaching a remote appliance | — | routed `<ext>.<tool>` MCP hop over Zenoh (routes by ext-id, never inspects tool) | ❌ |
| Live COV | the sidecar | the **generic** extension-watch primitive (`ce.watch`, `kind="watch"`) + opt-in `series` historian | ❌ |
| CE socket / CE token | manifest request | `net:tcp:127.0.0.1:<port>` + `secret:control-engine/*:get` (generic escape hatches, `mqtt` precedent) | ❌ |
| The wiresheet editor | `packages/ce-wiresheet` | federated `[ui]` remote + `bridge.call`/`bridge.watch` | ❌ |

**The only core change this whole effort implies is generic, not CE-specific:** the
**extension-watch** primitive (`scope/extensions/extension-watch-scope.md`) — "any extension can
contribute a live `watch` tool." That is exactly the kind of generic platform addition that keeps the
core CE-ignorant while making extensions equal citizens; CE is merely its first tenant, and can even
ship on the zero-core-change series-bridge until it lands. **No new WIT world** — `ce.*` reuse the
frozen `tool.call`/`host.call-tool` (like `mqtt`), so the stable ABI is untouched.

This invariant is **enforced by a regression test**, not just documented (see Testing plan).

## How it fits the core

- **Tenancy / isolation.** Every appliance record is `ce_appliance:{ws}:{id}`; every `ce.*` call
  is workspace-first checked, then routed only to an appliance in the caller's workspace. A ws-B
  caller cannot see or target a ws-A appliance (a mandatory isolation test). The routed hop
  carries the caller's workspace claim; the appliance node re-checks it.
- **Capabilities.** Each tool is gated by `mcp:control-engine.<verb>:call` (house convention,
  the manifest NAME is the gate). Read verbs (`tree`/`schema`/`watch`/`appliance.list`) vs write
  verbs (`patch`/`set-override`/`call-action`/`add-node`/`add-edge`/`remove-node`/`appliance.add`/
  `appliance.remove`) are distinct caps. The sidecar **requests** only generic host caps —
  `store:ce_appliance:read`/`write` (its own registry table), `net:tcp:127.0.0.1:<port>` (the native
  socket escape hatch, like `mqtt`), `mcp:ingest.write:call` (only when the opt-in series historian is
  on), and — only when the CE requires auth — `secret:control-engine/<appliance>/token:get` (mediated
  by `lb-secrets`; the wiresheet never sees it). **Not one of these is a verb the host special-cases.**
  **Deny path:** a caller without `mcp:control-engine.patch:call` calling `ce.patch` →
  `DENIED mcp:control-engine.patch:call`, before any CE round trip.
- **Placement.** `either`. The extension runs on every node; on an appliance edge node it binds
  localhost CE, on a cloud node it routes to appliances. No behavioral branch — config/role only.
- **MCP surface (API shape, §6.1).**
  - **Get / list:** `ce.tree` (subtree read — the structural source of truth CE re-numbers on
    restart), `ce.schema` (the add-node palette/type catalogue), `ce.appliance.list`.
  - **CRUD:** `ce.add-node`, `ce.patch`, `ce.set-override`, `ce.clear-override`, `ce.add-edge`,
    `ce.remove-node`, `ce.call-action`, plus registry writes `ce.appliance.add` /
    `ce.appliance.remove`. (Trait-complete `remove-edge`/`restore`/`copy`/`bulk`/`set-layout`
    are additive follow-ups.)
  - **Live feed:** `ce.watch` — COV is *changes, not a snapshot*, so it is a **streaming tool**
    (`kind = "watch"`) built on the generic extension-watch primitive
    (`scope/extensions/extension-watch-scope.md`): the host allocates a workspace subject, the
    appliance sidecar arms its local CE binary-WS COV publisher on first subscriber, and the gateway
    relays SSE (§6.13) — no polled `tree`, no CE-specific host route.
  - **Batch:** `ce.bulk` (one CE round trip) is bounded/always-fast → stays synchronous. A large
    graph **import** (many nodes/edges) MUST be a job (README §6.10), not a blocking loop —
    deferred, named as a follow-up.
- **Data (SurrealDB).** `ce_appliance:{ws}:{id}` = `{ id, name, mode: "local"|"appliance",
  node, base, secret_ref?, ts }` — the registry, state. Optional: bridge selected COV props onto
  the **series** plane (`ingest.write`) for history/replay, reusing the shipped `series.watch`
  SSE (the opt-in historian — see Decisions).
- **Bus (Zenoh).** Two motions, both on **generic** host-owned subjects (no bespoke CE subject):
  routed `ce.*` MCP calls (request/response, must-reach-the-node *online*), and `ce.watch` COV frames
  on the extension-watch-allocated subject (`ws/{ws}/ext/control-engine/watch/{h}`, fire-and-forget,
  replayable from series only if the historian is on).
- **Sync / authority.** The CE is the authority for its own graph; LB holds no shadow copy.
  `ce.*` commands are **online request/response** — if the appliance node is unreachable the call
  **fails loud**, it does **not** queue (an override you can't confirm applied is worse silently
  deferred). Hence **not** the outbox for interactive commands. (A must-deliver *effect* variant —
  e.g. "apply this graph when the appliance returns" — would go through the outbox; out of v1.)
- **Secrets.** Only a CE auth token, if the engine requires one: `secret:control-engine/
  <appliance>/token:get`, host-mediated, never exposed to the UI or logs.

## Example flow

**Editing a remote appliance's wiresheet from the cloud UI:**

1. An admin registers an appliance: `ce.appliance.add { id:"plant-1", mode:"appliance",
   node:"edge-7", base:"http://127.0.0.1:7979" }` (gated `mcp:control-engine.appliance.add:call`).
   A `ce_appliance:{ws}:plant-1` record persists.
2. A user opens the **Control Engine** page (the vendored `@nube/ce-wiresheet`, mounted federated).
   It picks `plant-1` and calls `ce.tree { appliance:"plant-1" }` through the host bridge.
3. The cloud host checks `mcp:control-engine.tree:call` (workspace-first), resolves `plant-1` →
   node `edge-7`, and **routes the MCP call over Zenoh** to `edge-7`.
4. On `edge-7`, the local `control-engine` sidecar runs `ce.tree` against its `localhost` CE via
   `ce-client-rust.get_tree(...)`, returns the `Tree`; the response rides the bus back; the wiresheet
   renders it on React Flow.
5. The user drags/edits → `ce.patch { appliance:"plant-1", node, props }` → same gate → same
   routed hop → `ce-client-rust.patch(...)` on `edge-7`. CE applies it; CE is the authority.
6. The page opens `ce.watch { appliance:"plant-1" }` → the extension-watch primitive allocates the
   workspace subject and routes the `arm` to `edge-7`, whose sidecar publishes decoded CE COV onto it;
   the cloud gateway relays SSE and live values stream to the canvas with no polling.
7. A ws-B user calling `ce.tree { appliance:"plant-1" }` is denied at gate 1 (not their workspace)
   — the appliance is invisible and unreachable across the wall.

## Build plan — eight slices (the detailed asks)

The scope is built in eight vertical slices, each its own doc in this folder and each an
implementing session's ask (HOW-TO-CODE takes one slice + the current stage). Each slice
carries its own deliverables, file map, mandatory tests, and exit gate — the table is the
map, the slice docs are the detail.

| # | Slice | Repo | Depends on | Exit gate (short form) |
|---|---|---|---|---|
| S1 | [`slice-1-wiresheet-transport-seam.md`](slice-1-wiresheet-transport-seam.md) — the `EngineTransport` seam, upstream branch `lb-transport` | **NubeIO/ce-wiresheet** | — | `CeEditor` runs against an injected `MockTransport`, no network; standalone byte-identical |
| S2 | [`slice-2-vendor-wiresheet.md`](slice-2-vendor-wiresheet.md) — vendor the branch snapshot to `packages/ce-wiresheet` | LB | S1 | package tests green in LB; pin + approval rule in README; zero LB edits to vendored files |
| S3 | [`slice-3-sidecar-local-mode.md`](slice-3-sidecar-local-mode.md) — sidecar crate, manifest, pinned `ce-client-rust`, `ce.tree`/`ce.schema`, both test tiers (`ce_fake.rs` + real-engine opt-in) | LB | — (parallel with S1/S2) | local read verbs green + deny tests + one real-engine run |
| S4 | [`slice-4-appliance-registry-routing.md`](slice-4-appliance-registry-routing.md) — `ce_appliance` registry, `appliance.add/list/remove`, local-vs-routed resolution | LB | S3 | two-node routed `ce.tree` green + full isolation/deny matrix + offline fail-loud |
| S5 | [`slice-5-write-verbs.md`](slice-5-write-verbs.md) — the seven v1 write verbs, local + routed | LB | S4 | all verbs green local, `ce.patch` green routed, per-verb deny tests |
| S6 | [`slice-6-ce-watch-cov.md`](slice-6-ce-watch-cov.md) — `ce.watch` on the extension-watch primitive (or the series-bridge fallback), the `cov`/`topology`/`schema` frame contract, opt-in historian | LB | S4 (+ extension-watch, with fallback) | routed watch green end to end: subscribe → arm → frame → SSE |
| S7 | [`slice-7-bridge-transport-ui.md`](slice-7-bridge-transport-ui.md) — `BridgeTransport` + the federated page + appliance picker | LB | S1+S2, S3–S5, S6 | `pnpm test:gateway` suite green; manual cloud-UI-edits-real-engine run |
| S8 | [`slice-8-e2e-hardening-ship.md`](slice-8-e2e-hardening-ship.md) — core-ignorance test, latency/backpressure measurement, `SKILL.md`, public promotion | LB | all | all suites + core-ignorance green; skill verified live; `public/` promoted |

**Critical path:** S3 → S4 → S5/S6 → S7 → S8, with S1 → S2 running in parallel on the
wiresheet side (they join at S7). The external repos in play: `NubeIO/ce-wiresheet` (S1 —
the only repo we branch), `NubeIO/ce-client-rust` (consumed as a pinned git dep, no branch
needed), `ce-studio` (the runnable real engine for the opt-in test tier — not a dependency,
a test harness).

## Testing plan

Per `scope/testing/testing-scope.md`, with the mandatory categories:

- **Capability deny (mandatory).** `ce.patch` / `ce.watch` / `ce.appliance.add` without the
  matching `mcp:control-engine.*:call` grant → denied *before* any CE/bus activity.
- **Workspace isolation (mandatory).** An appliance in ws-A is absent from ws-B's
  `ce.appliance.list`; a ws-B `ce.*` targeting it → not-found/denied. The routed hop carries and
  re-checks the workspace claim on the appliance node.
- **No mocks / no fake backend (CLAUDE §9, testing-scope §0).** The LB side — the gate, the
  appliance store, the host **router**, the bus COV subject, the routed cross-node hop — is
  exercised **for real**: two in-process `Node`s on a real Zenoh bus (the `offline_sync` /
  `cross_node_routing` pattern) prove the local **and** the routed-appliance path against the same
  code. The CE itself is the **one sanctioned true-external** (a C++20 engine we can't build in
  Rust CI): stubbed behind `ce-client-rust`'s `ControlEngine` trait in **one** named file
  (`ce_fake.rs`) for CI, PLUS an **opt-in real-engine tier** — `ce-studio` ships the engine
  prebuilt (`engine.tar.gz`, ce-rest on `:7979`), so an env-gated (`CE_ENGINE_BUNDLE`) integration
  suite runs the same tests against the real engine and the real `ce-client-rust` REST/WS
  transport on a dev box (S3 builds both tiers).
- **Core-ignorance invariant (mandatory, prove-absence).** A `ce_core_ignorance_test` greps the
  host/core crates (`rust/crates/host`, `rust/crates/mcp`, `rust/crates/caps`, `rust/role/gateway`,
  …) and asserts **no** live reference to `control-engine`/`ce_appliance`/CE concepts outside the
  extension folder and docs — the same "prove-absence" pattern as `chains_retired_test`. CI fails if a
  CE string leaks into core.
- **Offline behavior.** Appliance node unreachable → `ce.*` returns a loud error; assert it does
  **not** silently queue.
- **Hot-reload / restart.** The native sidecar restart loses no durable state — the appliance
  registry lives in SurrealDB; a respawn re-reads it (stateless-extension guarantee).
- **UI (real gateway, rule 9).** `pnpm test:gateway` — the vendored wiresheet drives a real
  spawned node: `ce.tree` renders, a `ce.patch` round-trips, `ce.watch` streams a COV frame, and
  the workspace/deny paths hold. No `*.fake.ts`.

## Risks & hard problems

- **Re-pointing `@nube/ce-wiresheet`'s transport is the biggest lift.** Its `src/lib/` (rest/ws/
  wire/store, ~1.3k lines) assumes a direct CE REST + **binary** WS with module-level state
  (`rest.ts` `BASE`, `ws.ts` owning the socket/session/reconnect). Mitigated by owning the upstream
  repo: the seam is an upstream branch (S1), the vendored copy stays byte-identical (S2), the
  bridge transport is LB-side injection (S7) — so upstream sync is a pin bump, not a 3-way merge.
  The binary-COV→JSON re-encode is bounded by S6's frame contract (`cov`/`topology`/`schema` kinds).
- **Interactive latency over the routed hop.** Wiresheet editing against a remote appliance pays
  a Zenoh round trip per command; COV backpressure over the bus. Needs measuring; batch/debounce
  where CE allows.
- **CE is an external, versioned C++ engine.** `ce-client-rust` already documents CE REST bugs it works
  around (`POST /edge` broken → `bulknodes`; name-required 400s). Version skew and CE restarts
  (UID re-numbering) mean the wiresheet must resync via `ce.tree`, never trust cached UIDs.
- **The `net:tcp` escape hatch** on the sidecar is real blast radius; scope it to the exact CE
  host:port from the appliance record and admin-approve at install.
- **Dependency on the extension-watch primitive.** `ce.watch` is the first real tenant of
  `scope/extensions/extension-watch-scope.md`; if that primitive slips, CE falls back to the
  series-bridge live path (no block) but carries two code paths until it migrates — track the
  sequencing.

## Decisions (best long-term calls — resolved at scope time)

The user asked us to make the long-term-best call on each of these; recorded here as decided,
with the rejected alternative, so the implementing session builds against them (not re-litigate).

- **COV surface → `ce.watch`, a streaming tool built on the generic extension-watch primitive
  (primary); series-bridge is the opt-in historian.** Rather than a bespoke CE bus subject or a
  CE-shaped gateway route, `ce.watch` is declared `kind = "watch"` and rides the platform's
  **generic** extension-watch contract (`scope/extensions/extension-watch-scope.md`): the host
  allocates the workspace subject, the sidecar arms its CE-COV publisher on first subscriber, the
  gateway relays SSE — **the core gains a generic streaming primitive, never any CE knowledge.** This
  keeps CE motion on the bus (rule 3) without persisting it as state. Historization stays a
  **separate, opt-in** concern: a per-appliance list of props mirrored onto the **series** plane via
  `ingest.write`, reusing `series.watch`/history — never all COV by default. **Sequencing:** if the
  extension-watch primitive isn't ready when CE v1 lands, live COV ships on the zero-core-change
  `ingest.write`→`series.watch` bridge and migrates to `ce.watch` when the primitive ships — CE is
  never blocked on it. *Rejected: a bespoke `ce/{ws}/{id}/cov` subject + CE-specific SSE route* —
  it would put CE knowledge in core, breaking the 100%-extension invariant.
- **Wiresheet fork strategy → seam upstream, snapshot vendored, bridge LB-side** (revised
  once we confirmed we own `NubeIO/ce-wiresheet`). (1) The generic `EngineTransport` seam is a
  **branch of the upstream repo** (`lb-transport`), a pure refactor kept mergeable to `main`
  (S1). (2) LB vendors that branch **byte-identical** into `packages/ce-wiresheet` as a
  `workspace:*` package (mirrors `packages/nav-rail`), pinned by commit SHA in the package
  README; re-sync is an approval-gated re-copy + pin bump — LB never edits vendored files (S2).
  (3) The LB `BridgeTransport` lives in `rust/extensions/control-engine/ui/`, outside the
  vendored package, injected via the `CeEditor` `transport` prop (S7). *Rejected: carving the
  seam inside the vendored copy* — a permanently divergent fork, every upstream sync a 3-way
  merge. *Rejected: a live git submodule* — fights the workspace resolver, lets upstream drift
  in unreviewed.
- **`ce-client-rust` → pinned git dependency** (reproducible, still update-with-approval), with a
  path dep as the local-dev-only override when both trees are checked out side by side. *Rejected:
  a bare path dep as the committed form* — non-reproducible off this machine.
- **v1 verb cut (decided):** `ce.tree`, `ce.schema`, `ce.add-node`, `ce.patch`, `ce.set-override`,
  `ce.clear-override`, `ce.add-edge`, `ce.remove-node`, `ce.call-action`, `ce.watch` +
  `ce.appliance.add`/`list`/`remove`. **Deferred (additive, same path):**
  `copy`/`restore`/`bulk`/`remove-edge`/`set-layout`, and a large **graph-import-as-`lb-jobs`-job**.
- **Appliance enrollment → reuse as-is.** An appliance is an already-enrolled machine principal
  (`api-keys` `kind="appliance"` + `edge-trust`); `ce.appliance.add` only **records** an enrolled
  node id + its localhost CE base in `ce_appliance:{ws}:{id}`. No new enrollment flow, no new cap
  family.
- **CE auth → optional, record-driven.** The manifest requests the prefix
  `secret:control-engine/*:get`; an appliance record carries a `secret_ref` only when its CE needs
  a token, and the sidecar fetches it (host-mediated) just for that appliance. A localhost-open CE
  configures none and the secret is never fetched.

## Open questions (for the implementing session)

- **Binary-COV → JSON re-encode fidelity.** The wiresheet consumes CE's *binary* COV frames; the
  bridge transport re-encodes them as JSON `ce.watch` events. Confirm the field set the canvas
  actually needs (value + quality + ts + component/prop handle) so we don't ship the whole binary
  layout as JSON. Grounded during the transport fork.
- **Routed-hop interactivity budget.** Measure editing latency against a remote appliance over the
  Zenoh hop; decide whether `ce.patch`/drag needs client-side debounce/coalescing (CE-permitting).

## Related

- README `§3` (symmetric nodes / state-vs-motion / MCP-is-the-contract / capability-first),
  `§6.3` (two-tier runtime), `§6.4` (registry/trust), `§6.13` (gateway SSE), `§13` (manifest).
- `scope/extensions/extensions-scope.md` (manifest contract), `native-tier-scope.md` (sidecar
  supervision), `ui-federation-scope.md` (federated `[ui]` + bridge), `reference-extensions-scope.md`
  (`net:*` caps; `mqtt`/`fleet-monitor` as templates).
- **`scope/extensions/extension-watch-scope.md`** — the generic extension-watch primitive `ce.watch`
  is the first tenant of (the only, and generic, core addition this effort implies).
- `scope/auth-caps/api-keys-scope.md` + `edge-trust-scope.md` (appliance = machine principal).
- `scope/datasources/datasources-scope.md` (the sibling native Tier-2 `federation` extension —
  same `net:*` + mediated-secret + workspace-pinned shape).
- External: `ce-client-rust` (the `ControlEngine` trait), `@nube/ce-wiresheet`
  (the editor package), `rbx-docs/content/control-engine/overview.mdx` (CE architecture).
- **Skill (on ship):** `skills/control-engine/SKILL.md` — the `ce.*` surface is agent-/CLI-drivable,
  so the implementing session writes and maintains a runnable how-to grounded in a live run.
