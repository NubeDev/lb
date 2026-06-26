# Auth and capabilities scope

Status: scope. **S0 decision doc** — fixes the README §13 "capability grammar + token shape"
forever decision, and the §3.5/§6.6 enforcement order. Promotes to `public/auth-caps/` once
the S1 spine proves it end to end.

> Read with: `../../README.md` §6.6 (identity/auth/caps), §7 (tenancy), §3 (principles 5–7),
> `../mcp/mcp-scope.md` (the tool surface caps gate), `../testing/testing-scope.md` §2
> (the mandatory deny + isolation tests this scope must satisfy).

---

## Goal

One identity → one scope set that **projects onto all three enforcement surfaces** (SurrealDB
records, Zenoh keys, MCP tools), checked **workspace-first, then capability**. Make the token
shape and the capability grammar concrete enough to implement the S1 spine and never re-cut
them later.

## Non-goals (v1 / early stages)

- OIDC human login, RBAC role hierarchy beyond the three named roles, key rotation/recovery
  flows — scoped later (S3+). S1 uses a **minted API-key principal** only.
- An `org` tier above workspace (README §7 defers it).
- Cross-hub federation of a user's workspaces (README §13 open question; assume co-located).

---

## DECISION (forever): the principal & token shape

A token is a **JWT**, signed by the issuing node's key (Ed25519). Edge nodes verify offline
with the public key (README §6.6). The claim set is deliberately small:

```jsonc
{
  "sub":  "user:ada",            // global identity (or "key:ci-bot" for an API-key actor)
  "ws":   "acme",                // THE workspace claim — the hard wall (§6.6, §7)
  "role": "member",             // super-admin | workspace-admin | member
  "caps": [                      // capability strings (grammar below)
    "mcp:hello.echo:call",
    "store:note:read",
    "bus:chan/*:sub"
  ],
  "iat": 1, "exp": 2            // injected clock in tests — never wall-clock (testing §3)
}
```

- **`ws` is mandatory and singular.** A token authorizes exactly one workspace. Switching
  workspaces in the UI mints/loads a different token. This makes "workspace-first" trivial to
  enforce: the check is `token.ws == resource.ws` before any capability is consulted.
- **`role` gates the *grant* of capabilities, not the *check*.** A `member`'s token simply
  carries fewer `caps`. The check path only reads `caps`; roles matter at mint time. (Keeps
  the hot path a pure set/grammar match — no role logic in the inner loop.)
- **`caps` may be inlined (small actors) or, when large, referenced** by a grant-set id
  resolved from `store` (`caps:grant_set:{id}`). S1 inlines; the store-backed path is the same
  grammar, just fetched. Either way the *grammar* below is the single source of truth.

**Rejected:** opaque session tokens with a server-side lookup on every call (defeats offline
edge verification, README §6.6). **Rejected:** putting the workspace in `caps` strings instead
of a top-level claim (would make isolation a capability you could forget to check, instead of
a hard precondition — violates "isolation first", §3.6).

---

## DECISION (forever): the capability grammar

A capability is a colon-delimited triple, lowercase, workspace-implicit:

```
<surface>:<resource>:<action>
```

- **`surface`** — exactly one of `mcp` | `store` | `bus` | `secret`. These are the three
  enforcement surfaces of §6.6 (store/bus/mcp) plus `secret` (§6.7), which is mediated the
  same way. No other surfaces; a new surface is a deliberate grammar change.
- **`resource`** — a `/`-segmented path within the surface, with `*` (one segment) and `**`
  (recursive tail) wildcards. The path meaning is surface-specific:
  - `mcp` → `<extension>.<tool>` (e.g. `hello.echo`)
  - `store` → `<table>` or `<table>/<sub>` (e.g. `note`, `note/*`)
  - `bus` → a Zenoh key-expression *tail* under the workspace prefix (e.g. `chan/*`,
    matching `ws/{id}/chan/*` — the `ws/{id}/` is added by the host, never written in a cap)
  - `secret` → `<extension>/<name>` (e.g. `github/token`)
- **`action`** — surface-specific verbs: `mcp:…:call`; `store:…:read|write`;
  `bus:…:pub|sub`; `secret:…:get`. `*` matches any action.

**Matching rule:** a request `(surface, resource, action)` is *granted* iff some held cap has
the same `surface`, its `action` is `*` or equals the request action, and its `resource`
**pattern-matches** the request resource (`*` = one segment, `**` = remaining segments). The
workspace is **not** part of the cap — it's the precondition checked first.

**Segment delimiter:** a "segment" is delimited by **`/` or `.`**. The mcp surface names
resources `<ext>.<tool>` (so `mcp:hello.*:call` matches `hello.echo` — `*` consumes the tool
segment after the dot); store/bus/secret use `/`. The wildcard rule is uniform across both
delimiters — this was settled while implementing the matcher (the dot-vs-slash boundary was
implicit in the §13 grammar; it is now explicit).

**Worked examples:**

| Held capability | Request | Result |
|---|---|---|
| `mcp:hello.echo:call` | call tool `hello.echo` | ✅ |
| `mcp:hello.*:call` | call tool `hello.echo` | ✅ |
| `mcp:hello.echo:call` | call tool `hello.secret` | ❌ no match |
| `store:note:read` | write `note` | ❌ wrong action |
| `bus:chan/**:sub` | sub `chan/eng/general` | ✅ recursive |
| (any cap, ws=`acme`) | resource in ws=`other` | ❌ **isolation fails first** |

The grammar is small on purpose (README §11.1 "expressive, safe, understandable"). It is also
**fuzzable** (testing §2 property/fuzz): the matcher's invariant is "no `*`/`**` ever matches
across a `/` it shouldn't" — a property test the `caps` crate owns.

---

## DECISION: enforcement order (the two-gate check)

`caps::check(principal, request)` is the one chokepoint. It runs **two gates, in order**:

1. **Isolation gate (hard wall, §3.6):** `principal.ws == request.ws`. Fail → `Denied::
   Workspace` immediately. No capability can override this; there is no "cross-workspace"
   capability in v1 (the near-empty platform-extension set in §11.5 is the only future
   exception, and it is out of scope here).
2. **Capability gate (§3.5):** does any held cap pattern-match `(surface, resource, action)`?
   No → `Denied::Capability`. Yes → `Allowed`.

Every surface routes through this: `store` queries, `bus` pub/sub, `mcp` tool dispatch, and
`secret` fetch all call `caps::check` *before* touching the resource. The host provides no
other path to a resource (README §3.5 — "nothing reachable except through a host-mediated
capability check").

## How it fits the core

- **Capabilities (deny test):** the matcher above — every tool/record/bus access ships a
  "denied without the grant" test (testing §2.1).
- **Tenancy/isolation (isolation test):** gate 1 — workspace B's token cannot read/write/sub
  workspace A's keys on any surface (testing §2.2).
- **Data:** grants stored as `caps:grant_set:{id}` records when not inlined; token public keys
  in `auth:key:{id}`.
- **MCP surface:** `caps` exposes no MCP tools itself in S1 (it's infrastructure); granting is
  an admin tool later.

## Testing plan

- `caps/tests/match_test.rs` — the grammar table above as cases (unit, no IO).
- `caps/tests/isolation_test.rs` — **mandatory** workspace-isolation: cross-ws denied on each
  surface.
- `caps/tests/deny_test.rs` — **mandatory** capability-deny: missing/insufficient cap denied.
- `caps/tests/match_prop_test.rs` — property test: wildcard segment-boundary invariant.
- Token: `auth/tests/token_test.rs` — mint→verify round-trip with an **injected clock**;
  tampered token rejected; expired token rejected.

## Open questions

- Grant *delegation* (an actor granting a subset to another) — needed for agents-calling-tools
  (§6.16). **RESOLVED at S5** (agent scope): `Principal::derive(sub, agent_caps)` mints a strictly
  narrower actor (same ws — delegation can't cross the wall — a distinct `agent:*` sub, the agent's
  caps, and the caller's caps as a `constraint`); `caps::check` gained **gate 2b** requiring a
  delegated request to match the constraint too — exact `agent ∩ caller` intersection, no pattern
  algebra, reusing the one chokepoint. An agent can never widen its access. Tested in
  `caps/tests/delegation_test.rs`. Signing a delegated/routed grant on the bus (token-on-the-bus) is
  still open — tracked in the mcp scope.
- Negative caps / deny rules — **rejected for v1**: deny-by-default + explicit grants is
  simpler and safer than mixing allow/deny. Revisit only if a real case needs it.
- Where the signing key lives per role (edge verifies, hub issues) — custody is README §13's
  super-admin custody open question; tracked there, not blocking S1 (S1 mints in-process).
- Inlined vs store-backed `caps` cutover threshold — measure at S2; not a forever decision.
