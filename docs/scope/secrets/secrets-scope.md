# Secrets scope — extension-owned, host-mediated secrets

Status: scope (the ask). Promotes to `public/secrets/secrets.md` as each slice ships.

Capability-mediated secrets in the **one datastore**, owned by an **extension** (or a user), where the
**owner controls visibility**: a secret is either **private to its owner** — only the owning
extension may read it, even an admin cannot — or **public within the workspace** — any workspace
member that holds the capability may read it. The owner flips this **at runtime**, not by an admin
re-granting a capability. This builds on the shipped `lb-secrets` baseline (workspace-walled
`secret:{ws}:{path}` records, the `Secret` capability surface, the value never leaked to a rule,
page, log, or query result) and adds the README §6.7 "per-extension secrets" layer plus an **MCP
surface** so extensions reach it through the host-callback ABI.

> Read with: README §6.7 (envelope encryption + per-workspace/per-extension secrets — the design),
> §6.6 (caps project onto every surface), §7 (workspace = tenant). Siblings:
> `../extensions/host-callback-scope.md` (how a guest reaches host tools — `caller ∩ install-grant`),
> `../document-store/document-store-scope.md` (the same owner/visibility gate-3 shape, for docs),
> `../datasources/datasources-scope.md` (the shipped federation DSN — the first mediated consumer),
> `../auth-caps/auth-caps-scope.md` (the grammar), `../auth-caps/authz-grants-scope.md` (subjects).

---

## What already shipped (the baseline this extends)

`lb-secrets` landed with the rules/datasources plane:

- A secret is a workspace-walled record **`secret:{ws}:{path}`** holding an **opaque string value** (a
  DSN, an API key). `path` is a namespace like `federation/tsdb`.
- Access is the **`Secret` capability surface**: `secret:{path}:get` / `secret:{path}:write`, checked
  through the one `caps::check` chokepoint (workspace-first).
- Host functions `set` (gated `:write`, idempotent upsert) and `get` (gated `:get`).
- **The value is mediated** — pulled by the host/supervisor and handed to the *consumer* (a connection
  pool), **never** returned to a rule, the page, a log, or a `federation.query` result.
- Stored in SurrealDB, never a separate secrets service (rule 2).
- **Explicitly deferred** in the crate's own note: **envelope-encryption-at-rest** (the crate is the
  seam; values are plaintext-in-store for now).

This scope adds three things the baseline does not have: **(1)** extension **ownership** + an
owner-controlled **visibility** gate, **(2)** an **MCP surface** (`secret.*`) so extensions call it,
**(3)** the path to **envelope encryption** (still its own stage, but signatures shaped for it).

## Goals

- **Extensions own secrets.** A secret created by an extension is owned by that extension's `ext:{id}`
  subject (the derived `caller ∩ install-grant` principal). By default an extension may write/read
  **only its own** path namespace (`ext/{id}/*`).
- **Owner-controlled visibility (the ask), as a runtime toggle:**
  - **`Private`** — only the **owner** may `get` the value. A different extension, or even a
    workspace admin holding a broad `secret:*:get`, is **denied** (the owner wall is *below* the
    capability — this is "block access so only it can access it").
  - **`Workspace`** — any principal in the workspace that passes the capability gate may `get` it
    (this is "make it public within the workspace"). The owner sets this; it is not an admin re-grant.
- **An MCP surface** (`secret.*`) reachable over the host-callback ABI, so the UI, agents, and
  extensions CRUD secrets the same way — with `list` returning **metadata only**, never values.
- **The mediation invariant holds** — a secret value never appears in a log, a rule result, a page
  bridge response, a `federation.query` result, serialized job state, an error message, or
  `secret.list`. Only `secret.get` to an authorized principal and the server-side mediated injection
  ever touch plaintext.
- **One datastore, hot-reload-safe.** The value lives in SurrealDB, not in an extension instance — so
  an extension can be hot-reloaded without losing (or carrying) its secret (rule 4).

## Non-goals (this slice)

- **Envelope encryption at rest** — the README §6.7 master-key → per-workspace data-key →
  per-extension-secret wrapping (OS keychain on desktop, encrypted-at-rest in SurrealDB on the
  server). It is its **own dedicated stage**; `lb-secrets` stays the seam and the verb signatures
  (opaque value) do not change when it lands. Flagged loudly: **values are plaintext-in-store until
  then** — acceptable for dev, **must** ship before production secrets.
- **Team / user sharing of a secret** (the doc-store `Private | Team | User | Workspace` ladder).
  v1 is **`Private | Workspace`** only — the two the owner asked for. The relation machinery to add
  `Team`/`User` is the same `(kind,a,b)` edge docs use; deferred until a caller needs it.
- **Secret value versioning / rotation history.** Upsert-mutable now; a rotation/version model is a
  follow-up (note below).
- **Cross-workspace / global secrets.** The wall is the product — a secret is workspace-scoped. A
  hub-level master key is node/env config, not a workspace secret.
- **Admin break-glass read of a Private secret** — by default the owner wall holds even against a
  super-admin. An audited `secret.admin_read` behind its own cap is an open question, not v1.

## Intent / approach

**Mirror the document-store three gates — secrets get the same owner/visibility wall.** The shipped
baseline is gates 1+2 (workspace, then capability). This scope adds **gate 3 — ownership/visibility**,
the *which-secret-within-the-workspace* layer, identical in shape to the doc membership gate:

1. **Gate 1 — workspace** (structural): `secret:{ws}:{path}` lives in the workspace namespace; a read
   for ws A physically cannot see ws B. Unchanged.
2. **Gate 2 — capability**: `secret:{path}:get|write` via `caps::check`. Says "this actor may use the
   secret surface for this path." Unchanged.
3. **Gate 3 — ownership / visibility (NEW)**: the record carries `owner` (the creating subject) and
   `visibility: Private | Workspace`.
   - `get` on a **`Private`** secret resolves: is the caller's subject **==** `owner`? If not →
     **denied**, even with the capability. (The owner wall — stronger than, and below, the cap.)
   - `get` on a **`Workspace`** secret: any principal past gates 1+2 may read.
   - `set` (overwrite), `set_visibility`, and `delete` are **owner-only** regardless of visibility.

**Ownership is the host-derived `ext:{id}` principal, not a guest claim.** When an extension calls a
host tool, the host sets its effective principal to `caller ∩ install-grant` at `build_call_context`
— the guest cannot forge a subject. So `owner = ext:mqtt` is trustworthy because the host stamped it.
A user-owned secret is the same shape with `owner = user:ada`. At install, an extension is granted
`secret:ext/{id}/*:get|write` over **its own** path namespace and nothing else — so the *default*
posture is already "private to this extension," and the visibility toggle is what opens it up.

**Two consumption modes, one invariant.** Preserve the shipped mediation rule by separating *who
receives plaintext*:
- **Direct fetch (`secret.get`)** — the **owner** (Private) or **any ws member** (Workspace) receives
  the value to use **in its own process**: an MQTT extension opening a broker connection, a webhook
  extension signing a request. This is legitimate — the authorized principal *is* the consumer.
- **Mediated injection** — a *platform* consumer (the federation pool, an outbox target, a rule's
  datasource) references a secret **by path**; the host resolves and injects it **server-side** into
  the connection/request and **never** returns it to the referencing rule/page/log/result. This is
  the shipped federation DSN path, unchanged.

The invariant — "plaintext only crosses to an authorized direct fetch or a server-side injection,
never anywhere else" — is the load-bearing safety property and gets its own absence-asserting test.

**Why a per-secret `visibility` field, not a capability re-grant.** "Make it public within the
workspace" could be modeled as an admin granting `secret:ext/mqtt/*:get` to the `member` role. We
**reject** that: it puts a runtime, owner-owned decision into the *admin* capability plane, can't be
flipped back by the owner, and scatters the access fact across grants instead of one record the owner
controls. A `visibility` field the owner toggles at runtime is the same call the doc-store makes
(sharing is a record the owner owns, not a capability you must chase down).

## How it fits the core

- **Tenancy / isolation:** gate 1 (ws namespace) holds the wall; gate 3 (owner) is a *second*
  isolation layer **within** a workspace — extension A cannot read extension B's Private secret in the
  same workspace.
- **Capabilities (deny path):** `secret:{path}:get|write` gates the surface; the **owner gate** is the
  new gate-3 deny. Two deny tests: **(a)** no cap → refused; **(b)** cap but **not owner** of a
  `Private` secret → refused (the load-bearing owner deny, mirroring the doc non-member deny).
- **Placement:** `either`. Secret records are workspace shared data → hub-authoritative, edge
  read-cache, §6.8 idempotent apply. The **master key** (when envelope encryption lands) is node/env
  config (KMS on cloud, OS keychain on desktop) — config/role, not a code branch (rule 1).
- **MCP surface (API shape, §6.1):**
  - **CRUD:** `secret.set` (create/overwrite — owner-stamped, default `Private`), `secret.delete`
    (owner-only), `secret.set_visibility(path, Private|Workspace)` (owner-only). Each its own MCP tool
    + capability, one responsibility per file.
  - **Get / list:** `secret.get` (the three-gate read — returns the value to an authorized direct
    consumer); `secret.list` (workspace-scoped **metadata only** — path + owner + visibility + ts,
    **never values**).
  - **Live feed:** **N/A.** A secret is state, not motion. A "secret rotated" signal, if needed, is an
    inbox/event later.
  - **Batch:** **N/A** v1 (no bulk import). A future migration/rotation sweep would be a **job**.
- **Data (SurrealDB):** `secret:{ws}:{path}` extended with `owner` + `visibility`; value opaque
  (plaintext now, wrapped when envelope encryption lands — same field, same verb). State only.
- **Bus (Zenoh):** none. A secret is pure state.
- **Sync / authority:** hub-authoritative; idempotent `(table,id)` apply (§6.8). Not a new sync test.
- **Secrets:** this *is* the secret surface. The value never crosses an unauthorized boundary
  (the mediation invariant).
- **One datastore / state vs motion / stateless extensions:** value in SurrealDB, no secrets service
  (rule 2); state (rule 3); the secret lives in the store so hot-reload is safe (rule 4). ✔
- **SDK/WIT impact:** the `secret.*` verbs are reached over the **existing** host-callback ABI — no
  change to the stable plugin boundary. An extension declares the secret paths it needs in its
  manifest grant (`secret:ext/{id}/*`), the same way it declares any capability.

## Example flow

1. The **MQTT extension** (subject `ext:mqtt`, granted `secret:ext/mqtt/*:write|get` at install)
   stores its broker password: `secret.set("ext/mqtt/broker-pw", "s3cr3t")`. The host writes
   `secret:acme:ext_mqtt_broker-pw` with `owner=ext:mqtt`, `visibility=Private`.
2. The extension opens its broker connection: `secret.get("ext/mqtt/broker-pw")` → gates 1+2 ✔, gate
   3: caller `ext:mqtt` == owner ✔ → value returned **to the extension's own process** (the consumer).
3. A workspace **admin** with a broad `secret:*:get` cap calls `secret.get("ext/mqtt/broker-pw")` →
   gates 1+2 ✔, gate 3: admin subject ≠ owner, secret is `Private` → **DENIED**. ("Only it can access
   it.")
4. The MQTT extension owner stores a **shared** weather API key and makes it workspace-public:
   `secret.set("ext/mqtt/weather-key", "k")` then `secret.set_visibility("ext/mqtt/weather-key",
   Workspace)`. Now a sibling **reporting extension** with `secret:ext/mqtt/weather-key:get` calls
   `secret.get` → gate 3: `Workspace` → **value returned**. ("Public within the workspace.")
5. The owner flips it back: `set_visibility(..., Private)` → the sibling's next `get` is **DENIED**.
6. `secret.list()` for any caller returns `[{path, owner, visibility}]` — **no values**, ever.

## Testing plan (mandatory categories apply)

- **Capability-deny (mandatory):** no `secret:{path}:write` → `set` denied; no `:get` → `get` denied.
- **Ownership-deny (the NEW load-bearing test):** a **non-owner with the `:get` capability** is
  **denied** a `Private` secret (mirrors the doc non-member deny). An admin with `secret:*:get` is
  denied a `Private` extension secret.
- **Visibility toggle:** owner flips `Private → Workspace` → a sibling principal with the cap now
  reads; flip back `→ Private` → the sibling is denied again. Only the **owner** may toggle.
- **Workspace-isolation (mandatory):** a ws-B principal cannot get/list/set a ws-A secret — across
  **store + MCP** (a ws-B tool call for a ws-A path refused before resolve).
- **Mediation invariant (absence test):** assert a secret value never appears in — a log line,
  `secret.list` output, a rule result, a `federation.query` result, serialized job state, or an error
  message. Only `secret.get` (authorized) and the server-side federation injection expose plaintext.
- **Extension reuse for real (no mocks, CLAUDE §9):** a **real seeded extension install** calls
  `secret.set` then `secret.get` through the host-callback context under `caller ∩ grant`, and a
  **second** installed extension is denied the first's Private secret — proving the owner wall across
  two real guests. Real store, real install records.
- **Envelope encryption:** deferred — when the stage lands, add encrypt-at-rest round-trip + key-wrap
  + "store dump shows ciphertext, not plaintext" tests. Noted, not v1.

## Risks & hard problems

- **The mediation leak surface is broad and regression-prone.** Every *new* consumer that touches a
  secret (a new outbox target, a new datasource kind, a log of a request) is a potential plaintext
  leak. Mitigate: a single resolve choke + the absence-asserting test above, run on every consumer.
  A value that reaches a log or a serialized job is the worst-case failure.
- **Plaintext-at-rest until envelope encryption ships.** The deferral is honest but load-bearing: a
  store dump currently reveals secrets. Acceptable for dev; **must** land before any production
  credential. Flag in `public/` and STATUS so it isn't forgotten.
- **Owner identity must come from the host, never the guest.** The whole owner wall rests on
  `owner = ext:{id}` being the host-stamped `build_call_context` principal, not a value the guest
  supplied. A path that let a guest assert its own subject would let any extension impersonate the
  owner. Tested by the two-guest deny case.
- **Admin break-glass tension.** A Private extension secret an admin *cannot* read is the right
  default for least-authority, but operationally a workspace owner may need recovery. Resolve as an
  explicit, **audited** `secret.admin_read` behind its own cap — not an implicit admin bypass.
- **Key rotation / re-wrap** (when envelope encryption lands): rotating a workspace data key must
  re-wrap every secret atomically; a partial re-wrap bricks access. A job with a resumable cursor is
  the shape — out of this slice but named.

## Open questions

- **Envelope-encryption stage** — when, and the KMS/keychain choice (env var dev, cloud KMS, desktop
  `keyring`). The crate is already the seam; this decides the at-rest layer.
- **Admin break-glass** — does a super-admin ever read a `Private` extension secret? Default no;
  if yes, an audited `secret.admin_read` cap. Decide before prod.
- **`Team` / `User` sharing** — extend visibility to the full doc-store ladder, or keep
  `Private | Workspace`? Add the `(kind,a,b)` edge when a caller needs per-team secret sharing.
- **Path-namespace enforcement** — is `ext/{id}/*` enforced structurally (the grant only covers that
  prefix) or also by the `owner` field? Belt-and-suspenders: both, and confirm a typo'd path can't
  escape the prefix.
- **Rotation / versioning** — upsert-mutable now; if an audit trail or "previous value" recovery is
  needed, adopt a version model (skills-style) — decide when the first consumer needs it.

## Related

- README `§6.7` (secrets — envelope encryption + per-ws/per-extension), `§6.6` (caps), `§7` (tenancy),
  `§6.8` (sync).
- Siblings: `../extensions/host-callback-scope.md` (the `caller ∩ install-grant` path extensions use),
  `../document-store/document-store-scope.md` (the same owner/visibility gate-3 shape),
  `../datasources/datasources-scope.md` (the shipped mediated DSN consumer),
  `../auth-caps/auth-caps-scope.md`, `../auth-caps/authz-grants-scope.md` (subjects/grants),
  `../jobs/jobs-scope.md` (the future rotation/re-wrap batch).
- Code: `rust/crates/secrets/src/lib.rs` (the shipped `set`/`get` + `Secret` surface),
  `rust/crates/host/src/federation/secret.rs` (the mediated injection precedent).
- Public (as slices ship): `../../public/secrets/secrets.md`.
</content>
