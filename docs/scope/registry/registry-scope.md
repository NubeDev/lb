# Registry scope — the signed extension registry (pull · verify · cache · rollback)

Status: **shipped (S7, first slice)** — promoted to `public/registry/registry.md`. The remaining open
questions are S7 follow-ups (real HTTP source, key custody/rotation, cache GC, public catalog union).

> Read with: `../../README.md` §6.4 (registry & distribution), §6.3 (the two-tier runtime —
> the cache feeds the loader), §13 (the manifest is the contract), `../extensions/extensions-scope.md`
> (the `extension.toml` the artifact carries + the `requested ∩ admin_approved` grant the install
> already computes), `../files/files-scope.md` (the S4 `Install` record — the registry install is its
> superset), `../auth-caps/auth-caps-scope.md` (the capability grammar the new `registry:*` /
> `mcp:registry.*` grants use), `../inbox-outbox/outbox-scope.md` (the `Target`/relay seam the
> registry-host's pull endpoint is *not* — see the open questions). Re-author note: the control
> plane shape is mined from `../../STAGES.md` "Reuse: the extension server".

A node installs an extension by **pulling a signed, versioned artifact from a registry, verifying
its signature, caching it locally, and instantiating it through the runtime** — and that cached copy
lets the node run **offline** thereafter and **roll back** to a prior version by pulling (or
re-selecting) the previous one. The registry is a catalog of signed artifacts; the trust gate is the
signature check; the cache is the offline/rollback substrate. This is the S7 exit gate's first half
and the prerequisite for packaging the S6 workflow/bridge as installed artifacts.

## Goals

- A **signed artifact** — the unit of distribution: the wasm bytes + its `extension.toml` manifest,
  bound by a content **digest** (SHA-256) and an **Ed25519 signature** over that digest by a
  publisher key. Verification is "the bytes I cached are exactly the bytes the publisher signed."
- A **catalog** — per-`(ext_id, version)` metadata records (digest, publisher key id, visibility,
  `ts`) the node can `list`/resolve **without** downloading bytes, so authorization and rollback
  selection happen before any transfer (mirrors "tools are declared, not discovered" — extensions
  scope).
- **Pull · verify · cache** — `pull(ext_id, version)` fetches the artifact, **verifies the signature
  against a trusted publisher key**, and writes the verified bytes into a **local SurrealDB cache**
  keyed by digest. A tampered or unsigned artifact is **rejected before it is cached or loaded**.
- **Offline-once-cached** — a `pull` whose digest is already in the local cache returns the cached
  bytes and performs **no network call**; an install of a cached `(ext_id, version)` succeeds with
  the registry server unreachable. The cache *is* the edge's offline store (§6.4).
- **Rollback** — install version *N*, then install version *N−1* of the same `ext_id`: the prior
  artifact loads (from cache if present, else pulled) and becomes the live install, with **no
  durable workspace state lost** (the stateless-extension guarantee — state lives in store/bus/job/
  outbox, never the instance). Rollback is "pull/select the previous version", not a bespoke path.
- **Capability- and workspace-gated** — pulling, caching, and installing-from-registry are MCP verbs
  gated by `mcp:registry.<verb>:call`; a private artifact lives in **one workspace's** namespace and
  is invisible/uninstallable from another. Public catalog entries are discoverable cross-workspace
  but confer **no extra privilege** — the existing `requested ∩ admin_approved` grant still bounds
  what the installed instance may do ("public" ≠ "privileged", §6.4).

## Non-goals (S7 first slice)

- **A real network transport / a running registry-host HTTP server.** The pull path delivers through
  a host-owned **`Source` trait** (the registry's analogue of the outbox's `Target` and the agent's
  `ModelAccess`): the host calls only the trait; the test supplies a deterministic in-memory source
  (a map of `(ext_id, version) → signed artifact`, able to be told "offline" / "tampered"). A real
  HTTP `registry-host` client rides behind the same trait later — exactly as the real GitHub `Target`
  is deferred behind the relay seam (outbox scope). **This keeps the slice's only external mocked.**
- **Publisher-key custody / a key-distribution PKI.** S7 trusts a **caller-supplied set of publisher
  verifying keys** (the workspace's "who may I install from" allow-list, a test fixture here, the
  same shape the S4 `admin_approved` set took). Rotation, revocation, a key directory on the hub, and
  the trust-on-first-use story are deferred (open questions). The *verification mechanism* ships now;
  the *key-management policy* does not.
- **`DEFINE BUCKET` blob storage.** Artifact bytes are stored as a SurrealDB **record** (base64/bytes
  under `data`, the same path S4 took for doc content — `kv-mem` has no buckets in our build; the
  bucket swap is the same S7 config change noted in STATUS). The cache is SurrealDB either way (§3.2).
- **Backoff / dead-letter / concurrent-pull contention.** A failed pull errors and the caller retries;
  there is no relay loop here (pull is request-scoped, not a must-deliver background effect — see the
  open question on why the registry pull is **not** an outbox effect).
- **The native Tier-2 supervisor** and **packaging the S6 workflow as an artifact** — the next two S7
  slices; this one ships the registry they install *through*.

## Intent / approach

**Three records and one verification, composed onto primitives that already exist.** The slice adds a
small `lb-registry` crate (the artifact identity + the signature check — the only new crypto surface)
and a host `registry` service (the verb chokepoint + the cache, beside `agent`/`channel`/`assets`/
`workflow`). It deliberately **reuses** the S4 install flow and the S6 seam pattern rather than
re-cutting them:

1. **`Artifact` + `CatalogEntry` records** (`lb-registry`). An `Artifact` is `{ ext_id, version,
   manifest_toml, wasm (bytes), digest, publisher_key_id, signature }`. A `CatalogEntry` is the
   metadata subset (`{ ext_id, version, digest, publisher_key_id, visibility, ts }`) — what `list`
   returns without moving bytes.

2. **`verify_artifact`** (`lb-registry`, the new crypto surface — flagged loudly below). Recompute the
   SHA-256 digest over `manifest_toml ‖ wasm`, check it equals the claimed `digest`, then verify the
   Ed25519 `signature` over `digest` with the trusted publisher key. **Reuses the `ed25519-dalek`
   idiom from `lb_auth::keypair`/`verify` verbatim** (no JWT lib, no second crypto stack — the same
   reason auth signs tokens directly: no cross-library key-encoding seam, debugging/auth/
   valid-token-fails-verification.md). Any mismatch → `RegistryError::Unverified`, before caching.

3. **The host `registry` service** — verbs, one per file, each gated by `authorize_tool`
   (`mcp:registry.<verb>:call`, workspace-first) exactly like `authorize_workflow`:
   - `pull(ws, ext_id, version)` → if the digest is cached, return cached bytes (**no `Source`
     call** — the offline path); else `Source::fetch`, **`verify_artifact`**, **cache the verified
     bytes** (`cache_artifact`), return them. Verify-before-cache is the load-bearing order.
   - `list_catalog(ws)` / `resolve(ws, ext_id, version)` → read catalog entries (public ∪ this
     workspace's private), no bytes moved.
   - `install_from_registry(ws, ext_id, version, admin_approved, ts)` → `pull` (verified), then call
     the **existing `lb_host::install_extension`** (which already persists `requested ∩
     admin_approved` as the `Install` record and loads the component). Rollback is the same verb with
     the prior `version`. The registry install *is the S4 install with a verified-pull in front* — no
     new grant logic, no second trust model.

4. **The cache is SurrealDB, workspace-scoped.** `cached:{digest}` holds the verified bytes;
   `catalog:{ext_id}:{version}` holds the metadata. Both in the workspace namespace, so the hard wall
   makes a private artifact and a workspace's cache structurally invisible to another workspace —
   isolation is the store's job, not a check we can forget (§7). Public catalog entries are resolved
   from a shared/`public` namespace **read-only** (the open question pins the exact mechanism).

**Why a `Source` trait and not the outbox.** A registry *pull* is a request-scoped, caller-driven
**read** that must complete before the install proceeds — the caller is waiting for the bytes. An
outbox *effect* is a fire-and-forget **must-deliver write** the caller does not wait on. Forcing pull
through the outbox would invert that (the install would have to poll for its own artifact). So the
registry borrows the outbox's *seam shape* (deliver/​fetch behind a host trait, deterministic test
impl) without borrowing its *relay* — the right reuse is the pattern, not the machinery. (Rejected:
a `registry-pull` outbox effect — it models the dependency backwards.)

**Rejected alternatives.** *(a) A second datastore / a real blob CDN for artifacts* — violates §3.2;
SurrealDB records hold the cache, the CDN is a transport detail behind `Source` later. *(b) Verifying
the manifest's requested caps as the trust gate* — conflates "are these the publisher's bytes"
(signature) with "what may this instance do" (the install-time `requested ∩ admin_approved`); they
are two gates and stay two (§6.4, §11.5). *(c) A bespoke rollback record/flag* — rollback is just
installing a prior version; a flag would be durable state the stateless-extension rule forbids.

## How it fits the core

- **Tenancy / isolation:** every cache/catalog/install record is workspace-namespaced. A private
  artifact and a workspace's cache cannot be read from another workspace (store-structural, §7). The
  **mandatory workspace-isolation test**: ws-B cannot `pull`/`resolve`/`install` ws-A's private
  artifact, nor read its cache, across store + MCP.
- **Capabilities:** new grants `mcp:registry.pull:call`, `mcp:registry.list:call`,
  `mcp:registry.install:call` (auth-caps grammar). The **mandatory deny test**: without the grant,
  `pull`/`install` is refused (`RegistryError::Denied`) before any `Source` call or store write — and
  separately, **a tampered/unsigned artifact is refused even *with* the grant** (the signature gate is
  independent of the capability gate — both must pass).
- **Symmetric nodes:** the registry-*client* (pull/verify/cache/install) runs on **every** node — an
  edge installs and runs offline from its cache identically to the hub. The registry-*host* (catalog
  authority + signed-artifact origin) is a **role** (`registry-host`, cloud) mounted by config, behind
  the `Source` trait — there is no `if cloud` in any core crate; the difference is which role the
  `node` binary mounts and which `Source` impl it hands the host (README §6.4, §8).
- **One datastore:** the cache and catalog are SurrealDB records — no package store, no separate blob
  service, no second queue (§3.2). Bytes-as-record now, `DEFINE BUCKET` is the later config swap.
- **State vs motion:** the artifact + cache + catalog are **state** (SurrealDB). Nothing rides the bus
  here; a future "new version available" notification would be motion (Zenoh), but the source of truth
  is always the catalog record, never the notification (§3.3 — same discipline as the outbox's
  pending-scan-over-LIVE-query).
- **Stateless extensions:** rolling back keeps the running instance stateless — the swap is the
  hot-reload path (§6.3/§6.4: pull-verify-cache *is* hot-reload, rollback is pulling the prior
  version). The **mandatory rollback test** asserts no durable workspace state (a channel message, a
  job step) is lost across an N → N−1 → N install sequence.
- **MCP is the contract:** `registry.pull` / `registry.list` / `registry.install` are host-native MCP
  tools through the `lb_mcp::authorize_tool` chokepoint and a `registry.*` bridge — the same surface
  the agent/UI/peer-extensions all call (README §6.4 "the registry is itself a platform extension
  exposing install/list/update as MCP tools").
- **Durability:** the cache write and the `Install` record are durable SurrealDB writes; an
  interrupted pull leaves no half-cached artifact (verify-before-cache means an unverified byte string
  is never written). Installing-from-registry reuses the S4 persist-before-load discipline.
- **SDK/WIT impact:** **none.** The artifact *carries* an `extension.toml` whose `runtime.world` the
  loader already checks against the host's WIT major (extensions scope); the registry does not touch
  the WIT boundary or the SDK. The new crypto surface (`verify_artifact`) is **internal** — it adds no
  guest-visible interface. (Flagged per the non-negotiable: signing/verification is new crypto, but it
  lives entirely host-side and reuses the existing `ed25519-dalek` stack — no SDK/WIT change.)

## Example flow

Install a signed extension, run it offline, then roll back — the exit-gate walkthrough:

1. A publisher signs `hello@0.2.0`: digest = SHA-256(`manifest_toml ‖ wasm`); `signature` =
   Ed25519-sign(digest) with the publisher key. The `Artifact` + its `CatalogEntry` live at the
   registry origin (in the test, the in-memory `Source`; the workspace's publisher allow-list holds
   the matching verifying key).
2. An admin calls `registry.install(ws="acme", ext_id="hello", version="0.2.0", admin_approved=[…])`.
   The host authorizes `mcp:registry.install:call` (workspace-first) → **denied without the grant**.
3. With the grant: `pull("acme","hello","0.2.0")` — digest not cached → `Source::fetch` returns the
   artifact → **`verify_artifact`**: recompute digest, check it matches, verify the signature against
   the allow-listed publisher key. A **tampered** wasm (digest mismatch) or an **unsigned/foreign-key**
   artifact → `Unverified`, **nothing cached, nothing loaded**.
4. Verified → `cache_artifact` writes `cached:{digest}` + `catalog:hello:0.2.0` in ws `acme`. Then the
   existing `install_extension` persists `Install{ ext_id:"hello", version:"0.2.0", granted:requested∩
   admin_approved }` and loads the component. `hello.echo` is now callable (subject to its own
   `mcp:hello.echo:call` grant).
5. **Offline:** the `Source` is switched to "offline" (every `fetch` errors). `install("acme","hello",
   "0.2.0")` again → `pull` finds `cached:{digest}` → returns cached bytes, **no `Source` call** →
   install succeeds. The edge ran fully offline from its cache.
6. **Rollback:** `install("acme","hello","0.1.0")` (the prior version, already cached from an earlier
   pull, or pulled+verified if not). The `Install` record upserts to `version:"0.1.0"`; the v0.1.0
   component loads. A channel message posted at step 4 and a job step are **still present** — no
   durable state was tied to the instance (stateless-extension guarantee).

## Testing plan

Mandatory categories (testing-scope §2) and the S7-specific ones, with the files:

- **Capability-deny** (`host/tests/registry_test.rs`): `denies_pull_without_grant`,
  `denies_install_without_grant` — refused before any `Source` call or store write. *(mandatory)*
- **Workspace-isolation** (`host/tests/registry_isolation_test.rs`): `ws_b_cannot_pull_ws_a_private_
  artifact`, `ws_b_cannot_resolve_or_read_ws_a_cache` — across store + MCP. *(mandatory)*
- **Signing/verification** (`registry/tests/verify_test.rs` + `host/tests/registry_test.rs`):
  `verifies_a_correctly_signed_artifact`, `rejects_tampered_wasm` (digest mismatch),
  `rejects_unsigned_artifact`, `rejects_signature_from_untrusted_key` — and at the host level
  `install_rejects_tampered_artifact_even_with_grant` (signature gate independent of caps gate).
  *(S7-mandatory — the new crypto surface)*
- **Offline** (`host/tests/registry_offline_test.rs`): `pull_serves_cached_bytes_without_source`,
  `install_succeeds_offline_once_cached` — `Source` set to always-error; the cached path must not
  call it. *(S7-mandatory)*
- **Rollback / hot-reload** (`host/tests/registry_rollback_test.rs`): `rolls_back_to_prior_version`,
  `rollback_preserves_durable_state` (a channel message + job step survive N → N−1). *(S7-mandatory)*
- **Unit** (`lb-registry`): digest determinism, `verify_artifact` truth table, `CatalogEntry`
  projection. **Frontend** (Vitest): a `RegistryView` listing catalog entries with install/rollback
  actions against the in-memory fake, mirroring the WorkflowView slice.
- **Regression**: a test for every bug fixed this session (debugging-scope §5).

Determinism: inject `ts`; generate publisher keys from a fixed seed (`SigningKey::from_seed`); the
`Source` and publisher keys are the only externals mocked (testing §3). Per the test-runner gotchas:
multi-thread tokio + unique workspace id per node-booting test; run the host `--test` binaries
individually (never `--workspace`).

## Risks & hard problems

- **Verify-before-cache ordering is load-bearing.** If an unverified artifact is ever written to the
  cache, the offline path would later serve poison. The cache write MUST be unreachable until
  `verify_artifact` returns `Ok`. Guarded by a type-level seam (`cache_artifact` takes a
  `VerifiedArtifact` newtype that only `verify_artifact` can mint) so the order is a compile-time
  guarantee, not a convention — the §11.5 "make the class impossible" preference.
- **Digest must bind manifest *and* bytes.** Signing only the wasm would let a tampered manifest
  (e.g. an inflated `capabilities.request`) ride a valid signature. The digest covers
  `manifest_toml ‖ wasm` with a length-prefixed framing so the two fields can't be slid past each
  other. (The grant still intersects with `admin_approved`, so even a tampered request can't widen
  privilege — but binding the manifest closes the door one layer earlier.)
- **Publisher-key trust is policy, not mechanism.** The slice proves "reject anything not signed by an
  allow-listed key"; *who populates the allow-list and how it rotates* is deferred. Surfacing this
  honestly (vs. pretending the registry is "secure" end-to-end) matters — the open questions name it.
- **Public-vs-private namespace read.** Public catalog entries are resolvable cross-workspace; getting
  that read path right without leaking a *private* artifact (or letting a public read mutate anything)
  is the one place the workspace wall is deliberately crossed read-only — it needs an explicit,
  tested mechanism (open question).
- **Rollback must not resurrect state.** The guarantee is "no durable *workspace* state lost"; it is
  *not* "the old version's in-flight instance state returns" (there is none — stateless). The test
  asserts the former and that the latter is a non-concept.

## Open questions

- **Public catalog storage + read path.** ~~A dedicated `public` namespace the registry-host owns and
  every node reads read-only? Or public entries replicated into each workspace's namespace at resolve
  time?~~ **RESOLVED (S7-first):** catalog entries are **workspace-namespaced** — private entries in the
  workspace namespace, so `resolve`/`list_catalog` are structurally isolated (the mandatory test
  `ws_b_cannot_see_ws_a_cache_or_catalog_in_store` proves it). The **public read-only union** is the
  deferred additive follow-up (a `public` namespace resolved read-only, `list_catalog` = private ∪
  public); recording per-workspace now keeps isolation airtight and makes the union a later add, not a
  re-cut. See `../../sessions/registry/registry-session.md`.
- **Publisher-key allow-list storage.** A workspace record (`registry_trust:{key_id}`) the admin
  manages, mirroring how `Install` persists the approved caps? S7-first takes it as a caller arg /
  fixture (like S4's `admin_approved`); the durable record + its admin flow is the follow-up.
- **Key rotation / revocation.** Out of scope now; needs the hub identity directory (README §6.6).
  What's the minimum that lets a compromised publisher key be retired without re-signing every
  artifact — a key-id → status record consulted in `verify_artifact`?
- **Cache eviction / GC.** Cached bytes accumulate per digest. A size/LRU policy is deferred; for now
  the cache is append-only (rollback *needs* old versions retained). When does GC become real, and
  does it respect "keep the currently-installed and the immediately-prior version"?
- **`registry.update` semantics.** README §6.4 lists `install`/`list`/`update`. Is `update` just
  `install(latest)`, or does it carry a changed-caps re-approval prompt (a new `requested` set the
  admin must re-approve)? Leaning: `update` = `install` of a newer version, and any *new* requested
  cap is simply absent until re-approved (the grant intersection already enforces this) — confirm.
- **Is the registry pull ever an outbox effect?** Argued no above (it's a read the caller waits on).
  But "publish a new version to the hub" (a *write* from an authoring node) *is* a must-deliver
  effect and would ride the outbox. Confirm the boundary: pull = `Source` (sync read); publish =
  outbox `Target` (async write). Publish is out of this slice.

## Related

- README `§6.4` (registry & distribution), `§6.3` (runtime tiers — the cache feeds the loader),
  `§6.6` (identity/keys — publisher-key custody lives here eventually), `§13` (manifest is the
  contract), `§11.5` (blast radius — "public" ≠ "privileged").
- `../extensions/extensions-scope.md` (the `extension.toml` the artifact carries + the grant
  intersection the install reuses), `../files/files-scope.md` (the S4 `Install` record the registry
  install extends), `../inbox-outbox/outbox-scope.md` (the `Target`/relay seam the `Source` mirrors
  but is not), `../auth-caps/auth-caps-scope.md` (the `registry:*`/`mcp:registry.*` grants),
  `../node-roles/node-roles-scope.md` (the `registry-host` role posture).
- `../../STAGES.md` S7 + "Reuse: the extension server" (the control-plane shape mined from rubix-cube).
- Sibling: `lb_auth::keypair`/`verify` (the Ed25519 idiom `verify_artifact` reuses verbatim).
</content>
</invoke>
