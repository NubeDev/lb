# Auth-caps scope — edge trust: node enrollment + cert issuance, mTLS, and token-on-the-bus

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped. A follow-up to the S0
`auth-caps-scope.md` decision doc, closing the parts it deferred ("key rotation/recovery flows —
scoped later") and the cross-node enforcement gap STATUS flags: *"token-on-the-bus so the hub can
verify a routed caller's grant (S5/S6 are in-process co-trust)"*. Natural companion to the S8 data
plane (a fleet of edge producers needs exactly this to be trusted).

We want a **cryptographically enforced edge↔cloud trust spine**, in three layers: (1) **mTLS on the
Zenoh link** so the transport is encrypted and mutually authenticated; (2) **node identity via
enrollment** — an edge node *signs up* with a bootstrap credential and is **issued a certificate**
binding it to a workspace; (3) **token-on-the-bus** — a routed MCP call **carries the caller's signed
grant token, and the receiving node re-verifies it** (signature + expiry + caps) instead of co-trusting
the calling node. Today only the token *primitives* exist (Ed25519 mint/verify, `lb_auth`); there is **no
node identity, no mTLS, and the hub co-trusts routed callers** — so this is the missing security spine
for everything edge.

## Goals

- **Node enrollment + cert issuance (the "signup").** An edge node presents a **bootstrap credential**
  (an admin-issued one-time enrollment token, or a provisioning key) + a **CSR** (it generates a keypair,
  keeps the private key) to a cloud **enrollment/CA service**, which verifies and **issues a node
  certificate** signed by the workspace/hub CA, binding `node-id + workspace + validity`.
- **mTLS on the Zenoh link.** Edge ↔ router sessions use the issued cert for **mutual TLS** — the link is
  encrypted and the router authenticates the edge by its cert. Config, not code (symmetric nodes).
- **Token-on-the-bus.** Every routed `lb_mcp::call` carries the caller's **signed capability token**; the
  **receiving (hub) node `lb_auth::verify`s it** (Ed25519 sig + unexpired + caps) before `caps::check`.
  This replaces in-process co-trust with cryptographic per-call authz across the wire.
- **Rotation + revocation** — short-lived certs (or a CRL/short-TTL strategy) so a compromised or
  decommissioned node loses access; key rotation without re-provisioning from scratch.
- **Offline-friendly verification.** A node verifies a peer's token offline with the issuer's **public
  key** (`auth-caps-scope.md` already states edge nodes verify offline) — enrollment needs connectivity,
  steady-state verification does not.

## Non-goals

- **No human login / OIDC / SSO.** That's the session work in `frontend/collaboration-scope.md` (S9) and
  a later IdP scope. This is **node-to-node** identity, not user credentials.
- **No full enterprise PKI / HSM.** A workspace/hub CA signing node certs — not a multi-tier X.509
  hierarchy with hardware key custody (note HSM as a future hardening for the CA key).
- **No cross-hub federation** of a node's workspaces (README §13 open question; assume one hub).
- **No new capability grammar.** The token shape + grammar are fixed by `auth-caps-scope.md`; this carries
  and verifies that token across the wire and adds node identity *beneath* it.
- **No second datastore / transport.** CA records + enrollment tokens live in SurrealDB; the link is the
  existing Zenoh (now with TLS). No new broker, no external CA service.

## Intent / approach

**Three layers, defense in depth.** The *token* half reuses primitives already in the repo —
`lb_auth` mints/verifies Ed25519 tokens, and the registry's Ed25519 sign/verify (the `VerifiedArtifact`
machinery, S7) is the same signature primitive. But the *cert* half is **genuinely new machinery**, not
just assembly — see the decision below.

**DECISION (forced by Layer 1): node certs are X.509 containers with Ed25519 keys inside.** Zenoh's TLS
is rustls-based, and rustls mutual auth requires **X.509 DER certificates** — you cannot present a custom
signed `{node_id, ws, pubkey, exp}` blob to a TLS handshake. So the CA issues a **minimal X.509 profile**
(generated with `rcgen` or similar) whose **key is Ed25519** (rustls supports Ed25519 signature algs),
carrying `node-id + ws + exp` in the subject/SAN/extensions. This means the CA crate gains an **X.509
generation dependency distinct from the registry's blob signing** — the "reuse existing primitives" story
holds for the *token* layer, **not** for cert issuance. (A bare Ed25519 blob was considered and rejected:
it cannot drive rustls mTLS, which is the whole point of Layer 1.)

**Layer 1 — mTLS on the Zenoh link.** Zenoh's rustls TLS config does mutual auth with the X.509 node cert
above: the edge presents it; the router validates it against the workspace CA; the session is encrypted
and peer-authenticated. Selected by **deployment config** (endpoints + cert paths), never an `if cloud {…}`.

**Layer 2 — enrollment / cert issuance.** The flow:
1. Edge boots with a **bootstrap credential** — an admin-issued one-time **enrollment token** (the
   trust-on-first-use root) or a provisioning key.
2. Edge **generates a keypair** (private key stays on the edge, in its own SurrealDB / OS keystore), builds
   a **CSR**.
3. Edge calls the **enrollment endpoint** (a gateway REST route *or* a Zenoh `enroll` queryable) with the
   bootstrap cred + CSR.
4. The **CA service** (a hub role) verifies the bootstrap, **issues a node certificate** — a minimal
   **X.509 cert with an Ed25519 key**, signed by the workspace/hub CA, binding `node-id + ws + exp` — and
   records it.
5. Edge stores the cert and uses it for mTLS thereafter; rotation re-runs steps 2–4 with the *existing*
   cert as the credential.

**Credential delivery is a transport, not a new layer — and a scannable one is embedder-reusable.** The
bootstrap credential in step 1 has to physically reach the box, and typing a one-time token into a
headless CM4 in a switchboard is exactly the error-prone field moment an embedder wants to remove. So the
enrollment token (its `node-id` + secret + hub endpoint, an opaque signed blob) is **renderable as a QR /
scannable code**, and `enroll(bootstrap, csr)` accepts a scanned payload identically to a typed one — the
CA verifies the *same* one-time token either way. This is a **generic lb affordance, owned here, consumed
by every embedder**: ems scans it to claim a gateway into a site, rubix-ai / cc-app to claim any
appliance. Two honest bounds keep it from over-reaching: (a) **scanning is delivery, never trust** — a QR
token is precisely as strong as the one-time-short-lived-single-workspace token inside it (see the
bootstrap-trust risk); it removes typing, not the provisioning-process problem. (b) **the payload grammar
lives in lb, not in any embedder** — an embedder consumes a resolved credential/`node-id`, it does not
mint or parse the enrollment blob. Manual entry stays a first-class path for a cracked lens or a
no-camera host.

**Layer 3 — token-on-the-bus.** Today `register_remote_extension` routes a call over the bus and the hub
**co-trusts** the caller. Change: the routed envelope **carries the caller's signed token**; the
receiving node `lb_auth::verify`s it (sig + `now < exp`) and derives the `Principal`, then runs the
**existing** `caps::check` (workspace-first, then capability) on *that verified principal*. No widening:
the hub trusts the *token's* caps, cryptographically, not the calling node's word.

**The split:** **cert = "which node you are" (Layer 1–2); token = "what this principal may call" (Layer 3);
workspace namespace = the wall (unchanged).** A cert binds a node to a workspace; the token's `ws` claim
must match; `caps::check`'s Gate 1 still refuses any cross-workspace key.

**Rejected alternatives:**
- *Keep in-process co-trust.* Rejected — it means a compromised or impersonating edge is trusted by the
  hub; unacceptable once real edge nodes exist. The whole point of this scope.
- *App-layer tokens only, no mTLS.* Rejected — without transport auth, anyone can open a Zenoh session to
  the router and probe; mTLS authenticates the *node* before any app traffic.
- *A full multi-tier X.509 PKI / HSM hierarchy.* Rejected — heavyweight; a **single workspace CA** issuing
  a **minimal X.509 profile** (Ed25519 keys) is enough. Note this still uses the X.509 *container* (forced
  by rustls, decision above) — what's rejected is the enterprise PKI tree, not the format.
- *A bare Ed25519 cert blob.* Rejected — cannot drive rustls mTLS (Layer 1); the X.509 container is required.
- *Long-lived certs, no revocation.* Rejected — a lost Pi must be revocable; short-TTL + rotation.

## How it fits the core

- **Tenancy / isolation:** an **edge** cert binds a node to **one workspace**; the token's `ws` claim is
  verified; `caps::check` Gate 1 still refuses any other workspace's keys. A node enrolled to ws-B cannot
  obtain or present a ws-A-scoped token, and Zenoh key scoping already prevents it naming ws-A keys.
  **Asymmetry — a hub/router serves many workspaces, so it carries its own *non-ws-bound* node identity**
  (a hub cert signed by the same CA, identifying the hub, not a tenant); the per-call **token's** `ws`
  claim — not the hub cert — is what scopes each routed request. Edge cert = ws-bound; hub cert = identity-
  only.
- **Capabilities:** token-on-the-bus makes **Gate 2 cryptographic across nodes** — the same `caps::check`
  chokepoint, now fed by a *verified* principal rather than a co-trusted one. Enrollment + CA verbs are
  themselves capability-gated (an admin act).
- **Placement:** the **CA / enrollment service is a hub role** (`either`, but normally cloud); mTLS config
  is per-deployment. No core code branches on role — the CA role mounts the endpoint, like the gateway.
- **MCP surface:** `enroll(bootstrap, csr) -> cert`, `rotate(cert, csr) -> cert`, `revoke(node_id)`,
  `ca_pubkey()`. Issued + revoked certs are records; verification is offline against the CA public key.
- **Data (SurrealDB):** `enrollment_token` (one-time, consumed), `node_cert` (issued certs + status),
  `ca_key` reference (the signing key is a **secret**, below). All workspace-scoped state.
- **Bus (Zenoh):** the link itself (now mTLS); the routed-call envelope gains a **token field** (motion
  carrying a signed credential — verified, not trusted). No durable state on the bus.
- **Sync / authority:** the CA is hub-authoritative; issued certs sync read-only to edges so they can
  verify peers offline. Steady-state token verification needs only the cached CA public key.
- **Secrets:** the **CA signing key** and each node's **private key** are secrets (`scope/secrets/`),
  **never** in the UI or an extension. The CA key is the highest-value secret in the system — flag HSM/at-
  rest protection.

## Example flow

1. An admin issues a **one-time enrollment token** for a new Pi in workspace `acme` (out of band).
2. The Pi boots, generates a keypair, builds a CSR, and calls `enroll(token, csr)` on the cloud CA.
3. The CA verifies the token (unused, unexpired), **issues a node cert** (`node:pi-7 / ws:acme / exp`),
   records it, and burns the enrollment token.
4. The Pi opens its **Zenoh session to the router with mTLS** using the cert; the router authenticates it.
5. The Pi makes a routed `ingest.write` call **carrying its principal's signed token**; the **hub
   verifies the token** (sig + exp) and runs `caps::check` on the verified principal — write allowed only
   if the *token's* caps + workspace permit it. A **forged or expired token is refused on the wire**.
6. The Pi is decommissioned → admin `revoke`s its cert → its next mTLS handshake (or short-TTL expiry)
   fails; it can no longer reach the hub.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — a routed call with a **forged or expired token is rejected on the receiving
  node** (the token-on-the-bus verify); an **unenrolled / revoked node is rejected at the mTLS handshake**;
  the `enroll`/`revoke` verbs are refused without the admin cap.
- **Workspace isolation** — a node enrolled to ws-B cannot present a ws-A-scoped token (verify fails on
  the `ws` claim) and cannot name ws-A keys; across the bus + store.
- **Offline / sync** — a node verifies a peer's token **offline** with the cached CA public key (no call
  to the CA per verification); enrollment correctly *requires* connectivity and fails closed offline.

Plus this slice's cases:

- **Enrollment** — a valid bootstrap + CSR yields a verifiable cert; a **reused** enrollment token is
  refused (one-time); a tampered CSR is refused.
- **Rotation** — an existing valid cert can obtain a new one; the old one is retired.
- **Revocation** — a revoked cert fails the handshake / next short-TTL renewal; a token minted before
  revocation is bounded by its `exp` (state the window).
- **Clock skew** — `exp` checks use the injected clock (never wall-clock), consistent with `auth-caps`.

## Risks & hard problems

- **Bootstrap trust (the root problem).** Enrollment is only as strong as how the bootstrap credential is
  delivered — a leaked enrollment token enrolls a rogue node. Mitigate: one-time, short-lived, admin-
  issued, single-workspace tokens; optionally a provisioning secret for manufactured fleets. This is the
  #1 risk and can't be fully solved in software (it's a provisioning-process concern). **A scannable QR
  token does not change this calculus** — it changes *how the same token is typed*, not what the token is;
  a QR sticker photographed off a discarded box is the same leaked-token threat, so the QR carries the
  same one-time / short-TTL / single-workspace constraints and is treated as secret in transit and at rest.
- **CA key protection.** The CA signing key compromises *everything* if leaked. Treat as the top secret;
  flag HSM / at-rest encryption / restricted-role access. Out of scope to *implement* HSM, in scope to
  *name* the requirement.
- **Offline token verification structurally cannot honor immediate revocation.** This is a **constraint,
  not a tunable.** For the **cert/mTLS layer** the router is online and checks revocation *at handshake*
  (example step 6 works). But for **token-on-the-bus between two edge nodes that verify offline**, there
  is no CA to consult — revocation is bounded by `exp`, full stop. So on the offline path **short TTL is
  the only option**, not a "lean"; an immediate-revocation requirement forces those callers online. State
  this plainly to callers: offline verification trades immediacy for availability.
- **The co-trust→wire-verify migration is broad.** Token-on-the-bus touches the routed-MCP path
  (`remote.rs`, `serve`, agent/route) everywhere a call crosses a node. Sequence it carefully; keep the
  in-process (same-node) path unchanged (no token needed when there's no wire).
- **mTLS + Zenoh operational friction.** Cert distribution, endpoint config, NAT traversal — deployment
  complexity that's easy to underestimate. The edge dials out (NAT-friendly), but cert plumbing is real.

## Open questions

- **Enrollment transport:** a gateway **REST** route (a node has HTTP even without a browser) vs a Zenoh
  **`enroll` queryable**. Lean: REST for enrollment (simple, pre-session), Zenoh for everything after.
- **Scannable-credential payload + who prints it:** the QR encodes the one-time enrollment token +
  `node-id` + hub endpoint — but is it minted-and-printed **at manufacture** (a provisioning-line sticker,
  fleet-friendly, but the token must then be long-lived-until-first-use) or **at admin-issue time**
  (short-TTL, printed/displayed when the admin creates the enrollment, scanned during the same visit)?
  Lean: admin-issue-time for short-TTL safety; note manufacture-time as the fleet-scale variant. Either
  way the payload grammar is lb's, so an embedder's scan UI (e.g. ems gateway commissioning) is a thin
  consumer that never parses the blob.
- **TTL length + revocation immediacy on the *online* path:** the offline path is settled (short TTL,
  revocation-by-expiry — see Risks). For online callers, how short a TTL, and CRL vs OCSP-style vs short-
  TTL-only for the router's handshake revocation check? Lean: short-TTL certs + an explicit `revoke` the
  router enforces at handshake.

Resolved in this doc (no longer open): **cert format is X.509-container-with-Ed25519-keys** (forced by
rustls mTLS — see the DECISION in Intent); the CA gains an X.509-generation dependency distinct from the
registry blob signing; offline token verification honors revocation only via `exp` (a constraint, not a
choice).
- **Where the CA lives:** a dedicated `ca` hub role/crate vs folded into an existing hub role. Lean: its
  own role (`lb-role-ca`), mirroring `lb-role-registry-host`.
- **Does the token's `sub` (principal) need to be bound to the node cert,** or are node identity and user
  identity independent (a node carries tokens for multiple users)? Lean: independent — the cert
  authenticates the node/link, the token authenticates the principal; a gateway node carries many users'
  tokens.

## Related

- `scope/auth-caps/auth-caps-scope.md` — the token + grammar this carries across the wire and extends with
  node identity (this closes its deferred "key rotation/recovery").
- `scope/auth-caps/authz-grants-scope.md` — the grants this token's caps come *from* (sibling scope); this
  one makes them verifiable cross-node.
- `scope/sync/sync-scope.md` — the routed/sync paths token-on-the-bus hardens (§6.8).
- `scope/registry/registry-scope.md` — the Ed25519 sign/verify primitive reused for the token layer; note
  the CA's **X.509 cert generation is new machinery**, not the registry's blob signing (see the Intent
  decision).
- `scope/secrets/` — the CA signing key + node private keys.
- `scope/node-roles/` — the CA / enrollment role (a stub today).
- `scope/node-roles/node-connection-scope.md` — **consumes** this scope's node credential as the
  **appliance API token** and wires it into the edge→hub connect (config + bus + sync).
- ems `docs/scope/field-install/scan-to-add-meter-scope.md` + `docs/scope/gateways/gateways-scope.md`
  (downstream embedder) — the **first consumer** of the scannable enrollment credential: a technician
  scans a gateway's QR to claim it into a site (`gateway.add`'s `node_id`), reusing this affordance rather
  than inventing an ems-side one. ems owns no QR grammar; it consumes a resolved `node-id`.
- README **§6.2** (Zenoh — the link mTLS rides), **§6.6** (identity/auth; offline verify), **§6.8** (sync),
  **§7** (tenancy / the workspace wall), **§13** (the token forever-decision).
