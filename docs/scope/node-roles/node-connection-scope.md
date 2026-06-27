# Node-roles scope — appliance ↔ hub connection (config, API tokens, access)

Status: scope (the ask). Promotes to `public/node-roles/` once shipped.

An **appliance** (a headless edge node — see README §5 personas) must **connect up to a hub**:
open its Zenoh peer to the hub's router, authenticate as a known **node identity with an API
token**, hold a read-cache and sync its writes up (§6.8), and appear in the fleet roster. And
that appliance is itself an **access-controlled resource**: an admin can reach any appliance in
the workspace, while a given user can be granted **restricted access** to only specific ones.
Today none of this is wired in the binary — `node` boots a solo demo, `Bus::peer()` opens a bare
default peer with no upstream, `Node::boot()` ignores the store path, and there is no node API
token in use. This scope is the **connection slice**: the config + identity + token + access
wiring that makes "appliance connects to hub" real. It **composes** the existing auth scopes
rather than re-inventing them.

> Read with: `README.md` §5 (roles + personas), §6.5 (Zenoh peer→router), §6.6 (auth/JWT),
> §6.8 (sync authority). **Builds on:** `auth-caps/edge-trust-scope.md` (node enrollment + cert
> + token-on-the-bus — the appliance API token *is* this), `auth-caps/authz-grants-scope.md`
> (the per-workspace grant model — "restricted access to an appliance" *is* a grant),
> `node-roles/fleet-presence-scope.md` (the roster the connected appliance shows up in),
> `node-roles/node-roles-scope.md` (roles are config), `sync/sync-scope.md` (edge→hub authority).

## Goals

- **Config-select the role + topology.** `node` reads `LB_ROLE` (→ `lb_host::Role`), and the bus
  honors a **mode + upstream**: a hub runs the Zenoh **router** (`LB_ZENOH_MODE=router`), an
  appliance runs a **peer** that **connects** to it (`LB_ZENOH_CONNECT=tcp/hub:7447`). Config,
  never a code branch (§3 rule 1).
- **Persistent store per node.** `Node::boot()` honors `LB_STORE_PATH` so each appliance has its
  own durable volume (today it's ignored).
- **Appliance API token (the node credential).** Each appliance authenticates to the hub with a
  **long-lived, workspace-bound node token** — the credential form of the `edge-trust` node
  identity. Issued/revoked by an admin; presented on connect; re-verified by the hub
  (token-on-the-bus). A headless appliance can't do an interactive login, so this is its login.
- **Appliance as an authorized resource.** A connected appliance is a **node resource** that
  grants target: an **admin** role reaches any appliance in the workspace; a **user/team** can be
  granted access to **specific appliances only**. This is an ordinary grant in
  `authz-grants-scope`, not a new mechanism.
- **It shows up.** A connected appliance announces node presence (fleet-presence scope) so an
  admin sees it online with its persona/version.

## Non-goals

- **The enrollment/CA + mTLS machinery itself** — owned by `edge-trust-scope.md` (CSR, cert
  issuance, mTLS on the Zenoh link, rotation/revocation). This scope **consumes** the resulting
  node credential and wires it into connect; it does not redefine PKI.
- **The grant store / roles / teams machinery itself** — owned by `authz-grants-scope.md`. This
  scope **names the appliance resource** those grants target; it doesn't rebuild RBAC.
- **Human login / OIDC** — that's the session work (`frontend/collaboration-scope.md`). Appliance
  auth is **node-to-node**, not a user credential.
- **Remote appliance control** (restart/drain/evict) — a later admin-action slice; "access" here
  means *reach its tools/data subject to grants*, plus read-only roster visibility.
- **A second datastore for node/credential records** — appliance identity + grants live in
  SurrealDB on the hub like everything else (§3 rule 2).

## Intent / approach

**Connection = role config + an authenticated Zenoh join.** Two links, both already designed,
neither yet wired in the binary:

1. **Bus (motion).** `Bus::peer()` gains config: a hub opens a router-mode session and listens; an
   appliance opens a peer and `connect`s to the hub endpoint. The Zenoh **mode + endpoints** are a
   `zenoh::Config` built from env — exactly the "endpoint config is a deployment concern" the bus
   scope already anticipates. No core crate branches on role to do this; the `node` binary (a thin
   wiring layer, §3.1) builds the config from `LB_ROLE`/`LB_ZENOH_*`.
2. **Trust (identity).** The link is **mTLS** with the appliance's issued cert, and every routed
   `lb_mcp::call` carries the appliance's **signed token** which the hub re-verifies — both straight
   from `edge-trust-scope`. The **appliance API token** is that node credential in a long-lived,
   admin-managed form (a headless node has no interactive session to mint a short token each time).

**The appliance API token.** A node token is an `lb_auth` JWT with `sub = node:<id>`, `ws =
<workspace>`, a node-appropriate `role`, and `caps` for what the appliance may do (publish its
series, call the tools it needs). It differs from a user token only in **subject kind** and
**lifetime** (long-lived + admin-revocable vs. session-derived). Admin verbs:
`appliance.token.issue` / `appliance.token.revoke` (names TBD with auth-caps), each an
admin-capability-gated MCP tool. Revocation rides the `edge-trust` short-TTL/CRL strategy.

**Restricted access to an appliance.** Treat each appliance as a **resource key**, e.g.
`node:<id>` (or `appliance:<id>`), and reuse the grant model: a grant `subject → caps` where the
cap names the appliance — `mcp:appliance.<id>.*:access` (or the resource-grant form
`authz-grants` settles on). Then:
- **admin** role bundles "all appliances in the workspace" (a wildcard/role-level grant);
- a **user** gets a grant for `appliance:42` only → Gate 2 (`caps::check`) permits that appliance
  and denies the rest; the workspace wall (Gate 1) already keeps other tenants' appliances
  invisible.
No new enforcement — the **three gates** (workspace → capability → membership) already do this;
this scope only **names the resource** and the admin verbs to manage the grants.

**Why compose, not invent.** Each requirement maps onto a scope that already exists: connect =
config the bus already anticipates; token = the `edge-trust` node credential; restricted access =
an `authz-grants` resource grant. Inventing a parallel "appliance auth" or "device ACL" would
duplicate both and drift from the three-gate model. **Rejected.** The only genuinely new surface
is (a) the binary/config wiring and (b) naming the appliance as a grantable resource + its admin
token verbs.

## How it fits the core

- **Tenancy / isolation:** an appliance token is **workspace-bound** (`ws` claim); its cert binds
  it to one workspace (`edge-trust`). A ws-B admin can neither see nor grant a ws-A appliance —
  Gate 1 first, every time.
- **Capabilities:** the appliance acts only within its token's caps; **access to** an appliance is
  a cap a user/team must be granted. Deny path: a user without the `appliance:42` grant calling its
  tools is refused at `caps::check`; an un-revoked-but-expired node token fails `lb_auth::verify`
  at the hub before any dispatch.
- **Symmetric nodes:** hub vs appliance is `LB_ROLE` + which Zenoh mode the **config** selects —
  the router-vs-peer choice is `zenoh::Config`, not an `if cloud {…}`. The same `Bus`/`Node` code
  runs both.
- **One datastore:** node identities, appliance tokens (their public/metadata records), and grants
  are SurrealDB records on the hub. No device registry, no separate auth store.
- **State vs motion:** the **connection** is motion (Zenoh); the **authorization** (who may reach
  the appliance, the issued-token metadata) is state (SurrealDB). Online/offline stays liveliness
  (fleet-presence), not a stored flag.
- **MCP is the contract:** `appliance.token.issue/revoke` and any appliance-access grant management
  are MCP tools, admin-gated — same contract as everything else.
- **Sync / authority:** the appliance is `Role::Edge` → holds a read-cache, queues writes through
  the **outbox** to the hub (authoritative). Offline: it keeps running locally and reconnects with
  idempotent apply (§6.8) — the token is verified **offline** against the issuer public key, so a
  brief hub outage doesn't lock the appliance out of local work.
- **Secrets:** the appliance's **private key** never leaves it (generated locally, CSR only sends
  the public part — `edge-trust`); the API token is bearer material handled like any secret.
- **API shape (§6.1 of SCOPE-WRITTING):** `appliance.token.issue` / `appliance.token.revoke`
  (writes); `appliances.list` + `appliance.get` (reads — the roster + one appliance's detail,
  reusing fleet-presence); access management = ordinary grant verbs. A **bulk enroll / bulk
  re-issue** of many appliances, if needed, is a **job** (long-running), not a blocking call.

## Example flow

1. Admin runs `appliance.token.issue` for workspace `acme` → the hub mints a long-lived node token
   `{sub: node:42, ws: acme, role: edge, caps:[…]}` and records the appliance. (Cert enrollment per
   `edge-trust`; the token is its bus-side credential.)
2. The appliance is configured: `LB_ROLE=edge`, `LB_ZENOH_CONNECT=tcp/hub:7447`,
   `LB_STORE_PATH=/data`, and its token/cert. It boots.
3. `Bus::peer()` builds a peer `zenoh::Config` with the hub endpoint + mTLS cert → connects to the
   hub's router. The hub authenticates the cert; the appliance announces node presence.
4. The appliance publishes its series; each routed `lb_mcp::call` carries its token → the hub
   `lb_auth::verify`s it → `caps::check` → accepted. It appears **online** in the admin roster.
5. A **user** with an `appliance:42` grant opens its dashboard → Gate 1 (ws acme) ✓ → Gate 2
   (`appliance:42` cap) ✓ → sees appliance 42's data. The same user hitting `appliance:99` →
   Gate 2 **denied**.
6. Admin runs `appliance.token.revoke node:42` → the token/cert is revoked (short-TTL/CRL) → on its
   next reconnect the appliance is refused; the roster shows it offline.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny-test (mandatory):** (a) a user **without** the `appliance:<id>` grant is denied
  its tools/data; **with** the grant, allowed. (b) A **revoked/expired** appliance token is refused
  at the hub (`lb_auth::verify` fails) before dispatch.
- **Workspace-isolation (mandatory):** a ws-B admin cannot see, grant, or reach a ws-A appliance;
  an appliance token minted for ws-A never authorizes in ws-B.
- **Offline / sync:** an appliance disconnected from the hub keeps doing local work, verifies its
  own token offline, and on reconnect syncs queued writes with idempotent apply; the roster flips
  online.
- **Multi-node E2E (the headline):** extend the `boot_as(role)` test — boot a **hub** (router) + an
  **appliance** (peer, `LB_ZENOH_CONNECT`) over a real transport; assert the appliance authenticates,
  connects, syncs a series up, and shows in `appliances.list`. Then revoke and assert refusal.
- **Unit:** env → `zenoh::Config` (router vs peer+connect) mapping; node-token claim shape
  (`sub=node:*`, ws-bound, long-lived); appliance resource-key ↔ grant mapping.

## Risks & hard problems

- **Token lifetime vs. revocation.** Long-lived appliance tokens are convenient but dangerous if
  leaked; lean on `edge-trust`'s short-TTL-cert + CRL so revocation is real, not just "the JWT
  expires in a year." Decide the appliance token TTL + refresh story explicitly.
- **Bootstrap/provisioning.** Getting the *first* credential onto a headless Pi (enrollment token /
  provisioning key) is the classic chicken-and-egg — owned by `edge-trust`, but this scope must not
  assume a credential magically present.
- **Endpoint config & NAT.** `LB_ZENOH_CONNECT` assumes the appliance can reach the hub; real edge
  deploys hit NAT/firewalls. Zenoh handles much of this, but document the supported topologies.
- **Grant granularity.** Per-appliance grants can explode (N appliances × M users). Lean on
  **roles/teams** (`authz-grants`) and wildcard/group grants ("all appliances", "appliances tagged
  `floor-2`" via the tags service) so admins don't hand-grant each one.
- **Offline authz staleness.** An appliance verifies tokens offline against a cached public key; a
  grant revoked while it's offline isn't seen until reconnect — acceptable for the read-cache model,
  but state the window.

## Open questions

- **Resource key spelling** — `node:<id>` vs `appliance:<id>` for the grant target, and whether
  appliance-access caps are `mcp:appliance.<id>:access` or a dedicated resource-grant form. Settle
  with `authz-grants`.
- **Appliance token vs. user token unification** — is the node token literally an `lb_auth` JWT with
  `sub=node:*` (recommended — one verify path), or a distinct credential type? Confirm with
  `edge-trust`.
- **Grant-by-tag** — "grant access to all appliances tagged `site-A`" using the tags service vs.
  explicit per-id grants. Powerful; defer unless asked.
- **Where the bus mode/endpoint config lives long-term** — env vars now; folds into the node config
  story (the `LB_STORE_PATH` scope already flags a "node config story to consolidate later").
- **Multi-workspace appliance** — an appliance serving one workspace (the `edge-trust` default) vs.
  several; default single-workspace, note the multi case.

## Related

- `README.md` §5 (personas), §6.5 (bus topology), §6.6 (auth), §6.8 (sync).
- `auth-caps/edge-trust-scope.md` (node enrollment + cert + token-on-the-bus — the appliance API
  token), `auth-caps/authz-grants-scope.md` (grants/roles/teams — restricted appliance access),
  `node-roles/fleet-presence-scope.md` (the roster), `node-roles/node-roles-scope.md` (roles are
  config), `sync/sync-scope.md` (edge→hub authority).
- Code to wire: `rust/node/src/main.rs` (read `LB_ROLE`/`LB_ZENOH_*`), `crates/bus/src/peer.rs`
  (router vs peer+connect `zenoh::Config`), `crates/host/src/boot.rs` (`LB_STORE_PATH`),
  `crates/auth` (`lb_auth` node-token mint/verify), `crates/host/src/role.rs` (`Role`).
- Prerequisite for: `node-roles/fleet-presence-scope.md` (needs a connected, identified node) and
  the `docker/` e2e fixture (needs an appliance that actually dials a hub).
