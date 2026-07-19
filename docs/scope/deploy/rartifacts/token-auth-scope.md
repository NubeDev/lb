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

## Decisions resolved in implementation (2026-07-19)

Slice 2 shipped in `rubix-fleet`. Session doc:
`docs/sessions/deploy/rartifacts-token-auth-session.md`; manual runbook:
`docs/testing/rartifacts-slice2-runbook.md`. These resolve the seams this scope left
open and correct three places where it describes something the platform does not do.
Everything below was found by running against a real booted node.

### Corrections to this scope

- **There are no `apikey.*` MCP tools at `node-v0.4.7` — the surface is REST only.**
  This scope says agent/publisher keys are "minted via the shipped lb api-key verbs".
  `mcp:apikey.manage:call` exists as a CAPABILITY STRING and gates the verbs, but no
  MCP tool by that name is registered; lb serves `/admin/apikeys{,/{id}/revoke,/rotate}`
  on the gateway router, over thin wrappers around public Rust fns (`lb_host::apikey_create`
  and friends). rartifacts therefore mints by calling `apikey_create` DIRECTLY at the
  Rust level, which is also what lets boot mint the anonymous principal with no HTTP
  round-trip against itself.
- **`apikey_create` chooses the key id; a caller cannot supply one.** So "the
  boot-minted anonymous principal" cannot be found again by a well-known id after a
  restart. The implementation adds a small **identity registry** side-table
  (`rartifacts_identity`) mapping this server's ROLE names to lb's generated ids. It
  holds no secrets and grants no authority — a row says "key `x` plays the anonymous
  role", never what `x` may do (that stays lb's resolved caps at lb's chokepoint).
  The same table carries the agent registration metadata, because `ApiKeyRecord` is a
  closed struct of eleven scalars with no metadata map and no extension point.
- **The claim hands out an lb api-key, NOT the `fleet-auth` token.** The scope reads
  as though the `fleet-auth` plaintext IS the admin credential. Implementing it that
  way would give the server two authentication systems: `AuthState` (which knows
  nothing of capabilities, workspaces, or lb's chokepoint) beside lb's own verifier,
  and every package route would need to accept both. Instead `fleet-auth` does the one
  thing it is uniquely good at — proving possession of the box before any identity
  exists — and its reward is a real api-key. The admin key is minted lazily AT CLAIM
  TIME, not at boot: a key minted at boot and held in RAM would be a live
  fully-privileged credential on every server that is never claimed.

### Seams the scope left open

- **Failure ordering in the claim is deliberate and lossy in the safe direction.**
  `try_claim` is consumed BEFORE the key is minted, so a mint failure leaves the
  server claimed-but-credential-less (a `500` naming `--reset-token` as the recovery).
  The other order is worse: a failed claim after a successful mint leaves a live admin
  key nobody holds and nobody knows exists. A lost claim is recoverable at the box; an
  orphaned admin credential is not detectable at all.
- **`LB_APIKEY_PEPPER` instability is a deployment trap and is now loud.** lb falls
  back to a PER-PROCESS RANDOM pepper when it is unset, and since only
  `HMAC(pepper, secret)` is stored, every api-key minted before a restart then fails
  as an opaque `401` — indistinguishable from revocation, across a whole fleet, after
  a routine restart. `RARTIFACTS_APIKEY_PEPPER` mirrors into it and its absence is
  warned at boot, mirroring slice 1's `RARTIFACTS_SIGNING_KEY` posture. `BootConfig`
  has no pepper field, so the env var is the only production seam.
- **The boot minter must hold a superset of every bundle it issues**, because
  `apikey_create` refuses to mint caps the CREATOR lacks (the privilege-escalation
  guard). It is constructed per call, never persisted, never reachable from a request
  path. It uses `Principal::for_key`, whose doc says its inputs are trusted because
  the gateway verified a secret; here they are trusted because they are compile-time
  constants. Stronger provenance, different provenance — recorded rather than glossed.
- **Revocation adds no route.** lb's `POST /admin/apikeys/{id}/revoke` is already on
  the same router, is instant, and busts the verification cache. `key_id` appears in
  every mint response and roster row precisely so an operator can use it. A second
  revoke path would be a second thing that must be correct.

### The bug this slice nearly shipped (worth carrying forward)

**Making the anonymous tier a real api-key silently broke the anonymous visibility
rule, and slice 1's unit tests stayed green while private packages were
world-readable.**

Slice 1 identified "anonymous" as *no `Caller` in the tool frame*. Correct then — the
anonymous tier was a host-side construction that never authenticated. But a real
api-key DOES authenticate, so lb stamps a `Caller` for it exactly as for an agent key,
and the policy then classified every token-less request as authenticated. The unit
tests passed throughout because they pass `None` and never exercise the path a booted
server takes; an integration test that seeds a private package and reads the index
over real HTTP is what caught it.

The fix keys the policy on the caller's IDENTITY (a stable marker subject the host
stamps, shared host↔extension and asserted equal by a test) rather than on whether the
platform happened to identify them. Note the alternative that would have failed OPEN:
"anonymous = holds only the two read caps" — the agent bundle is identical in this
slice (the tiers differ in what they SEE, not which verbs they may call), so a
cap-shaped test would have classified every agent as anonymous.

**Generalizable lesson for slices 3–5: a policy keyed on the ABSENCE of
platform-supplied identity is fragile, because making a tier more real makes its frame
more populated.**

### Behaviour deliberately changed from slice 1

**A presented-but-invalid credential now returns `401` instead of falling back to the
anonymous tier.** Slice 1 documented that fallback as "not a security difference",
which was true when anonymous and authenticated tiers reached the same rows. Revocation
made it false: a revoked agent key falling back to anonymous keeps receiving `200`s, so
revocation is invisible to the operator who performed it and to the agent subject to it
— while this scope's revoke story is "the machine's next poll 401s, surfaced as
`access revoked` in rubixd status". The rule is now absent-credential → anonymous,
present-but-bad → `401`. The public tier is unaffected (reached by sending nothing).

### Claims deliberately NOT made in the code's docs

- **The anonymous fallback weakens revocation during a store outage.** If the registry
  row is unreadable, the server falls back to the compiled-in anonymous bundle, so
  revoking the anonymous key does not take effect until the store recovers. Deliberate:
  the alternative is a transient store error closing public downloads fleet-wide. Every
  OTHER key still fails closed.
- **`last_seen` is never stamped.** The field exists and registration writes `0`; no
  call path updates it, because lb's auth path deliberately writes nothing back to a
  key record per request (the hot path is a read plus an HMAC). The roster tells the
  truth today, not the whole truth — the write-behind batch this scope anticipates is
  still owed.
- **Agent metadata is self-reported and unverified.** Acceptable only because none of
  it feeds an authorization decision. If slice 4 resolves artifacts BY the reported
  arch, that turns a display field into a security input and needs verification first.
- **An agent key reads ALL private packages in the workspace** — this scope's stated
  v1 position. Per-package narrowing needs lb's entity-scoped grants.
- **The 6-digit claim code is compared with `==`, not constant-time.** Inherited from
  `fleet-auth`, whose crate doc says "compare is constant-time via `subtle`" — true of
  the TOKEN, not of the CODE. The 3-strike lockout is the actual defence. Fixing it is
  a `fleet-auth` change that would touch rubixd, so it is recorded here rather than
  done in this slice.
