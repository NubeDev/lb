# rartifacts scope — claim, api-key principals, agents, anonymous tier

Status: scope (the ask). Slice 2 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

Identity on rartifacts is **lb identity**: the superadmin, every publisher, and every
registered rubixd agent are **api-key machine principals** (the shipped
`auth-caps/api-keys-scope.md` model — hashed bearer, per-request check, instant
revoke) carrying narrow capability bundles. This slice adds the two things lb doesn't
have: the **one-time boot claim** (fleet-auth) that hands the operator the admin
api-key, and the **anonymous tier** that lets `public` packages be downloaded with
no token at all.

## Goals

- **Boot + claim**: first boot mints the **admin api-key** (full `pkg.*` +
  api-key-management grants) and holds it unclaimed via `fleet-auth`: hash persisted,
  plaintext in RAM, journal logs the claim URL **and a 6-digit claim code —
  mandatory** (rartifacts binds `0.0.0.0`). `POST /api/claim {code}` (host-mounted,
  open) returns the api-key **once** → `410` forever; 3 wrong codes → `423` until
  restart; `rartifacts --reset-token` re-mints at the box. Recommendation carried
  from the earlier design: bind `127.0.0.1` until claimed, flip to the configured
  bind after (boot log says so).
- **Publisher principals**: an api-key with `mcp:pkg.publish:call` +
  `mcp:pkg.promote:call` (ownership enforced in-tool), linked to one or more
  registered **Ed25519 pubkeys** (`pkg.publisher.register`, admin-gated) — slice 3
  verifies artifact signatures against the *uploading publisher's* keys.
- **Agent principals** (registered rubixd instances): an api-key with read-only
  grants (`mcp:pkg.resolve:call`, `pkg.list/get`, blob read), plus registration
  metadata — `hostname`, `arch`, self-reported rubixd version, rolling `last_seen`
  (updated per authenticated call) — the live roster. **Revoke = revoke the api-key**
  (shipped lb behavior, instant): the machine's next poll 401s, surfaced as
  `access revoked` in rubixd status; running services unaffected. Unlimited agents
  per server, and one rubixd may hold agent keys for unlimited rartifacts remotes.
- **Anonymous tier**: boot also mints a **read-only `anonymous` principal** whose
  caps reach only the public read path. The host-mounted wire routes run token-less
  requests under it; the `pkg.*` read tools enforce visibility (`public` rows only
  for the anonymous principal; private resolve/download → the same `401` whether the
  name exists or not — no existence leak). Publishing is never anonymous.
- **Roster surface**: `pkg.agent.list` (admin) exposing the registration metadata —
  what the UI's Agents page renders.

## Non-goals

- No human user accounts/OIDC — operators hold api-keys v1 (lb's global-identity
  work is orthogonal and can layer in later). No per-package agent ACLs (an agent
  key reads *all* private packages v1 — open question). No token expiry policies
  (revocation covers v1). TLS is fronting infra; the docs say loudly: never claim or
  mint over untrusted plain HTTP.

## Intent / approach

Reuse over invention: the entire token table, hashing, revocation, and admin surface
come from lb api-keys; `fleet-auth` contributes only the claim state machine (shared
with rubixd, already built); this slice's own code is the cap-bundle presets
(admin/publisher/agent/anonymous), the registration metadata on the agent record,
and the visibility check inside the read tools. Alternative rejected: a parallel
token table in the extension (the pre-lb design) — it would duplicate a shipped,
tested lb surface and give the UI two auth systems.

## How it fits the core

Capability-first, literally: `granted = requested ∩ admin_approved`, the deny paths
are lb's own 401/403 plus fleet-auth's 410/423. The anonymous principal is a **leash,
not a hole**: a mandatory caps test proves it cannot reach a private read, a write,
or anything outside `pkg.*` reads. Workspace wall: all principals live in `fleet`.
Secrets: api-key plaintext shown once (claim/mint), hash at rest — both lb-shipped
properties.

## Example flow

1. Ops boots rartifacts behind Caddy TLS → journal: claim URL + code `481 062`.
2. Admin claims once (slice-5 UI; curl until then) → holds the admin api-key.
3. Mints publisher `ci` (+ registers the CI release pubkey) and agent `site-alpha`;
   drops the agent key into that box's `[[remote]] token_path`.
4. site-alpha resolves private packages; its `last_seen` ticks in the roster. Anyone
   downloads the `public` rubix-ai channel with zero tokens.
5. Box decommissioned → *Revoke* in the UI → next poll 401 (`access revoked` in
   rubixd status); public downloads unaffected.

## Testing plan

Real spawned node (the lb testing rules — no mocks):

- fleet-auth suite against the host wiring: claim-once → 410; wrong-code ladder →
  423; reset-token; restart-before-claim invalidates the old plaintext.
- **route × principal × visibility matrix**, generated from the route/tool table so
  an undeclared policy fails CI: {anonymous, agent, publisher, admin, revoked-agent}
  × {public pkg, private pkg} × {list, get, resolve, blob, publish, promote, mint} →
  expected {200, 401, 403}.
- anonymous honesty: token-less list shows only public; private get/resolve → same
  401 for existing and non-existing names.
- **anonymous-leash caps test**: the anonymous principal calling a write tool or a
  private read → denied at the wall (this is the mandatory capability-deny test).
- revoke: agent key revoked → next call 401; `last_seen` stops; roster shows it
  revoked. Mint returns plaintext once; `pkg.agent.list` never contains secrets.

## Risks & hard problems

- The claim window on a public bind — mandatory code + 423 + localhost-until-claimed
  (recommended) + TLS-fronting docs.
- Anonymous-principal creep: future tools must opt *in* to anonymous reach; the
  generated matrix test is the guard (a new tool defaults to authenticated).
- `last_seen` write amplification on busy fleets — write-behind (batch every N s),
  same trade the earlier design made.

## Open questions

- Per-package (or per-tag) agent read scopes — lb's `entity-scoped-grants-scope.md`
  is the natural mechanism once it ships; v1 stays all-private-or-nothing per agent.
- Should the admin claim also create a human login for the shell (when
  global-identity ships)? Defer; api-key in the shell's token box works today.

## Related

lb `docs/scope/auth-caps/api-keys-scope.md` (the substrate) ·
[`../rubixd/token-auth-scope.md`](../rubixd/token-auth-scope.md) (fleet-auth, shared)
· [`server-core-scope.md`](server-core-scope.md) (the tools the policy lives in) ·
[`web-ui-scope.md`](web-ui-scope.md) (claim + roster pages).
