# Auth and capabilities (shipped — S1)

The capability model — the actual core product (README §11.1). Scope/decisions:
`../../scope/auth-caps/auth-caps-scope.md`. Session: `../../sessions/core/s0-s1-spine-session.md`.

## Token (the principal)

A token is an **Ed25519-signed JWT** (compact JWS, `alg: EdDSA`), signed/verified with
`ed25519-dalek` directly (one crypto library, no JWT/ring seam). Claims are small:

```jsonc
{ "sub": "user:ada", "ws": "acme", "role": "member",
  "caps": ["mcp:hello.echo:call"], "iat": 0, "exp": 100 }
```

`ws` is mandatory and singular — one token authorizes exactly one workspace. `verify(key,
token, now)` proves the signature and that `now < exp` (the clock is injected, never wall),
then yields a `Principal`. There is no public raw `Principal` constructor: an unverified
principal cannot exist.

## Capability grammar

```
<surface>:<resource>:<action>
```

- **surface** — `mcp | store | bus | secret`.
- **resource** — a path within the surface; segments delimited by `/` or `.`, with `*` (one
  segment) and `**` (recursive trailing run). `mcp` names `<ext>.<tool>`; store/bus/secret use
  `/`. Bus caps are written *without* the `ws/{id}/` prefix (the host adds it).
- **action** — `mcp:…:call`; `store:…:read|write`; `bus:…:pub|sub`; `secret:…:get`; `*` = any.

Deny-by-default: an unparseable capability grants nothing.

## The two-gate check (the one chokepoint)

`caps::check(principal, request)` runs, in order:

1. **Workspace isolation** (hard wall): `principal.ws == request.ws`, else `Denied::Workspace`.
   No capability overrides this.
2. **Capability**: some held cap pattern-matches `(surface, resource, action)`, else
   `Denied::Capability`.

Every surface (store, bus, mcp, secret) routes through this *before* touching the resource —
there is no other path. MCP dispatch authorizes here before resolving the tool, so a denial
never reveals whether the tool exists.

## Tested guarantees (S1)

- Capability-deny: without the grant, the call is refused (`caps/tests/deny_test`,
  `host/tests/spine_test::echo_is_refused_without_the_grant`).
- Workspace-isolation: a principal in workspace B is denied on workspace A across all surfaces,
  even holding a matching cap — gate 1 fires first (`caps/tests/isolation_test`,
  `store/tests/isolation_test`, `host/tests/spine_test::second_workspace_cannot_call_into_the_first`).
- Token round-trip / expiry / wrong-key / tamper (`auth/tests/token_test`).

## Deferred

Grant delegation (S5), OIDC + RBAC hierarchy + key rotation/custody (S3+), store-backed
grant-sets at scale (measure at S2). Negative/deny caps: rejected for v1 (deny-by-default +
explicit grants).

**API keys** — machine principals (appliance/cli/api/agent) as a non-human `Subject::Key("{id}")`
over this same grant model, authenticated by a **peppered bearer secret** and authorized through the
one chokepoint above. **Shipped** (`lb-apikey` + host `apikey` service + gateway bearer-auth + admin
"API Keys" tab). The credential is the bearer — verified per request, never exchanged for a token —
with the grammar `lbk_{ws}.{keyid}.{secret}` (dot-delimited Crockford base32 fields; the `{ws}.{keyid}`
prefix is an O(1) ws-scoped lookup). The secret is `HMAC-SHA256(pepper, secret_field)` (input is the
secret field **alone**, never the full bearer; pepper from env, never the DB), compared constant-time.
A small hash→`Principal` cache (5s TTL) keeps the hot path cheap and is **busted on revoke**, so a
revoked key is refused on the very next request on the revoking node (the multi-node floor is sync +
TTL). `Principal::for_key` builds the verified principal (NOT the co-trust `routed` path — a bearer
key from an untrusted appliance is a different trust context). Read-only vs read-write and tool/page
limits are just *which caps the key resolves to* (two built-in roles, `apikey-read`/`apikey-write`,
seeded idempotently; custom caps are an ordinary grant on `key:{id}`). Expiry is a **lazy check at
authentication** (security never depends on a scheduler); the outbox only tombstones + notifies. The
privilege-escalation guard runs in `apikey.create`: the key's effective resolved caps must be ⊆ the
creator's own (covering the built-in-role path the grant path's `role:` exemption would otherwise
miss). Scope + session: `../../scope/auth-caps/api-keys-scope.md`,
`../../sessions/auth-caps/api-keys-session.md`.

**Global identity + membership (the Slack model)** — one global identity per person belonging to many
workspaces. **Shipped.** Identity lives in a reserved system namespace `_lb_identity` (mirroring the
shipped `_lb_workflow_directory` reserved-namespace pattern): `identity:{sub}` = `{sub, display_name?,
created_ts}`, hub-writable and resolution-read-only, carrying **no tenant data** and **no credential**
(the dev-login sits behind the identity seam; OIDC is an additive later slice). `sub` stays the
human handle (`user:ada`), **globally unique** — keeping it avoids retrofitting every existing
`Subject::User(sub)` grant row; `display_name` is a separate non-unique field.

Each workspace gains a per-workspace `membership` roster: `membership:{sub}` = `{sub, joined_ts}` in
the workspace's own namespace — the single source of truth for "who is in this workspace" (NO
`role_hint`; role is grant-driven). The verbs (each its own capability + file):
- **`identity.create/get/list`** + **`identity.workspaces`** — gated `mcp:identity.manage:call`.
  `identity.workspaces(sub)` is a **hub-only bounded scan**: it reads the node's workspace directory,
  then checks each workspace's `membership` table for `sub` (runs once at login / when the switcher
  opens — NOT a hot path; the per-workspace `membership` table IS the index, no denormalized reverse
  index in v1).
- **`membership.add/remove/list`** — gated `mcp:members.manage:call`. `add` writes the row AND
  grants the built-in `member` role (a SYSTEM effect via the raw `grant_assign`, not the gated
  `grants_assign` — a join is not a caller widening). `remove` tombstones the row AND **composes** the
  shipped `revoke_subject` + `token_revoke_mark` (it does not duplicate them) for a clean exit — the
  member's live token is refused on the next verify. `list` returns the **effective** roster =
  membership rows ∪ legacy `user:*` rows (lazy migration, so an upgraded workspace loses nobody).

**Login** resolves identity → memberships → the EXISTING `(sub, ws, caps)` token: an effective member
mints; a brand-new **empty** workspace bootstraps the requester as the first `workspace-admin`
(decision #3 — the dev-login realization of the first-member bootstrap, preserving the auto-seed
demo); a workspace that already has members but not this sub refuses with "not a member of any
workspace" (decision #4 — a provisioned identity with zero memberships cannot mint). `create_workspace`
auto-memberships the creator AND grants `workspace-admin`. The token/cap grammar is **unchanged**; the
Access console "People" tab re-points from `user_list` → `membership.list` (decision #9, the proving
surface); the workspace switcher resolves through `identity.workspaces`. Migration is **lazy**: a
legacy `user:<name>` row with no `membership` is an implicit membership; `identity:{user:<name>}` is
created idempotently on first resolution — no big-bang upgrade, no access gained or lost (pinned by a
test). Identity + membership WRITES are hub-only; edges verify tokens offline and cache identity.
Scope + session: `../../scope/auth-caps/global-identity-scope.md`,
`../../sessions/auth-caps/global-identity-session.md`.
