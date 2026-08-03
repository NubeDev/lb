# Node-roles scope — `update.*` + the streaming upload seam: a node that can replace itself

Status: scope (the ask). Promotes to `doc-site/content/public/node-roles/node-roles.md` once shipped.

A node reports its health, its store, its identity and its extensions — but it cannot tell you
whether the **binary running it** is current, and it has no mediated way to replace itself. Today that
is a shell job. This scope gives lb the two missing seams, both generic and both embedder-filled:

1. **`update.*`** — a host-native verb family over a `BootConfig.update` provider, so an operator sees
   "you are on 0.1.0, 0.1.2 is available" and applies it from the app the node itself serves, behind
   the same caps wall and the same MCP contract as every other verb.
2. **`BootConfig.upload_sinks`** — a resumable, **never-buffered** binary upload route feeding
   embedder-registered sinks, because on an airgapped box the new artifact has to arrive somehow, and
   a data-federation sidecar is measured in **gigabytes**. lb's only large-payload ingest today
   (`POST /extensions`) JSON-encodes the artifact into a byte array at roughly 8× inflation and holds
   it in memory; nothing in that shape survives a multi-GB artifact on a 959 MB edge box.

**lb performs no update and stores no artifact.** The mechanism — a supervisor, an orchestrator, a
package manager — is the embedder's, exactly as `node-identity-scope.md` split machine identity
(rule 10). lb owns the vocabulary, the wall, the audit trail, and the bytes' safe passage.

## Goals

- **A mediated update surface.** `update.status` / `check` / `apply` / `rollback` / `history` +
  `credential.status|set|claim`, callable identically by the UI, an agent, a flow node, or a script.
- **The mechanism is the embedder's.** `BootConfig.update: Option<UpdateConfig>` carrying an
  `Arc<dyn UpdateProvider>`. No core crate names a supervisor, a package format, or a product.
- **Credential custody in the secret plane, unreadable by anyone.** The provider never touches the
  store; lb resolves the credential per call and hands over an opaque string. The sealed record is
  **host-owned**, so no principal — admin included — can read it back through `secret.get`.
- **Bytes stream, never buffer, and resume.** `PATCH` with `Content-Range` into an embedder sink;
  peak memory is one chunk regardless of artifact size; an interrupted 2 GB upload continues from its
  offset rather than starting over.
- **Additive and honest when absent.** No provider ⇒ `update.status` answers `{"supported": false}`
  and every other verb is `Unsupported` — the `UnconfiguredModel` posture. No sinks ⇒ the upload route
  is not mounted. Byte-for-byte prior behaviour for every existing embedder.

## Non-goals

- **Performing the update.** lb hands a request to a provider and reports what it says. It does not
  stage binaries, swap symlinks, write units, or restart anything.
- **Gating the update.** Health-gating, snapshotting and auto-rollback belong to whatever executes the
  swap — a process cannot gate its own replacement. lb's job ends at "accepted".
- **Storing or verifying artifacts.** The upload route is a **pipe**: it owns framing, resumption,
  bounds and the wall, and never a byte of durable artifact state. Digest verification, signature
  trust and the content-addressed cache belong to the sink's backend, which already has all three.
- **Fleet-wide rollout**, and **updating extensions** (`ext.*` owns that lifecycle, untouched).
- **Retrofitting `POST /extensions` onto the new sink.** It is the obvious follow-up and it is a
  separate scope with its own compatibility story: the ABI, the signature check and the SDK all
  assume the JSON artifact today. Decided, not deferred-by-omission — this scope does not touch it.

---

## Seam 1 — `UpdateProvider` and the `update.*` family

```rust
pub struct UpdateConfig {
    pub provider: Arc<dyn UpdateProvider>,
    /// Secret PATH (never a value). Sealed in the node's BOOT workspace, host-owned.
    pub credential_secret: Option<String>,
    /// Env var NAME (never a value) — the fallback when nothing is sealed.
    pub credential_env: Option<String>,
}

#[async_trait]
pub trait UpdateProvider: Send + Sync {
    async fn status(&self, cx: &UpdateCx) -> Result<UpdateStatus, UpdateError>;
    async fn check(&self, cx: &UpdateCx) -> Result<Vec<AvailableVersion>, UpdateError>;
    async fn apply(&self, cx: &UpdateCx, version: &str) -> Result<Accepted, UpdateError>;
    async fn rollback(&self, cx: &UpdateCx) -> Result<Accepted, UpdateError>;
    async fn history(&self, cx: &UpdateCx, limit: u32) -> Result<Vec<UpdateEvent>, UpdateError>;
    /// Drive the backend's own enrolment handshake. `Unsupported` is a normal answer.
    async fn provision_credential(&self, code: Option<&str>) -> Result<String, UpdateError>;
    /// A cheap authenticated probe — refuse a wrong credential BEFORE sealing it.
    async fn verify_credential(&self, candidate: &str) -> Result<(), UpdateError>;
}

pub struct UpdateCx { pub credential: Option<String>, pub actor: String }
```

`UpdateError = Unsupported | Unauthorized{code_required} | NotFound{version} | Conflict{reason} |
Backend(String)`, mapped onto `ToolError` at the bridge with the reason preserved in the body — a bare
refusal tells an operator nothing actionable.

**The types are pinned here, and they are generic** — no field may name a backend. A provider whose
backend has a richer notion (rubixd's `bad_versions`, an orchestrator's rollout state) maps it into
these shapes; a field that only one backend can fill is a leak and is refused in review:

```rust
pub struct UpdateStatus {
    pub supported: bool,
    pub backend: String,              // provider-chosen label, e.g. "supervisor"
    pub package: Option<String>, pub instance: Option<String>,
    pub current_version: Option<String>,
    pub signing_key_durable: bool,    // per-boot key ⇒ this update signs everyone out
    pub in_flight: Option<String>,    // tx id, when the backend reports one
    pub last: Option<UpdateOutcome>,  // {tx, outcome: Committed|RolledBack|Failed, reason}
    pub quarantined: Vec<String>,     // versions the backend refuses to retry
    pub credential: CredentialStatus, // {configured, source: Sealed|Env|None, fingerprint}
    pub target_matches_self: bool,    // the provider's own identity check (see Risks)
}
pub struct AvailableVersion { pub version: String, pub size: Option<u64>, pub source: String }
pub struct Accepted { pub tx: String }
pub struct UpdateEvent { pub tx: String, pub at: String, pub from: Option<String>,
                         pub to: Option<String>, pub outcome: String, pub reason: Option<String> }
pub struct UploadMeta { pub size: u64, pub digest_hex: Option<String>, pub meta: Value }
pub struct UploadHandle { pub id: String, pub offset: u64 }
```

Verbs are host-native, dispatched by **exact name** (`HOST_NATIVE_EXACT`), never by an `update.`
prefix: reserving a namespace against a hypothetical extension named `update` is the mistake
`ext.list` already avoided. Catalog rows land in `system/catalog/update.rs`.

| verb | cap | answers |
|---|---|---|
| `update.status` | `mcp:update.read:call` | supported? backend, package/instance, running version, **key durability**, in-flight tx, last outcome, credential state |
| `update.check` | `mcp:update.read:call` | reachable versions, provider's order, `update_available` |
| `update.apply {version}` | `mcp:update.apply:call` | accepted + tx — **not** "it worked" |
| `update.rollback` | `mcp:update.apply:call` | accepted + tx |
| `update.history {limit?}` | `mcp:update.read:call` | provider events, each merged with lb's audited actor |
| `update.credential.status` | `mcp:update.read:call` | `{configured, source, fingerprint}` — never the value |
| `update.credential.set {value}` | `mcp:update.credential:call` | verify-then-seal; returns fingerprint |
| `update.credential.claim {code?}` | `mcp:update.credential:call` | provider enrols, lb seals; returns fingerprint |

**`apply` returns accepted, never done.** The process serving the reply is the process about to be
replaced; any other contract is a lie the first time it is true. The verdict lives with the executor
and is read back through `update.status` after the node returns — which is also why `history` is a
verb and not a field. This binds the **provider** too: a backend whose apply call is synchronous
through its own health gate (minutes, ending in this process's death) must be driven through an
async accept — the provider surfaces early typed refusals (unknown version, quarantined, revision
conflict) and then detaches; it never holds `apply`'s reply hostage to a swap that kills the replier.
A backend that cannot offer an async accept is driven fire-and-validate: refusals are read
synchronously, the connection being severed after acceptance is the expected outcome, not an error.

**`status` reports key durability.** If the node's token-signing key is per-boot, the restart
invalidates every session and signs the operator out mid-update — a support call that looks exactly
like "the update broke it". A UI cannot warn about that unless the node says so, so the node says so.

### Credential custody — host-owned, verified, unreadable

The credential resolves per call: **sealed secret → env NAME → `None`**. The sealed record lives at
`secret:{boot_workspace}:{credential_secret}` and is stamped **owner = the node host principal**, not
the calling admin. That one choice settles three things at once:

- **Nobody can read it back.** The secret plane denies `get` on a `Private` secret to any non-owner
  *even with the capability*; since the owner is the host, every human caller is a non-owner. The
  value leaves the node process exactly never.
- **Anyone properly granted can rotate it.** Ownership does not become a hostage of whichever admin
  enrolled first — rotation goes through `update.credential.*`, gated by its own cap.
- **It is node-scoped, like the thing it controls.** An update is not workspace data; sealing into the
  boot workspace rather than the caller's keeps one node credential rather than one per workspace.

`set` **verifies before sealing** (`verify_credential`), so a mistyped token fails at enrolment
instead of at 3am during an update. Only a fingerprint (first/last 4 of a SHA-256 hex) ever crosses
the wire, so an operator can tell two credentials apart without seeing either. The `secret` table is
already walled out of snapshots by `snapshot_guard`; nothing here changes that.

**Prerequisite — the raw-read wall, shipped first.** The owner gate holds on the secret plane
(`secret.get` denies `Private` to any non-owner even with `secret:**:get`; `secret.list` is
metadata-only), but the guarantee above is only as strong as every read surface over the store — and
today `store.query` is not one of them. It parse-allowlists read-only `SELECT` in the caller's
workspace with **no secret-table refusal** (`host/src/store_query/run.rs`), and its cap
`mcp:store.query:call` sits in the author-tier bundle — so `SELECT * FROM secret` hands the plaintext
value to a non-admin member of the boot workspace, straight past the owner wall. `store.scan` and
`store.graph` share the property at admin tier. Therefore, as part of this scope and **landing ahead
of any credential being sealed**: `store.query`, `store.scan` and `store.graph` refuse the
`SECRET_TABLES` set structurally — the same refuse-not-redact posture `snapshot_guard` already takes,
independent of caps, asserted in the testing plan. This wall is a standalone security fix and ships
even if the rest of this scope never does.

**First-use auto-enrolment.** Zero-touch matters (an unattended box must end up enrolled with nobody
at the console), and the seam above gives the host no way to seal a credential outside a caller's
`credential.claim`. So lb closes the loop itself: when a verb resolves the credential and finds
nothing sealed and no env value, lb calls `provision_credential(None)` **once**, seals the result
host-owned, writes the standard audit record (actor = the caller whose verb triggered it, marked
`auto_enrolled`), and proceeds. `Unsupported` or `Unauthorized{code_required}` degrade to the normal
unconfigured/claim-needed answers — auto-enrolment is an optimisation of the happy path, never a
second protocol. A concurrent double-trigger is serialized on the seal; the loser re-resolves and
finds the winner's secret.

---

## Seam 2 — `upload_sinks`: resumable bytes with no buffer

```rust
/// Embedder-registered upload sinks keyed by an opaque name. Names nothing (rule 10) —
/// the exact posture of `OutboxProviders::targets`.
pub upload_sinks: Vec<(String, Arc<dyn UploadSink>)>,

#[async_trait]
pub trait UploadSink: Send + Sync {
    /// The capability a caller must hold. The sink chooses; lb enforces.
    fn required_cap(&self) -> &str;
    /// Begin — the sink allocates its own durable id and reports the offset it already holds
    /// (non-zero when this upload is being resumed against the sink's backend).
    async fn begin(&self, meta: &UploadMeta) -> Result<UploadHandle, UploadError>;
    /// Append one chunk at `offset`. Called repeatedly; must be idempotent per (id, offset).
    async fn append(&self, id: &str, offset: u64, chunk: Bytes) -> Result<u64, UploadError>;
    /// Finalize — the sink verifies and commits. lb reports the sink's verdict verbatim.
    async fn complete(&self, id: &str, meta: &UploadMeta) -> Result<Value, UploadError>;
    async fn abort(&self, id: &str) -> Result<(), UploadError>;
}
```

Routes, mounted only for registered sinks:

```
POST  /uploads/{sink}        {size, digest_hex?, meta{…}}   → 201 {id, offset}
PATCH /uploads/{sink}/{id}   Content-Range: bytes a-b/total → 200 {offset}
GET   /uploads/{sink}/{id}                                  → {offset, size}
POST  /uploads/{sink}/{id}/complete                         → the sink's verdict
DELETE /uploads/{sink}/{id}                                 → abort
```

The contract that makes this safe at any size:

- **Chunks are forwarded as they arrive.** The body is read as a stream and handed to `append` in
  bounded pieces; lb never collects a request body and never writes an artifact to its own disk. Peak
  memory is one chunk (64 KiB), independent of artifact size. A sink that forwards straight to a
  local backend means the bytes land **once**, on the backend's volume — which matters when the node
  and the backend share one SD card.
- **Offsets are the sink's truth.** A `PATCH` whose range does not begin at the sink's current offset
  is a **409 carrying the correct offset**, so a client that lost track resumes without guessing.
  Resumption therefore survives an lb restart, because lb holds no upload state to lose.
- **Resume identity is the digest.** A `begin` carrying a `digest_hex` the sink already holds a
  partial for returns the **existing** handle and its offset, not a fresh id — so a client that lost
  its id (browser refresh, new session) resumes instead of double-filling the backend's disk with a
  second partial of the same artifact. A sink given no digest allocates fresh ids and owns the
  consequence.
- **Bounded by config, never unlimited.** `max_upload_bytes` per sink (declared by the sink) is
  checked at `begin` against the declared size, and enforced as a running total during append. A sink
  may refuse at `begin` for its own reasons — no disk, wrong digest shape — and that refusal is
  passed through with its reason.
- **Cap-gated per sink.** The sink names the capability; the gateway checks it on every call in the
  sequence, not only at `begin`, so a session that loses its grant mid-upload stops there.
- **lb does not verify artifacts.** It carries `digest_hex` and `meta` as opaque values to
  `complete`. Verification, signature trust, and the durable cache are the backend's — it already has
  them, and a second trust chain in lb would be one more thing free to disagree.

Why a generic sink registry rather than a route that knows where the bytes go: this is the same
argument the outbox-target registry already settled. The embedder knows its backend; lb knows framing,
bounds and the wall. A sink named `"package"` on one host and `"firmware"` on another needs no lb
change, and lb never learns whose bytes it moved.

---

## How it fits the core

- **Tenancy / isolation:** the update verbs are **node-scoped** — the deliberate exception
  `store.status`/`store.compact` already set. They neither read nor write workspace data, and the
  answer is the same in every workspace, by design. The credential is sealed in the boot workspace and
  host-owned; uploads are workspace-agnostic and gated by capability alone.
- **Capabilities:** three grants split by blast radius — reading a version is not applying one, and
  applying one is not holding the backend's credential. Default grants: `read` to workspace-admin;
  `apply` and `credential` to workspace-admin only, never to a member and **never in the default agent
  capability ceiling** (an agent that can replace the node's binary is a different product). Denials
  are the standard opaque `ToolError::Denied` at the bridge, re-checked inside each verb.
- **Placement:** either. A cloud node behind an orchestrator and an edge node under a supervisor fill
  the same seam with different providers — role is config, never a code branch.
- **MCP surface:** get-list + command for `update.*`. **No SSE:** the interesting motion is the node
  going away and coming back, which no stream on that node can report; callers poll `/health` then
  `update.status`. The upload lane is deliberately **not** MCP — binary bytes do not belong in a JSON
  tool call, which is precisely the mistake `POST /extensions` is living with.
- **Data:** no new table. `update.history` comes from the provider (the executor's journal is the
  authority; a second copy would be free to disagree). lb writes one record of its own: an **audit**
  entry per `apply`/`rollback`/`credential.*` and per completed upload — actor, workspace, version or
  sink+digest, verdict — because "who replaced the binary on this box" must survive the binary.
- **Secrets:** covered above; the value is never logged, echoed, returned, or snapshotted.
- **Bus:** none. Node-local by definition.

## Example flow

1. UI calls `update.status` → `{"supported": true, "current_version": "0.1.0", "signing_key_durable":
   false, "credential": {"configured": false}}`. The page shows the enrol card *and* the
   "you will be signed out by this update" warning, before anything is clicked.
2. Operator clicks **Claim** → `update.credential.claim {}`. The provider performs the backend's
   one-time handshake and returns the plaintext **to lb, not to the UI**. lb seals it host-owned in the
   boot workspace and returns `{"configured": true, "source": "backend", "fingerprint": "rbd_…fvw"}`.
   A backend needing a second factor answers `Unauthorized{code_required: true}` and the UI re-submits
   with `{code}`.
3. `update.check` → `{"current": "0.1.0", "newest": "0.1.2", "update_available": true, "available":
   [{"version": "0.1.2", "size": 151270900, "source": "remote"}]}`.
4. **Airgapped variant of step 3.** No remote; the operator has a 2.4 GB signed sidecar bundle on a
   laptop. The UI opens `POST /uploads/package {size, digest_hex}`, streams `PATCH`es, and the link
   drops at 1.4 GB. It re-`GET`s the offset and continues from there. `complete` returns the backend's
   verdict; the version now appears in `update.check`. lb's memory never exceeded one chunk and its
   disk never held a byte of it.
5. `update.apply {"version": "0.1.2"}` → cap re-checked, credential resolved, audit written, provider
   called → `{"accepted": true, "tx": "…"}`.
6. The backend snapshots, stages, restarts. **This node dies mid-flight.** The UI, which expected
   exactly that, polls `/health` until it answers.
7. `update.status` → `{"current_version": "0.1.2", "last": {"outcome": "committed"}}` — or, if the new
   binary failed its gate, `{"current_version": "0.1.0", "last": {"outcome": "rolled-back", "reason":
   "health-probe-failed"}}`. The honest answer, from the node that came back.

## Testing plan

Mandatory categories:

- **Capability deny** — each verb refused without its grant; `update.read` alone cannot `apply`;
  `update.apply` alone cannot `credential.set`; the default agent ceiling holds none of the three; an
  upload is refused at `begin` **and** at a mid-sequence `PATCH` when the grant is revoked. Denials
  opaque.
- **Workspace isolation** — the sealed credential is resolved identically from any workspace (it is
  node-scoped) and is **unreadable from every one of them**, on **every** read surface: `secret.get`
  on the path is denied to a workspace admin holding `secret:*:get`, because the owner is the host;
  and `SELECT * FROM secret` via `store.query` (author cap), plus `store.scan` and `store.graph`
  (admin caps), are refused at the table wall. All four asserted, not assumed — a test that only
  covers `secret.get` tests the locked door and ignores the window.
- **Unconfigured** — `update = None` ⇒ `{"supported": false}` and clean `Unsupported`s;
  `upload_sinks` empty ⇒ the routes are absent (405/404 as today), and every existing route is
  byte-for-byte unchanged.

No mocks (rule 9): tests boot a real node through `boot_full` against a real in-test HTTP backend
speaking the same shapes. Key cases: verify-before-seal refuses a bad credential and leaves the store
untouched; a sealed credential beats the env NAME; `apply` writes exactly one audit record that
survives restart; the plaintext appears in no response body and no log line (asserted on both);
`provision_credential` returning `Unsupported` degrades to "paste it instead", not an error page;
first-use auto-enrolment seals exactly one secret under concurrent triggers and writes an
`auto_enrolled` audit record. Upload: a **3 GB** payload through a memory-capped process (cgroup
`MemoryMax`) — assert completion and flat RSS, which is the entire point and is meaningless as a unit
test; resume after an **lb restart**; a `begin` re-sent with the same `digest_hex` returns the
existing handle and offset, never a second partial; a wrong-offset `PATCH` returns 409 with the right
offset; a declared size over the sink's ceiling is refused at `begin`, and a stream that exceeds its
declared size is cut off mid-append.

## Risks & hard problems

- **The reply outlives the responder.** Everything about `apply`'s contract exists to keep that honest.
  The temptation to await the outcome must be refused; a node awaiting its own replacement returns
  nothing.
- **Tokens die with the signing key.** Mitigated by reporting durability in `status` so the UI warns
  before applying — the cheapest possible fix for the worst failure story this feature has.
- **A credential that grants more than updating.** lb cannot narrow a backend's credential, so
  `mcp:update.credential:call` is documented as **equivalent to backend admin** and kept out of every
  default role bundle. The real narrowing happens backend-side (rubixd's scoped tokens); lb's job is to
  make the grant's weight visible rather than quietly bundle it.
- **Backpressure through a proxy.** A resumable upload that streams faster than the sink drains must
  apply backpressure, not accumulate — `append` is awaited per chunk, and the route must not
  read-ahead. Getting this wrong reintroduces the buffer the seam exists to remove, invisibly.
- **Version strings are the backend's.** lb must not parse, compare, or order them. `check` returns
  the provider's order; the UI shows it.

## Decisions (no open questions)

1. **No idempotency key on `apply`.** The provider's in-flight `Conflict` is authoritative — it is the
   only party that can see the race. A double-click gets a typed refusal, not a second transaction.
2. **`history` comes from the provider**, merged with lb's audit by tx id: the provider is the
   authority on *what happened to the binary*, lb's audit on *who asked*.
3. **The credential is sealed in the boot workspace and host-owned** (§Credential custody) — not the
   caller's workspace and not caller-owned. Node-scoped resource, node-scoped custody, unreadable by
   every principal.
4. **`credential.set` verifies before sealing.** A store write that has not been proven to work is a
   trap set for the next outage.
5. **The upload lane is HTTP, not MCP**, and lb holds no upload state — offsets are the sink's, so
   resumption survives an lb restart for free.
6. **`update.*` is dispatched by exact name**, never by prefix, so no extension namespace is reserved.
7. **A typed provider + named verb family beats a generic "embedder registers arbitrary MCP tools"
   registry.** The registry is tempting (`Registry::register_local_dispatch` already exists), and to
   be precise about what it does give: registry dispatch **is** caps-gated with full parity
   (`mcp:<ext>.<tool>:call`, workspace-first). It is still wrong here, for three exact reasons: the
   pseudo-extension is **discoverable to callers yet invisible to the lifecycle** — it surfaces in
   `system.tools`/`tools.catalog` (which walk the Registry) but not in `ext.list` (which reads
   `Install` records), so it can be called but never listed, versioned, or uninstalled; it sits
   outside the signature/manifest trust gate entirely; and the collapsed three-cap grant design
   (`mcp:update.read:call` covering `status`/`check`/`history`) is only expressible through the
   host-native cap-alias table — via the registry every tool gates on its own literal name. And it
   gives every embedder a private vocabulary no shared UI can target. One vocabulary, many backends —
   that is the whole value.
8. **`POST /extensions` is not migrated onto the sink in this scope** (§Non-goals): it is the right
   follow-up and it carries ABI, signature and SDK compatibility work that would swamp this ask.
9. **The raw-read wall ships first and independently.** `store.query`/`store.scan`/`store.graph`
   refuse `SECRET_TABLES` structurally (§Credential custody) before any credential is sealed. The
   host-owned posture without this wall is theater, so the wall is not optional and not sequenced
   behind the seam.
10. **Zero-touch is first-use auto-enrolment inside lb**, not a host back door: lb calls
    `provision_credential(None)` when resolution finds nothing, seals host-owned, audits with the
    triggering caller. No separate host-side seal API exists — every path into the sealed record is
    one of `set`, `claim`, or auto-enrolment, all audited.
11. **The provider types are pinned and generic** (§Seam 1). Backend-specific vocabulary
    (`bad_versions`, rollout phases) is mapped into `quarantined`/`UpdateOutcome`, never surfaced as
    fields only one backend can fill.

## Related

- `node-roles/embed-node-scope.md` — the `BootConfig`/`boot_full` seam this extends by two fields.
- `BootConfig.identity` (`rust/node/src/config.rs`) + `GET /node` — the shipped identity seam: the
  same "embedder supplies the platform-specific half, lb owns the seam and the surface" shape
  (rule 10); the closest precedent. (It shipped without a scope file of its own — do not cite
  `node-identity-scope.md` here; it does not exist in this repo.)
- `secrets/` — the `secret:{ws}:{path}` plane, the owner/visibility gates decision 3 relies on, and
  `snapshot_guard`.
- `host-tools/host-tools-scope.md` — the neighbouring host-native family and its file layout.
- `inbox-outbox/` — `OutboxProviders::targets`, the registry posture `upload_sinks` copies.
- Downstream: `NubeIO/rubix-ai` → `docs/scope/deploy/self-update-scope.md`, which fills both seams
  against `NubeIO/rubix-fleet` → `docs/scope/deploy/rubixd/programmatic-lifecycle-scope.md`.
