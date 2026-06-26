# Registry (as built)

The signed extension registry: a node **pulls** a signed artifact, **verifies** it, **caches** it
locally, **installs** it through the existing runtime, runs it **offline** once cached, and **rolls
back** to a prior version. Shipped in S7 (the first platform-maturity slice). The full ask is
[`../../scope/registry/registry-scope.md`](../../scope/registry/registry-scope.md); the build log is
[`../../sessions/registry/registry-session.md`](../../sessions/registry/registry-session.md).

> Read with: README §6.4 (registry & distribution), [`../extensions/extensions.md`](../extensions/extensions.md)
> (the `extension.toml` the artifact carries + the `requested ∩ admin_approved` grant the install
> reuses), [`../inbox-outbox/inbox-outbox.md`](../inbox-outbox/inbox-outbox.md) (the `Target`/relay
> seam the `Source` mirrors but is not).

## What exists

### `lb-registry` — artifact identity + verification (the one new crypto surface)

- **`Artifact`** — the untrusted unit of distribution: `{ ext_id, version, manifest_toml, wasm,
  digest_hex, publisher_key_id, signature }`. Untrusted until verified.
- **`digest(manifest_toml, wasm)`** — SHA-256 with **length-prefixed framing** (`len ‖ manifest ‖ len
  ‖ wasm`), so the digest binds the manifest AND the bytes and no byte can slide across the boundary.
- **`verify_artifact(artifact, trusted_keys) → VerifiedArtifact`** — recompute + match the digest, then
  Ed25519-verify the signature against the allow-listed publisher key. Any failure →
  `RegistryError::Unverified`. **Reuses the `lb_auth` `ed25519-dalek` idiom verbatim** — no JWT/COSE
  library, no second crypto stack.
- **`VerifiedArtifact`** — a newtype whose **only constructor is `verify_artifact`**. The cache accepts
  only this, so an unverified artifact *cannot* be cached — verify-before-cache is a compile-time
  guarantee, not a call-ordering convention.
- **`CatalogEntry`** — the bytes-free metadata (`{ ext_id, version, digest_hex, publisher_key_id,
  visibility, ts }`) listed/resolved without moving bytes; the basis for offline rollback selection.

### The host `registry` service (beside `agent`/`channel`/`assets`/`workflow`)

Holds no durable state of its own; the cache, catalog, and install record are SurrealDB records in the
workspace namespace. One verb per file:

- **`Source` trait** — the fetch seam (the outbox `Target` / agent `ModelAccess` analogue). The host
  calls only the trait; the test supplies a deterministic in-memory source; a real HTTP `registry-host`
  client rides behind it later. *Why a `Source` and not the outbox:* a pull is a request-scoped read the
  caller waits on, not a fire-and-forget must-deliver write — routing it through the outbox would invert
  the dependency.
- **`pull`** — if the version is cached (a catalog entry holds its digest and the bytes are present),
  return them **without calling the source** (the offline path); else `Source::fetch` → **verify** →
  **cache** the verified bytes → record the catalog entry. A tampered/unsigned/untrusted artifact is
  refused here — never cached, never returned.
- **`cache_artifact` / `read_cached`** — the local cache, keyed by content digest (`registry_cache:
  {digest}`), workspace-namespaced (a ws-B install/relay can never read ws-A's cache — structural).
  `cache_artifact` takes only a `VerifiedArtifact`.
- **`record_catalog` / `list_catalog` / `resolve`** — the catalog reads (`registry_catalog:{ext}:{ver}`),
  workspace-namespaced. Distinct versions of one extension coexist — the precondition for rollback.
- **`install_from_registry`** — `pull` (verified) THEN the **existing** S4 `install_extension` (which
  persists `requested ∩ admin_approved` as the `Install` record and loads the component). **Rollback is
  the same verb with a prior version** — no bespoke path; the `Install` record upserts.
- **`authorize_registry`** — the `mcp:registry.<verb>:call` gate (workspace-first), like
  `authorize_workflow`.
- **`registry.*` MCP bridge** — `list`/`resolve` over the one MCP contract. `pull`/`install` are typed
  entries (not bridged) because they need the `Source` + publisher keys, exactly as `workflow::triage`
  needs a `ModelAccess`.

### UI

A `RegistryView` + `registry.api` client mirroring the verbs one-to-one, with a faithful in-memory fake
exercising the capability gate, the signature gate (an untrusted version is refused), and rollback. See
[`../frontend/frontend.md`](../frontend/frontend.md).

## The rules it holds

- **Two independent gates.** The **capability** gate (`mcp:registry.<verb>:call`) and the **signature**
  gate (`verify_artifact`). Granted ≠ trusted: a fully-granted caller is still refused a bad artifact,
  and a trusted artifact still needs the grant.
- **Verify before cache.** Enforced by the type system (`cache_artifact` takes a `VerifiedArtifact`), so
  the offline path can never serve poison.
- **One datastore.** Cache + catalog are SurrealDB records (bytes-as-record in the `kv-mem` build; the
  `DEFINE BUCKET` swap is a later config change). No package store, no blob service, no second queue.
- **Workspace is the hard wall.** Cache, catalog, and install records are namespace-scoped; a private
  artifact and a workspace's cache are structurally invisible to another workspace.
- **Stateless extensions → safe rollback.** Pull-verify-cache *is* the hot-reload path; rollback is
  pulling the prior version. No durable workspace state is tied to the instance, so an N→N−1 rollback
  preserves channel/job state (proven through real wasm).
- **Symmetric nodes.** The registry *client* runs on every node (an edge installs + runs offline from
  its cache identically to the hub); the registry *host* (catalog authority + signed-artifact origin) is
  a cloud **role** behind the `Source` trait — no `if cloud` in any core crate.

## Tested

145 Rust + 22 Vitest + 2 shell tests pass overall. Registry-specific: signing/verification (the new
crypto surface — tampered wasm/manifest, unsigned, foreign-key, unknown-key all rejected),
capability-deny (each verb), workspace-isolation (store + MCP), offline (cached install with the source
unreachable, asserting zero source calls), and rollback/hot-reload (durable state preserved across
N→N−1 through real wasm). Externals mocked: the `Source` + publisher keys only; real SurrealDB + Zenoh +
wasm everywhere else.

## Not yet built

A real HTTP `Source`/`registry-host` server (the in-memory source is the only stub); a durable
publisher-key allow-list + the admin trust-management flow; key rotation/revocation (needs the hub
identity directory); cache eviction/GC (keeping the current + immediately-prior version); the public
catalog read-only union (this slice ships per-workspace catalog entries); `registry.update` semantics;
gateway/Tauri wiring for `registry_*`. Tracked in [`../../STATUS.md`](../../STATUS.md) and the scope's
open questions.
