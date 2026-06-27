# 0003: Worked example — the B2B Fleet IoT Dashboard (KFC & McDonald's)

A concrete, end-to-end walkthrough of a **B2B IoT monitoring product** built **entirely as
extensions** on the Lazybones core. As with the coding workplace (0002), nothing here is
special-cased in the platform: the "IoT Dashboard" is a composition of core primitives —
workspaces, users, teams, capabilities, the timeseries/ingest data plane, tags, the bus,
jobs, MCP tools, and the shared UI shell — arranged across the four deployment **personas**
(README §5): `appliance`, `workstation`, `mobile`/`browser`, and `hub`.

The product is sold to **restaurant chains** that must monitor kitchen and refrigeration
equipment across hundreds of stores — walk-in coolers and freezers (food-safety temperature
logging), fryers, and HVAC. **Two customers run on the one hub: KFC and McDonald's.** They are
**two separate workspaces** and the headline of this probe is that they **never share data and
never cross paths** — the workspace is the hard wall (README §3.6, §7). This is the **B2B** shape:
one workspace = one company. (Its B2C sibling — one workspace *per household*, with guest invites —
is `vision/0004-consumer-iot.md`.)

Read this as a design probe. It exercises the whole node model at once: headless edge nodes
producing data, a cloud hub owning it, two tenants walled from each other, and humans viewing it
from a desktop, a phone, or a browser. Every place it reaches for something the core doesn't yet
offer is a **finding** for a scope, not a feature to bolt onto the kernel.

> **The one thing to take away:** the core never knows the word "fryer" or "sensor." It knows
> series, samples, tags, capabilities, presence, and a routed MCP namespace served as one app.
> The dashboard is the *arrangement* of those — and the same app the `workstation` runs locally
> is the app the `hub` serves to a `browser`. KFC and McDonald's run the *same* extensions over
> the *same* hub, each in its own walled workspace.

---

## 1. The product, across four personas

A restaurant chain monitors kitchen and refrigeration equipment across its stores. The deployment
(shown for one chain; KFC and McDonald's each get the identical stack in their own workspace):

| Persona | Role | Hardware | What it does here |
|---|---|---|---|
| **`appliance`** | edge | Raspberry-Pi in a store's kitchen | Headless. Reads local probes (cooler/freezer temps, fryer state), writes **samples** into its own SurrealDB, syncs up to the hub. Runs offline through a network drop. |
| **`workstation`** | edge | a field tech's desktop | The full app in **Tauri**, talking to its **local** host. The tech's own node — local data, offline-capable. |
| **`mobile`** / **`browser`** | client | phone / any browser | Thin clients. A store manager or regional ops lead **logs in to the hub**, sees dashboards served over SSE/HTTP. No local store. |
| **`hub`** | hub | cloud server | Authority for shared data; Zenoh router; **serves the dashboard app** to browser/mobile clients; hosts the registry + (optional) AI gateway. **One hub hosts both KFC and McDonald's**, each walled in its own workspace. |

All one binary; the difference is **config and role** (§3 rule 1). The dashboard is delivered
as a small bundle of registry artifacts, installed per workspace with only the capabilities
the admin granted:

| Extension | Tier / placement | Role |
|---|---|---|
| `sensor-source` | native (Tier-2), `local-only` | On an `appliance`/`workstation`: reads hardware (GPIO/Modbus/serial — temp probes, fryer controllers), emits canonical **`Sample` envelopes** into the ingest buffer. Native because it touches local hardware. |
| `dashboard` | WASM, `either` | The dashboard UI extension + its tools (`panel.query`, `series.find`). Reads series/tags; renders panels. Holds **no** durable state. |
| `alerts` | WASM, `cloud-only` | Watches incoming series for threshold breaches (e.g. a walk-in cooler above 5 °C — a food-safety event); raises **inbox items** and queues **outbox** notifications. The orchestrator — stateless. |

"On the appliance" vs "on the hub" is **placement**, not a privilege tier (§6.6, §7).

---

## 2. How it maps to the core

- **Workspace** `kfc` = the tenant = the hard wall. One SurrealDB namespace, one
  `ws/kfc/**` bus prefix, its own secrets. Every series, panel, alert, and appliance below
  lives inside it. **`mcdonalds` is a second workspace on the same hub and sees none of it** —
  not "we check a flag," but structurally: a `mcdonalds` principal cannot even *name* a
  `ws/kfc/**` key, and a query for `mcdonalds` selects a different SurrealDB namespace, so KFC's
  fryer temps are physically unreachable (tenancy-scope.md). **The two chains never cross paths.**
- **Users / teams** — `operators` (view dashboards), `store-techs` (view + ack alerts),
  `admins` (manage appliances + grants). Global identities, members of `kfc` (a regional manager
  who consults for both chains would be **one identity with a membership in each workspace**,
  switching between them — never a bridge that leaks data across the wall).
- **Appliances are principals *and* resources** — each appliance authenticates with a
  workspace-bound **API token** (node credential) and is itself an **access-controlled
  resource**: an admin reaches any appliance; an operator is granted only the ones for their
  store (`node-connection-scope.md`).

### Data plane (the part that already largely exists — S8)

- **Ingest:** `sensor-source` emits the canonical `Sample` envelope
  `{ series, producer, ts, seq, payload, labels, qos }` into the durable, exactly-once ingest
  buffer (`ingest-scope.md`). A "device" is just a **producer**; dedup is `(series, producer,
  seq)`. **No device registry** — that was deliberately rejected (`ingest-scope.md`).
- **Store:** samples land in SurrealDB **timeseries** (record-ID ranges + rollup views). The
  one datastore, on every node (§3 rule 2).
- **Tags:** every series is tagged (`store:downtown-0421`, `kind:temperature`, `unit:degC`,
  `equipment:walk-in-cooler`) via the tags service; the dashboard discovers series by **faceted
  tag search + `series.find`**, not a hand-maintained list. The same tags drive **grant-by-tag**
  ("operators see all appliances tagged `region:emea`").

### Motion vs. state

- **Live values** stream over the **bus** (Zenoh): a panel subscribes to a series key and
  updates in real time — motion, never a polling loop on the store (§3 rule 3).
- **History** is a store query (timeseries rollups). State.
- **Appliance online/offline** is **Zenoh liveliness** (the fleet roster,
  `fleet-presence-scope.md`), not a stored flag.

---

## 3. End-to-end flow

1. **Provision.** A KFC admin issues an appliance API token (`appliance.token.issue`) and grants
   `operators` access to appliances tagged `region:emea`. The store's Pi is configured
   `LB_ROLE=edge`, `LB_ZENOH_CONNECT=tcp/hub:7447`, `LB_STORE_PATH=/data`, + its token/cert.
2. **Connect.** The appliance boots, opens a Zenoh peer to the hub's router (mTLS), authenticates,
   and **announces node presence** → it appears **online** in KFC's admin fleet roster (and never
   in McDonald's).
3. **Produce.** `sensor-source` reads the walk-in cooler probe every second → emits `Sample`s into
   the ingest buffer → durable in the appliance's local SurrealDB. **This works offline**: if the
   hub link drops, samples keep buffering locally.
4. **Sync up.** As an `edge` node the appliance is *not* authoritative for shared data — it queues
   its writes through the **outbox** to the hub (authoritative, §6.8). On reconnect, idempotent
   apply merges them into the `kfc` namespace; nothing is lost across the outage.
5. **Alert.** On the hub, `alerts` sees the cooler cross its threshold (above 5 °C — a food-safety
   event) → raises an **inbox item** (`{series, value, store}`) and queues an **outbox**
   notification (email/SMS/push) for `store-techs`. Must-deliver → outbox, not raw pub/sub
   (§3 durability).
6. **View — cloud dashboard.** A KFC ops lead opens a **browser**, logs in to the hub. The hub serves
   the **same** dashboard app a `workstation` runs locally. Their JWT carries `ws:kfc` + caps →
   Gate 1 (workspace) ✓, Gate 2 (panel/appliance caps) ✓ → they see **only** KFC's EMEA dashboards,
   never a single McDonald's reading. `panel.query` returns history; live values stream over SSE.
   A `mobile` (Flutter) client is the same path on a phone.
7. **View — workstation.** A field tech's `workstation` runs the same app in Tauri against
   its **local** host, against locally-synced data — works in the kitchen with no connectivity.
8. **Acknowledge.** A `store-tech` acks the alert inbox item via an MCP tool → an outbox row records
   the ack and notifies the team channel.
9. **Revoke.** A decommissioned appliance: admin runs `appliance.token.revoke` → on next reconnect
   it's refused; the roster shows it offline.

The core never knew "fryer", "cooler", or "dashboard" — only series, samples, tags, inbox/
outbox, capabilities, presence, and one app served two ways. And it never knew "KFC" or
"McDonald's" either — only two workspaces it kept structurally apart.

---

## 4. The hub serves the same app the workstation runs (the key idea)

This example only works because **the dashboard UI is one app with two deliveries** (README §6.12):

- **`workstation`** bundles it in **Tauri**, talking to the **local** host. One user, local data,
  offline.
- **`hub`** serves it to **`browser`/`mobile`** clients over **SSE/HTTP**. Many users log in; data
  lives on the hub; access is scoped per token.

Same React/TS app, same MCP tools (`panel.query`, `series.find`, `alerts.ack`), same capability +
workspace wall. The only differences are **delivery** (Tauri-local vs. browser-remote) and **where
the data lives** (the user's own node vs. the hub). The hub doesn't *become* a workstation — it
**hosts the UI** for thin clients. That is the whole "cloud dashboard."

---

## 5. Why this is a good design probe

It stresses every axis of the node model in one product:

- **Symmetric nodes** — appliance, workstation, and hub are the same binary; only config differs.
- **Edge authority + sync** — appliances produce, the hub owns; offline + reconnect is the headline.
- **State vs motion** — history (store) vs live values (bus) vs online (liveliness), kept distinct.
- **Capability + workspace wall** — KFC and McDonald's as two walled tenants on one hub, with
  per-appliance, per-store access inside each.
- **One app, two deliveries** — the cloud dashboard is the workstation UI served to browsers.
- **Extensions all the way down** — hardware I/O, dashboards, and alerting are installable artifacts;
  the kernel stays generic.

---

## 6. Scaling to a fleet (Niagara-style: thousands of stores)

The single-store walkthrough above scales to a chain's whole fleet in the spirit of
Tridium Niagara without changing the model — only config and topology. KFC and McDonald's each
scale this way **independently inside their own workspace**:

- **Peer-to-peer within a store.** Appliances are **Zenoh peers**: they exchange live points
  **directly on the LAN** (no hub round-trip) for local control, and connect **up** to a hub when
  reachable (§6.5). The hub is out of the hot path; it still **owns** the durable shared record (§6.8).
- **~1000s of stores.** Each store is one (or a few) appliances peering locally and connecting to a
  **regional hub**; regional hubs aggregate to a central hub — all config, no code branch (§3 rule 1).
  **Workspaces** partition the fleet (per tenant — `kfc` vs `mcdonalds` — and optionally per region),
  so the hard wall also bounds roster size and blast radius.
- **Grant-by-tag at scale.** You don't hand-grant 1000 nodes: an operator is granted appliances
  **by tag** (`region:emea`, `store:0421`) — the tag-based grant form of `authz-grants-scope.md`.
- **Custom role-gated pages** (energy / water / HVAC scheduling). Each page is a **frontend
  extension** whose actions are MCP tools (`energy.*`, `hvac.schedule.*`); every action re-runs the
  **three gates**, so a page can never exceed a user's grants (§6.13). Roles bundle page access
  (`energy-operator`, `facilities-admin`, read-only `auditor`). An HVAC schedule edit is a gated tool
  call that syncs setpoints down to the appliance's local control loop.
- **Multi-language + preferences.** The prefs crate (`user-prefs-scope.md`) stores data
  **canonically** (UTC, SI units, locale-neutral codes) and localizes on the way out — language,
  timezone, date/number style, and units as **independent** axes. A tech in Madrid and an analyst in
  Chicago read the same point in their own locale; an alert email renders in the **recipient's** language.

### An external warehouse (e.g. TimescaleDB) — without breaking "one datastore"

A customer may already run **TimescaleDB** + BI/Grafana and want long-horizon SQL analytics off the
hot path. This does **not** violate §3 rule 2 (one datastore: SurrealDB) — *as long as the external
DB is a downstream sink, not a node's persistence layer*:

- **SurrealDB stays the source of truth** on every node. Nothing persists platform state in Timescale.
- **TimescaleDB is an egress target**, reached exactly like GitHub/email: an **outbox/job target
  adapter** (`timescale-target`) that owns the connection + secret. The **job system** (§6.10) runs
  the **migration** as a durable, **resumable, checkpointed batch** — reading SurrealDB timeseries
  ranges and writing them out, so a restart resumes mid-fleet (the batch-as-a-job rule).
- **Bringing extra data *in*** is the mirror: an extension's job **pulls** from the external source and
  **ingests** it as samples into SurrealDB, so it joins the one datastore rather than being queried live.
- **Rejected:** making Timescale a node store or a sync peer — that forks the datastore principle and
  the authority model. It is a consumer, not a second authority.

---

## 7. Findings (gaps this probe surfaces — each is a scope, not a kernel feature)

- **Node connection unwired.** The binary can't yet select role or dial a hub —
  `node-connection-scope.md` (`LB_ROLE` + Zenoh router/connect + `LB_STORE_PATH` + appliance API
  token). **The gate for this whole example.**
- **Fleet roster unbuilt.** "Admin sees appliances online" needs `fleet-presence-scope.md` (node
  identity + liveliness + `appliances.list`).
- **Per-appliance access** rides `authz-grants-scope.md` (grant-by-tag for "all of region EMEA") +
  `edge-trust-scope.md` (the node token/cert). Both scoped, neither wired into the binary.
- **`sensor-source` is illustrative** — a real native hardware source is a Tier-2 extension to scope
  (placement `local-only`, platform target per `platform-targets-scope.md`); the GPIO/Modbus drivers
  themselves are product code, not core.
- **`mobile` (Flutter) client** is scoped as a persona but not built; `browser` is the shipped path.
- **Dashboard UI extension slots** — panels as installable UI extensions lean on the frontend
  extension-slot model (`frontend/`); confirm the dashboard's panel API fits it.
- **Fleet scale (§6)** — regional-hub topology + roster at ~1000 nodes is a scaling question for
  `node-connection-scope.md` (peer-mesh limits, sharding rosters per workspace/region).
- **Grant-by-tag** ("all appliances tagged `region:emea`") needs the tag-based grant form in
  `authz-grants-scope.md` — currently per-id; required at fleet scale.
- **External-DB egress** — a `timescale-target` outbox/job adapter + a resumable **migration job** is
  *new work* (reuses the outbox/jobs pattern); flag it so it isn't mistaken for a second datastore.

---

## 8. Related

- README §5 (personas), §6.1 (store/timeseries), §6.2/§6.5 (bus + presence), §6.8 (sync), §6.12 (UI).
- `vision/0002-coding-agent-workplace.md` (the sibling worked example — same "all extensions" thesis).
- `vision/0004-consumer-iot.md` (the **B2C** sibling — workspace *per household*, global identity, guest invites).
- `scope/ingest/ingest-scope.md` (the `Sample` envelope + "device = principal"), `scope/tags/tags-scope.md`
  (series discovery + grant-by-tag), `scope/store/` (timeseries), `scope/node-roles/node-connection-scope.md`
  (appliance↔hub), `scope/node-roles/fleet-presence-scope.md` (roster),
  `scope/auth-caps/edge-trust-scope.md` (node token/cert), `scope/auth-caps/authz-grants-scope.md`
  (restricted access + grant-by-tag), `scope/inbox-outbox/outbox-scope.md` (the egress-target pattern
  the TimescaleDB adapter follows), `scope/prefs/user-prefs-scope.md` (multi-language + preferences),
  `scope/frontend/` (UI shell + extension slots).
