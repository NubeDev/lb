---
name: secrets
description: >-
  Manage Lazybones secrets — capability- and workspace-mediated credential storage read/written only
  through the host, never returned to a log, record, or result except to an authorized caller. Set,
  get, list, delete, and toggle the visibility of a secret via `secret.*`. Use when a task says "store
  a secret/credential/API key/DSN", "read a secret", "set secret visibility", or "call secret verbs".
  A secret passes THREE gates: workspace (structural) → capability (`mcp:secret.*:call`) → owner/
  visibility. A `Private` secret is walled behind its owner even against an admin holding a broad
  `secret:*:get`. Providers/adapters mediate credentials through this surface; the value never rides a
  domain record.
---

# Managing secrets (`secret.*`, the three-gate store)

A secret is credential material the host stores and hands out **only** to an authorized caller — never
to a log, a domain record, a query result, or an extension that hasn't passed the gates. It's how a
datasource's DSN, a provider key, or a webhook secret lives in the platform without leaking. The crate
is `rust/crates/secrets/` (`lb-secrets`); the host bridge is `rust/crates/host/src/host_tools/secret/`.

Every `secret.get` passes **three gates, in order**:

1. **Workspace (structural)** — `secret:{ws}:{path}` lives in the workspace namespace; a ws-B caller
   physically cannot resolve a ws-A secret.
2. **Capability** — `mcp:secret.<verb>:call` (e.g. `mcp:secret.get:call`), workspace-first.
3. **Owner / visibility** — a `Private` secret resolves only for `caller.sub() == owner`; a `Workspace`
   secret is readable by any principal past gates 1+2.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities: `mcp:secret.get:call`, `secret.set:call`, `secret.list:call`, `secret.delete:call`,
`secret.set_visibility:call`. The **grant is necessary but not sufficient** for a `Private` secret —
gate 3 (owner) still applies, so even an admin holding `secret:*:get` is denied another owner's
`Private` value.

## 2. The verbs (over `POST /mcp/call`)

| Verb | Args | Behavior |
|---|---|---|
| `secret.set` | `path, value, visibility?` | Create/overwrite — **owner-stamped** to the caller, `Private` by default. |
| `secret.get` | `path` | The three-gate read → `{path, value}` (only to an authorized caller). |
| `secret.list` | — | The secret **paths** in the workspace the caller may see — **never the values**. |
| `secret.delete` | `path` | Remove a secret (owner/cap gated). |
| `secret.set_visibility` | `path, visibility` | **Owner-only** runtime toggle of `Private` ↔ `Workspace`. |

`visibility` is `private` or `workspace`. A `Private` secret is walled behind its owner even against a
workspace admin; `Workspace` opens it to any principal that clears gates 1+2.

```bash
# store a webhook secret (Private by default, owner-stamped), then read it back
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"secret.set","args":{"path":"github/webhook","value":"whsec_…"}}'

curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"secret.get","args":{"path":"github/webhook"}}'          # → {"path":"github/webhook","value":"whsec_…"}

# open it to the whole workspace (owner only)
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"secret.set_visibility","args":{"path":"github/webhook","visibility":"workspace"}}'
```

## 3. Mediation — how adapters use secrets without seeing them

The point of the surface is that a **credential is handed to the code that needs it, not to the caller
who triggered the action**. The pattern (from the `federation` datasource extension):

- An admin registers a source and the DSN is mediated into `secret:federation/{name}` — the datasource
  *record* keeps only the **ref**, never the DSN.
- At query time the **host** pulls the DSN under the extension's OWN `secret:federation/*:get` grant
  and hands it to the connection pool — it never returns to the rule, the page, a record, or a log.

So a secret path (`secret_ref`) travels freely; the value is fetched at the last moment, by the host,
for the authorized consumer only. Same shape as any provider key or webhook secret.

## Gotchas

- **`secret.list` returns paths, never values** — enumerate to discover, `get` (gated) to read.
- **The capability is not enough for `Private`** — gate 3 checks `caller == owner`; an admin with
  `secret:*:get` is still denied another owner's `Private` secret. Use `Workspace` visibility (owner-
  set) to share.
- **`set` stamps the caller as owner** and defaults to `Private` — pass `visibility:"workspace"` to
  share on creation, or toggle later with `set_visibility` (owner only).
- **The value never leaks** — it is absent from `secret.list`, from a datasource `list`, from query
  results, and from logs; only `secret.get` (three gates) returns it.
- **Workspace-walled** — `secret:{ws}:{path}`; a ws-B caller can't resolve a ws-A secret regardless of
  caps.
- **Denials are opaque** — a missing cap, a wrong owner, and an absent secret look the same.

## Related

- The primary consumer (DSN mediation): `docs/skills/datasources/SKILL.md`,
  `docs/scope/datasources/datasources-scope.md`.
- Capability grammar + who can grant `secret:*`: `docs/skills/auth-caps/SKILL.md`.
- Scope: `docs/scope/secrets/`. README §3 (capability-first), §7 (workspace wall).
- Source: `rust/crates/secrets/` (`lb-secrets`), `rust/crates/host/src/host_tools/secret/`.
