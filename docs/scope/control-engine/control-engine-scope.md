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
  caps-gated MCP tool surface** mirroring the `rubix-ce` `ControlEngine` trait
  (`tree`/`schema`/`patch`/`set_override`/`call_action`/`add_edge`/…/`watch`).
- **Local mode:** bind a CE over `localhost` REST + binary-WS via the `rubix-ce` client.
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

- **No new CE transport codec.** We do **not** implement `rubix-ce`'s reserved `zenoh` feature
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
`ce.*` tools. It depends on the **`rubix-ce`** crate (`ce-client-rust`), whose `ControlEngine`
trait is already the narrow, transport-replaceable contract for a CE; each `ce.*` tool is a thin,
caps-gated map onto one trait method.

For **remote appliances**, we reuse the platform's single most important property (README §3
rule 1, symmetric nodes): the appliance runs the **same** `control-engine` extension against its
**own** localhost CE. A `ce.*` call carries an `appliance` id; the host resolves it to the owning
node from a workspace-scoped `ce_appliance` record. If that node is *this* node, the sidecar hits
`localhost` CE via `rubix-ce`; otherwise the host **routes the MCP call over Zenoh** to that node
(the existing `rmcp`-over-Zenoh routed hop, `key-stack` "MCP/tool layer") where the same tool
runs locally. There is **no `if cloud`** and **no CE-on-Zenoh codec** — motion (COV, routed
calls) rides Zenoh; state (the appliance registry, optional COV history) lives in SurrealDB
(README §3 rule 3).

**Why MCP-only over a raw CE proxy (user decision):** the user chose "over MCP … so the AI can
use it." A raw proxy would keep `@nube/ce-wiresheet` byte-for-byte unforked but adds a new,
un-gated-by-default host transport surface and gives agents nothing (they can't drive a raw
REST/WS socket through the cap system). Making `ce.*` the one surface means the wiresheet, the
CLI, the central agent, and other extensions all drive CE through the same gate. The cost —
**re-pointing `@nube/ce-wiresheet`'s transport onto the bridge** — is a real fork of the package's
core (its `src/lib/` rest/ws/wire/store layer). The user accepted this ("fork if it makes it
better, or make a branch"): we vendor it to `packages/ce-wiresheet` and maintain a bridge-transport
layer there, **approval-gated** on every change.

**Rejected — `ce-zenoh` direct transport:** implement `rubix-ce`'s `zenoh` feature so the sidecar
speaks Zenoh straight to a CE with `ce-ext-core`'s Zenoh extension. It couples us to a CE-specific
fabric, bypasses LB routing/caps/workspace-walls on the wire, and makes the appliance a bare CE box
instead of a symmetric LB node — losing enrollment, isolation, and the routed-MCP reuse.

**Rejected — raw gated reverse-proxy:** keeps the package unforked but violates rule 7 (a
non-MCP path to a capability) and is dead to agents/CLI.

## How it fits the core

- **Tenancy / isolation.** Every appliance record is `ce_appliance:{ws}:{id}`; every `ce.*` call
  is workspace-first checked, then routed only to an appliance in the caller's workspace. A ws-B
  caller cannot see or target a ws-A appliance (a mandatory isolation test). The routed hop
  carries the caller's workspace claim; the appliance node re-checks it.
- **Capabilities.** Each tool is gated by `mcp:control-engine.<verb>:call` (house convention,
  the manifest NAME is the gate). Read verbs (`tree`/`schema`/`watch`/`appliance.list`) vs write
  verbs (`patch`/`set-override`/`call-action`/`add-node`/`add-edge`/`remove-node`/`appliance.add`/
  `appliance.remove`) are distinct caps. The sidecar itself requests `net:tcp:127.0.0.1:<port>`
  (the native socket escape hatch, like `mqtt`) and, if the CE requires auth,
  `secret:control-engine/<appliance>/token:get` (mediated by `lb-secrets`; the wiresheet never
  sees it). **Deny path:** a caller without `mcp:control-engine.patch:call` calling `ce.patch` →
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
  - **Live feed:** `ce.watch` — COV is *changes, not a snapshot*, so it is a `watch` tool backed
    by a workspace bus subject (`ce/{ws}/{appliance}/cov`) surfaced through the gateway SSE route
    (§6.13), not a polled `tree`. The appliance sidecar subscribes its local CE's binary-WS COV
    and republishes decoded frames onto that subject.
  - **Batch:** `ce.bulk` (one CE round trip) is bounded/always-fast → stays synchronous. A large
    graph **import** (many nodes/edges) MUST be a job (README §6.10), not a blocking loop —
    deferred, named as a follow-up.
- **Data (SurrealDB).** `ce_appliance:{ws}:{id}` = `{ id, name, mode: "local"|"appliance",
  node, base, secret_ref?, ts }` — the registry, state. Optional: bridge selected COV props onto
  the **series** plane (`ingest.write`) for history/replay, reusing the shipped `series.watch`
  SSE (an alternative to a bespoke `ce.watch` subject — see Open questions).
- **Bus (Zenoh).** Two motions: routed `ce.*` MCP calls (request/response, must-reach-the-node
  *online*), and COV frames (`ce/{ws}/{appliance}/cov`, fire-and-forget live feed, replayable
  from series if bridged).
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
   node:"edge-7", base:"http://127.0.0.1:7878" }` (gated `mcp:control-engine.appliance.add:call`).
   A `ce_appliance:{ws}:plant-1` record persists.
2. A user opens the **Control Engine** page (the vendored `@nube/ce-wiresheet`, mounted federated).
   It picks `plant-1` and calls `ce.tree { appliance:"plant-1" }` through the host bridge.
3. The cloud host checks `mcp:control-engine.tree:call` (workspace-first), resolves `plant-1` →
   node `edge-7`, and **routes the MCP call over Zenoh** to `edge-7`.
4. On `edge-7`, the local `control-engine` sidecar runs `ce.tree` against its `localhost` CE via
   `rubix-ce.get_tree(...)`, returns the `Tree`; the response rides the bus back; the wiresheet
   renders it on React Flow.
5. The user drags/edits → `ce.patch { appliance:"plant-1", node, props }` → same gate → same
   routed hop → `rubix-ce.patch(...)` on `edge-7`. CE applies it; CE is the authority.
6. The page opens `ce.watch { appliance:"plant-1" }` → gateway SSE over the
   `ce/{ws}/plant-1/cov` subject; `edge-7`'s sidecar bridges CE COV onto it; live values stream to
   the canvas with no polling.
7. A ws-B user calling `ce.tree { appliance:"plant-1" }` is denied at gate 1 (not their workspace)
   — the appliance is invisible and unreachable across the wall.

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
  Rust CI): stubbed behind `rubix-ce`'s `ControlEngine` trait in **one** named file
  (`ce_fake.rs`), OR — preferred where feasible — a tiny **real** localhost HTTP/WS server
  speaking the CE `/api/v0` + `/ws` subset so even the `rubix-ce` transport is real. Pick one,
  name it, keep it to one file behind the trait.
- **Offline behavior.** Appliance node unreachable → `ce.*` returns a loud error; assert it does
  **not** silently queue.
- **Hot-reload / restart.** The native sidecar restart loses no durable state — the appliance
  registry lives in SurrealDB; a respawn re-reads it (stateless-extension guarantee).
- **UI (real gateway, rule 9).** `pnpm test:gateway` — the vendored wiresheet drives a real
  spawned node: `ce.tree` renders, a `ce.patch` round-trips, `ce.watch` streams a COV frame, and
  the workspace/deny paths hold. No `*.fake.ts`.

## Risks & hard problems

- **Forking `@nube/ce-wiresheet`'s transport is the biggest lift.** Its `src/lib/` (rest/ws/wire/
  store) assumes a direct CE REST + **binary** WS. Re-pointing it onto `bridge.call('ce.*')` +
  `bridge.watch` means re-encoding the binary COV codec into JSON frames and keeping the vendored
  copy in sync with upstream — under the **approval-gated** update rule. Contain the change to a
  single swappable transport module in the package.
- **Interactive latency over the routed hop.** Wiresheet editing against a remote appliance pays
  a Zenoh round trip per command; COV backpressure over the bus. Needs measuring; batch/debounce
  where CE allows.
- **CE is an external, versioned C++ engine.** `rubix-ce` already documents CE REST bugs it works
  around (`POST /edge` broken → `bulknodes`; name-required 400s). Version skew and CE restarts
  (UID re-numbering) mean the wiresheet must resync via `ce.tree`, never trust cached UIDs.
- **The `net:tcp` escape hatch** on the sidecar is real blast radius; scope it to the exact CE
  host:port from the appliance record and admin-approve at install.
- **COV transport choice** (bespoke `ce.watch` subject vs bridging into the series plane) affects
  history, replay, and load — decide before the UI locks its subscribe path.

## Open questions

- **COV surface:** a dedicated `ce/{ws}/{id}/cov` bus subject + `ce.watch` SSE, **or** bridge
  selected COV props onto host **series** via `ingest.write` and reuse the shipped `series.watch`
  (history + replay for free, at the cost of series churn per sample)? Lean bespoke-subject for
  the live canvas, series-bridge as an opt-in per-prop historian.
- **Vendoring mechanics:** `packages/ce-wiresheet` as a **copied** `workspace:*` package (like
  `packages/nav-rail`) vs a git submodule/branch of `ce-wiresheet`. And `rubix-ce`: a path dep vs
  a pinned git dep. Both are "update with approval first" per the user.
- **v1 verb subset vs full trait-mirror:** confirm the v1 cut
  (`tree`/`schema`/`add-node`/`patch`/`set-override`/`clear-override`/`add-edge`/`remove-node`/
  `call-action`/`watch` + `appliance.add`/`list`/`remove`) and defer
  `copy`/`restore`/`bulk`/`remove-edge`/`set-layout` + graph-import-as-job.
- **Appliance enrollment:** confirm we reuse `api-keys` (`kind="appliance"`) + `edge-trust`
  as-is, with `ce.appliance.add` recording an already-enrolled node id (no new enrollment flow).
- **CE auth:** does a target CE require a token (→ `secret:control-engine/<appliance>/token:get`)
  or is localhost open in practice? Drives whether the secrets cap is in the v1 request set.

## Related

- README `§3` (symmetric nodes / state-vs-motion / MCP-is-the-contract / capability-first),
  `§6.3` (two-tier runtime), `§6.4` (registry/trust), `§6.13` (gateway SSE), `§13` (manifest).
- `scope/extensions/extensions-scope.md` (manifest contract), `native-tier-scope.md` (sidecar
  supervision), `ui-federation-scope.md` (federated `[ui]` + bridge), `reference-extensions-scope.md`
  (`net:*` caps; `mqtt`/`fleet-monitor` as templates).
- `scope/auth-caps/api-keys-scope.md` + `edge-trust-scope.md` (appliance = machine principal).
- `scope/datasources/datasources-scope.md` (the sibling native Tier-2 `federation` extension —
  same `net:*` + mediated-secret + workspace-pinned shape).
- External: `ce-client-rust` (`rubix-ce`, the `ControlEngine` trait), `@nube/ce-wiresheet`
  (the editor package), `rbx-docs/content/control-engine/overview.mdx` (CE architecture).
- **Skill (on ship):** `skills/control-engine/SKILL.md` — the `ce.*` surface is agent-/CLI-drivable,
  so the implementing session writes and maintains a runnable how-to grounded in a live run.
