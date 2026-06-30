# Secrets

Extension-owned, host-mediated secrets — an opaque value (a DSN, an API key) stored as a
workspace-walled record `secret:{ws}:{path}`, gated by the `Secret` capability surface, with an
owner-controlled **visibility** wall. The value is **mediated**: it crosses only to an authorized
direct consumer or a server-side injection, never to a rule, page, log, query result, serialized
job state, error message, or `secret.list`.

## The three gates

Every access runs three gates, in order:

1. **Workspace** (structural) — `secret:{ws}:{path}` lives in the workspace namespace; a read for
   ws A physically cannot see ws B. Enforced by the store namespace selection.
2. **Capability** — `secret:{path}:get|write` via the one `caps::check` chokepoint (workspace-first).
3. **Ownership / visibility** — the record carries `owner` (the host-stamped creating subject) and
   `visibility: Private | Workspace`:
   - `get` on a **`Private`** secret requires `caller.sub() == owner`; denied otherwise, **even
     with the capability** (an admin holding `secret:**:get` is denied another extension's Private
     secret). The owner wall is *below* the cap.
   - `get` on a **`Workspace`** secret: any principal past gates 1+2 may read.
   - `set` (overwrite), `set_visibility`, and `delete` are **owner-only** regardless of visibility.

## Ownership

`owner` is the host-derived principal (`ext:{id}` for an extension via `caller ∩ install-grant`, or
`user:…`), **never a guest claim** — the host stamps `owner = principal.sub()` at write time. So a
path that let a guest assert its own subject would defeat the wall; it is tested by the two-guest
deny (extension A cannot read extension B's Private secret in the same workspace).

At install, an extension is granted `secret:ext/{id}/*:get|write` over its own namespace and
nothing else — the default posture is already "private to this extension." The visibility toggle is
what opens a secret up, and it is an **owner-owned runtime decision**, not an admin capability
re-grant (the owner can flip `Workspace → Private` back without chasing down grants).

## Consumption modes

- **Direct fetch** (`secret.get`) — the owner (Private) or any ws member (Workspace) receives the
  value to use **in its own process** (an MQTT extension opening a broker connection).
- **Mediated injection** — a platform consumer (the federation pool, an outbox target) references a
  secret **by path**; the host resolves and injects it **server-side** and never returns it to the
  referencing rule/page/result. The shipped federation DSN path
  (`host::federation::secret::mediate_dsn`) is the precedent.

The **mediation invariant** — plaintext only crosses to an authorized direct fetch or a server-side
injection, never anywhere else — is the load-bearing safety property and has an absence-asserting
test (the value never appears in `secret.list`, an error message, etc.).

## Surface

- `lb-secrets` crate: `set` (default `Private`), `set_with` (explicit visibility), `get`,
  `set_visibility`, `delete`, `list` (metadata only).
- `secret.*` MCP tools over the host-callback ABI: `secret.set`, `secret.get`,
  `secret.set_visibility`, `secret.delete`, `secret.list`. Each runs the MCP `mcp:<tool>:call` gate
  first, then the crate's three-gate read.
- `secret.list` returns `{path, owner, visibility}` — **never values**. Listing requires the
  `secret:**:get` browse grant; visibility gates the *value*, not existence (an owner needs to
  discover a shared secret's path to request access).

## Data

Stored in SurrealDB (the one datastore — never a separate secrets service, rule 2): the `secret`
table per workspace, record id derived from the path, value opaque. State only (rule 3); the value
lives in the store so an extension hot-reload is safe — the secret is neither lost nor carried
(rule 4). Hub-authoritative, idempotent `(table,id)` apply.

## Deferrals (flagged, not shipped in this slice)

- **Envelope encryption at rest** — README §6.7 master-key → per-workspace data-key → per-secret
  wrapping (KMS on cloud, OS keychain on desktop). **Values are plaintext-in-store until this
  lands; acceptable for dev, must ship before production secrets.** The crate is already the seam;
  verb signatures (opaque value) do not change when it arrives.
- **Team / User sharing** — v1 is `Private | Workspace` only. The `(kind,a,b)` edge to add
  `Team`/`User` is deferred until a caller needs it.
- **Admin break-glass** — by default the owner wall holds even against a super-admin. An audited
  `secret.admin_read` behind its own cap is an open question, not v1.
- **Rotation / versioning** — upsert-mutable now; a version/audit model is a follow-up.
