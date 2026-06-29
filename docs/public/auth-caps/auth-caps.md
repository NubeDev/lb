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
