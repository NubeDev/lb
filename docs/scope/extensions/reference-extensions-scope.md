# Extensions scope — five reference extensions (native-first) + the platform fixes they need

Status: scope (the ask). Promotes to `public/extensions/extensions.md` once shipped.

Five concrete extensions a workspace owner would actually install — a markdown doc store with
PDF export, a todo app, an MQTT pub/sub bridge, a Timescale time-series connector, and a
Zenoh appliance/workstation gateway. They are chosen to **exercise the whole extension
surface** end to end: native (Tier-2) backends that own real external resources (a PDF
engine, an MQTT socket, an external DB connection, a raw Zenoh session), the secret store, the
ingest/series data plane, the bus + SSE, and a federated UI page each. This doc specifies all
five **and the platform gaps that block them today**, so the build order is unambiguous: the
fixes land first, then the five fall out cheaply.

> Read with: `extensions-scope.md` (the manifest contract), `native-tier-scope.md` (the Tier-2
> supervisor), `host-callback-scope.md` (the guest→host call-back — the foundational gap),
> `../secrets/secrets-scope.md`, `../ingest/ingest-scope.md`, `../auth-caps/auth-caps-scope.md`
> (the capability grammar these extend), `../../README.md` §3 (the core rules), §6.3 (two tiers).

---

## Doctrine: rule 2 holds; a native extension is the escape hatch

README §3 rule 2 ("one datastore — SurrealDB only") governs the **platform's own state** —
identity, capabilities, channels, inbox/outbox, jobs, series, assets. That never moves off
SurrealDB. It is **not** a ban on an extension talking to the outside world.

A **native (Tier-2) extension is a supervised OS process** (`native-tier-scope.md`). It is the
deliberate escape hatch for everything a sandboxed wasm guest can't do: open a TCP socket, link
a PDF/crypto/codec library, hold a connection pool to an external database, or join a Zenoh
mesh. The rules that keep this safe, not anarchic:

1. **The external resource is the extension's, not the platform's.** Timescale data, MQTT
   topics, and an appliance's Zenoh state are reachable **only through that extension's MCP
   tools** — never promoted to a second store for *platform* records. Rule 2 is about
   lazybones state; this is the extension's own data behind the MCP wall.
2. **Every external resource is capability-gated.** A native extension reaches the network /
   filesystem / an external DB **only** through a declared capability the admin approved at
   install (rule 5). This needs **one new capability family** — `net:*` (below) — because the
   current grammar (`store:`, `mcp:`, `secret:`, `bus:`) has no word for "may open this socket".
3. **It stays stateless toward the platform** (rule 4): the extension's *durable platform*
   state lives in SurrealDB via host tools; its external connection is a runtime resource the
   supervisor can kill and respawn. A crashed MQTT bridge reconnects; it loses no lazybones
   state because it kept none.
4. **One named trait, one file** (CLAUDE §9, FILE-LAYOUT): the external client (an MQTT client,
   a Postgres pool, a Zenoh session) lives behind one trait in one clearly-named file in the
   extension's crate — the same discipline as the GitHub "true external" carve-out.

So: **native-first.** All five are native Tier-2. WASM Tier-1 remains for pure, sandboxed
transforms (e.g. `github-bridge.normalize`); none of these five is pure, so none is wasm.

---

## Goals

- Ship five installable reference extensions, each a **single folder = native backend + federated
  UI page**, proving the full native-extension surface with no placeholders (CLAUDE §9).
- Establish the **`net:*` capability family** so a native extension can own an external socket /
  DB / mesh under admin-approved, workspace-walled authority.
- Land the **native host-callback transport** (child→host MCP) so a sidecar can do real platform
  work — `ingest.write`, `series.latest`, `inbox.resolve`, `outbox` — under `caller ∩ grant`.
- Add the **two small platform verbs** the set needs and proves are missing: a generic
  per-extension record store (`kv.*`, for the todo app) and a binary-artifact write path (for
  the PDF, since `DEFINE BUCKET` is unavailable in the current engine build).

## Non-goals

- No change to rule 2, rule 4, or rule 5. The doctrine above *applies* them; it does not relax
  them.
- No wasm Tier-1 variants of these five (they all need native resources).
- No new model-provider work; no doc-site/native-window work.
- Not building the host-callback **wasm** half here — that is `host-callback-scope.md`. This doc
  needs its **native** sibling (the same chokepoint, a different transport).

---

## Intent / approach

**The five extensions are easy; the platform owes them four things.** Each extension is a thin
native sidecar plus a page. What makes them buildable is closing four gaps, in order:

### Platform fix 1 — native host-callback transport (the foundational unlock)

Today a native sidecar (`native-tier-scope.md`) is supervised over stdio and the host calls
*into* it (`tool.call`). The **child→host MCP callback** is a listed follow-up and is the thing
that blocks every backend here. It is the **native transport for the same chokepoint**
`host-callback-scope.md` defines for wasm: the sidecar emits a framed `call-tool(name, input)`
request on its stdio channel; the supervisor dispatches it through
`lb_host::call_tool(node, &effective_principal, ws, name, input)` — authorize-then-dispatch,
`effective_principal = caller ∩ install-grant`, `ws` host-set from the invoking token, never
sidecar-supplied. Same security property, same deny tests, second front door (rule 7). Build
this once; all five backends use it.

### Platform fix 2 — the `net:*` capability family

The capability grammar (`auth-caps`) has no term for outbound network. Add one, the minimal
shape that an admin can read and approve:

```
net:tcp:<host>:<port>      # e.g. net:tcp:broker.local:1883   (MQTT)
net:tls:<host>:<port>      # TLS variant (MQTTS, Postgres-over-TLS)
net:zenoh:<scope>          # join a Zenoh scope/key-prefix as a peer (appliance mesh)
```

The supervisor enforces it at connect time: a native extension may open a socket **only** to a
host:port its install grant covers (`requested ∩ admin_approved`, same as every other cap).
Deny is opaque; the deny test is "connect to a port the grant omits → refused, even with the
binary present". This is what makes "user can add any extension they want" safe — the admin sees
exactly which endpoints an extension will reach before approving.

### Platform fix 3 — generic per-extension record store (`kv.*`)

The todo app surfaced a real gap: there is **no generic store verb for arbitrary extension
records** — only `ingest`/`series`, `assets`/`docs`, and `tags`. Add a small host service
exposing `kv.put` / `kv.get` / `kv.list` / `kv.delete`, namespaced `kv:{ws}:{ext}:{key}` (the
`ext` prefix from the install, never caller-supplied), gated `mcp:kv.<verb>:call`. SurrealDB
records, workspace-walled, the one datastore — this is **not** a new persistence layer, it's a
typed table behind MCP. Many future extensions need exactly this and shouldn't each grow a
bespoke host service.

### Platform fix 4 — binary-artifact write path

`assets` stores content **as records** (the current engine has no `DEFINE BUCKET`, a documented
S8 finding). A generated PDF is binary and can be large. Add `assets.put_blob` / `assets.get_blob`
(chunked record storage behind the existing `assets` gate, `store:doc/*`) so the doc-store
extension can persist an exported PDF without a bucket. When the engine gains buckets (a config
swap), the verb's storage backend changes; the MCP shape does not.

**Why native-first and not wasm + more host services.** The status quo forces every backend that
touches an external resource into a *host service* (github-bridge had to). That doesn't scale —
the platform can't grow a bespoke service per user extension, and an external connection
(MQTT/Timescale) has no business living in core. Native Tier-2 + the callback + `net:*` makes
"ship your backend in your extension" finally true, with the wall intact.

---

## The five extensions

Each is `rust/extensions/<id>/` — a native sidecar (own crate, behind a `Launcher`, supervised)
plus a co-located federated `ui/` page, one manifest. Tools are declared in `extension.toml`
and gated `mcp:<id>.<tool>:call`.

### 1. `doc-store` — markdown doc store + PDF export

- **Tier:** native (PDF engine is a native crate).
- **Tools:** `doc-store.create` / `update` / `get` / `list` (markdown docs) · `doc-store.export_pdf`
  (render markdown → PDF, persist via `assets.put_blob`, return the blob id).
- **Caps requested:** `store:doc/*` (docs + the blob), `mcp:assets.*:call`, `mcp:kv.*:call`
  (optional, for index). **No `net:*`** — fully local.
- **Data/motion:** docs reuse the shipped `lb-assets` model + the S4 three-gate authz
  (private→team→workspace). The PDF is a blob (fix 4). No motion.
- **Depends on:** fix 1 (callback to read docs + write the blob), fix 4 (blob write).
- **Notes:** this is the gentlest — most of it (`assets`, `DocView`) already shipped; export +
  blob storage is the only genuinely new code.

### 2. `todo` — a workspace todo app

- **Tier:** native (could be the one that's *almost* wasm, but kept native for one toolchain).
- **Tools:** `todo.add` / `todo.list` / `todo.toggle` / `todo.delete`.
- **Caps requested:** `mcp:kv.*:call` only. No `net:*`, no secret.
- **Data/motion:** records via the generic `kv.*` store (fix 3), `kv:{ws}:todo:{id}`. No motion
  (a `list` re-read is fine; live multi-user sync is a follow-up over channel motion).
- **Depends on:** fix 1 (callback), fix 3 (`kv.*`).
- **Notes:** the smallest *real* proof of the native-backend + page loop. Good first extension
  to build once fixes 1 + 3 land — it needs nothing exotic.

### 3. `mqtt-bridge` — MQTT pub/sub ingress/egress

- **Tier:** native (holds a long-lived MQTT socket — impossible in wasm).
- **Tools:** `mqtt.subscribe` (register a topic→series mapping) · `mqtt.publish` (UI/agent → MQTT)
  · `mqtt.status`.
- **Caps requested:** `net:tcp:<broker>:1883` (or `net:tls:…:8883`), `secret:mqtt-bridge/*` (the
  broker password — mediated, never logged, `lb-secrets`), `mcp:ingest.write:call`,
  `mcp:bus.publish:call` (optional, for channel motion).
- **Data/motion:** **inbound** MQTT message → the sidecar calls `ingest.write` via the callback →
  series → UI sees it over the **already-shipped SSE** (`GET /series/{s}/stream`). **Outbound**
  `mqtt.publish` is a host→sidecar `tool.call`. This is the github-webhook **ingress pattern**
  generalized to a long-lived connection.
- **Depends on:** fix 1 (callback for `ingest.write`), fix 2 (`net:*` for the socket), the
  secret store (must be built/wired — see Open questions).
- **Notes:** the richest of the five — exercises native socket + secret + ingest + SSE + the
  supervisor's restart (a dropped broker connection reconnects, losing no platform state).

### 4. `timescale` — external time-series connector

- **Tier:** native (a Postgres/Timescale client + connection pool).
- **Tools:** `timescale.query` (read a range → rows) · `timescale.write` (push samples out) ·
  `timescale.status`.
- **Caps requested:** `net:tls:<host>:5432`, `secret:timescale/*` (connection string),
  `mcp:series.*:call` (optional, to mirror into the local series plane).
- **Data/motion:** **this is the doctrine case.** Timescale holds *the extension's* data; rule 2
  is untouched because none of it is platform state. The extension exposes it through `timescale.*`
  MCP tools; a caller never sees a raw DB handle (rule 5). The external client lives behind one
  trait in one file.
- **Depends on:** fix 1, fix 2, the secret store.
- **Notes:** include a short "when NOT to reach for this" — for *lazybones'* own time-series, use
  the shipped `lb-ingest` + `series.*` + `lb-tags` plane (that IS the platform's Timescale).
  `timescale` is for bridging an **existing external** warehouse, not a second internal store.

### 5. `zenoh-gateway` — appliance / workstation mesh access

- **Tier:** native (joins a Zenoh mesh as its own peer session).
- **Tools:** `zenoh.discover` (list live appliances/workstations on the scope) · `zenoh.read`
  (pull a resource) · `zenoh.command` (push to a device) · `zenoh.bridge` (map a Zenoh key →
  local series, like `mqtt.subscribe`).
- **Caps requested:** `net:zenoh:<scope>`, `mcp:ingest.write:call`, `mcp:bus.publish:call`.
- **Data/motion:** appliances are **nodes** on the mesh (symmetric nodes, edge role) or raw
  Zenoh devices; this extension is the **bridge** from that mesh into the workspace's series/bus
  so the UI sees them over SSE. The extension holds its **own** Zenoh session as a `net:zenoh`
  resource — it does **not** get the platform's core bus handle (rule 4/5); device↔device
  routing stays a node concern.
- **Depends on:** fix 1, fix 2 (`net:zenoh`).
- **Notes:** clarifies the boundary the user asked about — "Zenoh access for appliances" is a
  *bridging extension* over `net:zenoh`, not a raw bus handle handed to arbitrary code.

---

## How it fits the core

- **Tenancy / isolation:** every tool authorizes workspace-first; the callback's `ws` is
  host-set from the token, never sidecar-supplied. `kv.*` / `net:*` / blobs are all
  `{ws}`-keyed. Two-workspace isolation is a mandatory test on each new verb and on `net:*`
  (ws-B's `mqtt-bridge` reaches none of ws-A's topics/series).
- **Capabilities:** the deny path is the headline. Mandatory denies: a verb the install grant
  omits → denied even if the caller holds it (callback narrowing); a `net:*` endpoint the grant
  omits → connection refused; `kv`/`assets`/`ingest` per-verb deny.
- **Placement:** `either` for all five (symmetric). A native sidecar runs where scheduled; a
  routed `<ext>.<tool>` uses the existing queryable path. No `if cloud`.
- **MCP surface (API shape, §6.1):** CRUD + get/list on `kv.*`, `doc-store.*`, `todo.*`; a
  **live feed** is the existing series SSE (no new `watch` verb — the bridges write series and
  the UI streams them); **batch** — none runs unbounded here, but `doc-store.export_pdf` of many
  docs and `timescale.query` of a large range MUST become **jobs** (§6.10) if unbounded (stated,
  with the bound).
- **Data (SurrealDB):** `kv:{ws}:{ext}:{key}` (fix 3), `doc`/blob records (fix 4) — all the one
  datastore. External stores (Timescale) are the extension's, behind MCP, never platform state.
- **Bus (Zenoh):** bridges publish series motion (`ingest`'s `publish_sample`) and optionally
  channel messages; must-deliver effects (none here, but e.g. an MQTT command ACK) would go
  through the **outbox**, not raw pub/sub.
- **Secrets:** `mqtt-bridge` and `timescale` pull credentials from `lb-secrets` via
  `secret:<ext>/*`; the secret never reaches the page and is never logged.
- **SDK/WIT impact:** the **native** callback transport is a forever-shaped boundary (the framed
  child→host protocol) — flag it; it mirrors `host-callback-scope.md`'s WIT import exactly so the
  two transports stay one contract. `net:*` is a capability-grammar addition (auth-caps), not a
  WIT change.

## Example flow (the MQTT bridge, end to end)

1. Admin installs `mqtt-bridge`, approving `net:tls:broker.acme:8883`, `secret:mqtt-bridge/*`,
   `mcp:ingest.write:call`. The host stores `granted = requested ∩ approved`.
2. The supervisor spawns the sidecar; at connect time it checks `net:tls:broker.acme:8883` is in
   the grant (else refuse), pulls the password via `secret:mqtt-bridge/password`, and opens the
   MQTT socket.
3. A device publishes to `acme/cooler/temp`. The sidecar maps it to series `cooler.temp` and
   emits a framed `call-tool("ingest.write", {series:"cooler.temp", payload:4.0, …})`.
4. The supervisor dispatches it through `call_tool` against `effective_principal = caller ∩
   grant` in the install's `ws` → authorizes `mcp:ingest.write:call` → stages + drains.
5. `POST /ingest` publishes motion; the dashboard's `GET /series/cooler.temp/stream` SSE shows
   4.0 live. The page never held a token or a socket.
6. **Deny path:** install without `net:tls:broker.acme:8883` → step 2 refuses the connect, opaque
   error, sidecar marked degraded — even though the binary is perfectly capable.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all against the **real** supervisor +
real store + real caps + a real socket where applicable (no mocks; the broker/Timescale/Zenoh
peer is a true external behind one trait — the only sanctioned fake, §0):

- **Capability deny — per verb and per `net:*` endpoint:** a tool the grant omits → `Denied`; a
  socket/endpoint the grant omits → connect refused. Both directions of the callback intersection
  (caller-lacks and grant-omits).
- **Workspace isolation:** ws-B's `kv.*`, blobs, MQTT topic map, and series are invisible to
  ws-A; the callback `ws` is un-spoofable.
- **Offline / restart (supervision):** kill the sidecar mid-connection → respawn → reconnects →
  **no platform state lost** (a series sample written before the kill is intact); the external
  reconnection is the sidecar's, the durable truth is in SurrealDB.
- **Happy round-trips:** MQTT inbound → series → SSE; `todo` add→list; `doc-store` create→export
  PDF→read blob; `timescale.query` over a seeded external; `zenoh.discover` against a real peer.
- **Frontend (real gateway):** each extension's page over the bridge — `ProofPanel`-style
  `*.gateway.test.tsx` + a Playwright e2e per page (e.g. publish an MQTT message, see the series
  tick in the dashboard).

## Risks & hard problems

- **The native callback transport is the long pole.** Framing, back-pressure, re-entrancy/depth
  (a sidecar calling back while the host is mid-`tool.call` into it), and a dead-child mid-callback
  must all be specified — reuse `host-callback-scope.md`'s depth-guard + fresh-instance discipline.
- **`net:*` enforcement must be real, not advisory.** If the supervisor can't actually prevent a
  socket outside the grant, the capability is theater. Decide the enforcement point (pre-connect
  check is the minimum; OS-level egress filtering is the hardening follow-up).
- **Secret handling.** The password/connection-string must reach the sidecar without ever
  touching a log, the page, or a record. Confirm `lb-secrets` is built and the mediation path
  exists (Open questions).
- **"User can add any extension" is the threat model.** An AI-written or third-party native
  sidecar runs as a real process. The whole safety story is install-time admin approval of an
  explicit `net:*` + cap set, plus process-group isolation (native-tier posture). The tests must
  prove the deny path bites a *real* binary, not a displayed message.
- **Blob size.** PDFs/large queries can be big; chunked records + a size cap + jobs for unbounded
  work (don't block a tool handler).

## Open questions

1. **Is `lb-secrets` built or still scoped?** STATUS references `secret:<ext>/*` but the secrets
   slice isn't on the shipped list. If unbuilt, it becomes **platform fix 5** and gates
   `mqtt-bridge` + `timescale`. Resolve before sequencing.
2. **`net:*` granularity** — `host:port` (proposed) vs. coarser `net:outbound` vs. finer
   (per-protocol verbs)? Lean: `host:port` — readable by an admin, enforceable pre-connect.
3. **`kv.*` shape** — typed records vs. opaque JSON blobs? Lean: opaque JSON value + a string key,
   workspace+ext-namespaced; tagging via the existing `tags` graph if an extension wants query.
4. **Native callback re-entrancy depth** — share the constant with the wasm host-callback (#1
   there). Lean: one shared small fixed limit.
5. **Build order** — fixes 1+2 are foundational; then `todo` (proves callback+kv), then
   `mqtt-bridge` (proves net+secret+ingest+SSE), then `doc-store`/`timescale`/`zenoh-gateway`.
   Confirm.

## Related

- `extensions-scope.md` — the manifest contract (these add `net:*` to `[capabilities] request`).
- `native-tier-scope.md` — the Tier-2 supervisor these build on (its child→host callback
  follow-up is **platform fix 1**).
- `host-callback-scope.md` — the **wasm** half of the same chokepoint; fix 1 is its native dual.
- `../secrets/secrets-scope.md` — the credential mediation `mqtt-bridge`/`timescale` need.
- `../ingest/ingest-scope.md` — the series plane the bridges write into; the lazybones-native
  answer to "Timescale".
- `../auth-caps/auth-caps-scope.md` — the grammar `net:*` extends.
- `proof-panel-scope.md` / `ui-federation-scope.md` — the federated page + bridge each page uses.
- README `§3` (rules 2/4/5/7), `§6.3` (two tiers), `§11.2` (the forever ABI the callback touches).
</content>
</invoke>
