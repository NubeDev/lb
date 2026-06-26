# Registry — the signed extension registry (pull · verify · cache · rollback) (session)

- Date: 2026-06-26
- Scope: ../../scope/registry/registry-scope.md
- Stage: S7 — platform maturity (STAGES.md). This is the first S7 vertical slice.
- Status: done

## Goal

Build the **extension registry** slice end to end (store → caps → bus → MCP → UI): a node pulls a
**signed** artifact, **verifies** it, **caches** it locally, **installs** it through the existing
runtime, runs it **offline** once cached, and **rolls back** to a prior version — and rejects a
tampered/unsigned/untrusted artifact. This is the first half of the **S7 exit gate** ("an extension
installs from the signed registry, runs offline once cached, and rolls back to a prior version") and
the prerequisite for the other two S7 slices (native Tier-2, packaging the S6 workflow as artifacts).

## What changed

The registry scope doc was a stub, so per HOW-TO-CODE/SCOPE-WRITTING it was **authored first**
([scope/registry/registry-scope.md](../../scope/registry/registry-scope.md)) before any code.

New crate **`lb-registry`** — artifact identity + the one new crypto surface (verification):
- [crates/registry/src/digest.rs](../../../rust/crates/registry/src/digest.rs) — `digest(manifest, wasm)`,
  SHA-256 with **length-prefixed framing** so the digest binds manifest AND bytes (a byte cannot slide
  across the boundary); `digest_hex` for the cache key.
- [crates/registry/src/model/artifact.rs](../../../rust/crates/registry/src/model/artifact.rs) — the
  untrusted `Artifact` and the **`VerifiedArtifact` newtype** whose only constructor is
  `verify_artifact` (the load-bearing seam).
- [crates/registry/src/model/catalog.rs](../../../rust/crates/registry/src/model/catalog.rs) —
  `CatalogEntry` (bytes-free metadata) + `Visibility`.
- [crates/registry/src/verify.rs](../../../rust/crates/registry/src/verify.rs) — `verify_artifact`:
  recompute+match the digest, then Ed25519-verify the signature against an allow-listed publisher key.
  **Reuses the `ed25519-dalek` idiom from `lb_auth` verbatim** — no second crypto stack.

New host **`registry` service** (beside `agent`/`channel`/`assets`/`workflow`), one verb per file:
- [source.rs](../../../rust/crates/host/src/registry/source.rs) — the `Source` fetch seam (the
  registry's analogue of the outbox `Target` / agent `ModelAccess`).
- [cache.rs](../../../rust/crates/host/src/registry/cache.rs) — `cache_artifact` (**takes only a
  `VerifiedArtifact`** → verify-before-cache is a compile-time guarantee) + `read_cached`.
- [catalog.rs](../../../rust/crates/host/src/registry/catalog.rs) — `record_catalog` / `list_catalog` /
  `resolve`.
- [pull.rs](../../../rust/crates/host/src/registry/pull.rs) — fetch · **verify** · cache, serving the
  cached copy **without calling the source** (the offline path).
- [install.rs](../../../rust/crates/host/src/registry/install.rs) — `install_from_registry`: pull THEN
  the **existing** `lb_host::install_extension` (S4). Rollback = the same verb, a prior version.
- [authorize.rs](../../../rust/crates/host/src/registry/authorize.rs) — the `mcp:registry.<verb>:call`
  gate (workspace-first), like `authorize_workflow`.
- [tool.rs](../../../rust/crates/host/src/registry/tool.rs) — the `registry.*` MCP bridge (the
  store-only read verbs `list`/`resolve`; pull/install are typed because they need the `Source` + keys,
  exactly like `workflow::triage` needs a `ModelAccess`).

UI **`registry` feature** (mirrors the WorkflowView slice): `registry.types`, `registry.api` (one call
per verb), a faithful in-memory `registry.fake` (capability gate + signature gate + rollback),
`useRegistry`, `RegistryView`, and a Vitest spec. Wired into the `fake.ts` dispatcher.

Workspace wiring: `lb-registry` added to members + path deps; `sha2` added to the stack; `ed25519-dalek`
added to host **dev**-deps (tests sign artifacts). The `registry-host` role crate's doc updated to name
it the **server** end of the `Source` seam (still a placeholder — no HTTP transport this slice).

## Decisions & alternatives

- **A `Source` trait, NOT the outbox, for pull.** A pull is a request-scoped READ the caller waits on;
  an outbox effect is a fire-and-forget must-deliver WRITE. Routing pull through the outbox would invert
  the dependency (the install would poll for its own artifact). So the registry borrows the outbox's
  *seam shape* (deliver/fetch behind a host trait, deterministic test impl) without its *relay*.
  Rejected: a `registry-pull` outbox effect — models the dependency backwards.
- **Verify-before-cache enforced by the type system.** `cache_artifact` accepts only a
  `VerifiedArtifact`, and the only constructor is `verify_artifact`. So "an unverified artifact is never
  cached" is a compile-time guarantee, not a call-ordering convention the next edit could forget (the
  §11.5 "make the class impossible" preference). Rejected: documenting "call verify first" — too fragile
  for the one invariant that, if broken, poisons the offline path.
- **The digest binds manifest ‖ wasm with length framing.** Signing only the wasm would let a tampered
  manifest (e.g. an inflated `capabilities.request`) ride a valid signature; signing the bare concat
  would let bytes slide across the boundary. Framing closes both. (The grant intersection still
  neutralizes an inflated request — defense in depth, but the digest closes the door a layer earlier.)
- **Rollback is `install(prior_version)`, no bespoke path.** A rollback flag would be durable state the
  stateless-extension rule forbids; rollback is just pulling/selecting the previous version, and the S4
  `Install` record upserts to it. The hot-reload test already proved durable state survives an instance
  swap — rollback inherits that guarantee.
- **The registry install composes the S4 install, not a copy.** `install_from_registry` = `pull` +
  `install_extension`. One trust model, one `requested ∩ admin_approved` grant computation. The
  signature gate (in pull) and the capability/grant gate (in install) stay two independent gates.
- **Catalog entries are workspace-namespaced (S7-first).** The "public catalog namespace" open question
  is resolved conservatively: private entries in the workspace namespace, so the isolation guarantee is
  airtight and structural; the public read-only union is a later additive change, not a re-cut.
- **`tier` stays "wasm" this slice.** The manifest already carries `tier="native"`; the native Tier-2
  supervisor is the *next* S7 slice. The registry installs whatever the loader accepts.

## Tests

Mandatory categories (testing-scope §2) + the S7-specific ones, all green. Externals mocked: the
`Source` and the publisher keys only (testing §3); real embedded SurrealDB + in-proc Zenoh + **real wasm**
(`hello` v0.1.0 / v0.2.0) everywhere else.

- **Signing/verification** (the new crypto surface) — `lb-registry` unit
  ([verify.rs](../../../rust/crates/registry/src/verify.rs), [digest.rs](../../../rust/crates/registry/src/digest.rs)):
  correctly-signed verifies; tampered wasm / tampered manifest / unsigned / foreign-key / unknown-key all
  rejected; malformed publisher key rejected at the boundary; digest determinism + framing.
- **Capability-deny** (mandatory) — `denies_pull_without_grant` (refused at the gate before any source
  call/store write).
- **Signature × capability** — `install_rejects_tampered_artifact_even_with_grant`,
  `pull_rejects_artifact_signed_by_untrusted_key` (the signature gate is independent of the cap gate;
  nothing installed).
- **Workspace-isolation** (mandatory, store + MCP) — `ws_b_cannot_see_ws_a_cache_or_catalog_in_store`
  (ws-B resolve/read_cached → None, and ws-B's offline pull can't ride ws-A's cache),
  `ws_b_principal_is_denied_at_the_registry_gate_for_ws_a` (workspace-first MCP deny).
- **Offline** (S7-mandatory) — `pull_serves_cached_bytes_without_source`,
  `install_succeeds_offline_once_cached` (assert **zero** source fetches on the cached path),
  `offline_with_nothing_cached_fails` (the negative).
- **Rollback / hot-reload** (S7-mandatory) — `rolls_back_to_prior_version_preserving_durable_state`
  (install v0.2.0 → post channel messages → roll back to v0.1.0 → v1 instance answers AND the channel
  history is intact), `rollback_is_offline_when_prior_version_cached`.
- **Frontend** (Vitest, [RegistryView.test.tsx](../../../ui/src/features/registry/RegistryView.test.tsx)):
  install-signed, refuse-unverified, rollback, deny-no-grant — through the real api → invoke → fake path.

Green output (Rust registry; full host suite + UI suite also green):

```
##### lb-registry unit #####
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

##### host: registry_test #####
running 4 tests
test denies_pull_without_grant ... ok
test pull_rejects_artifact_signed_by_untrusted_key ... ok
test install_rejects_tampered_artifact_even_with_grant ... ok
test installs_a_signed_artifact_end_to_end ... ok
test result: ok. 4 passed; 0 failed

##### host: registry_offline_test #####
running 3 tests
test offline_with_nothing_cached_fails ... ok
test pull_serves_cached_bytes_without_source ... ok
test install_succeeds_offline_once_cached ... ok
test result: ok. 3 passed; 0 failed

##### host: registry_rollback_test #####
running 2 tests
test rolls_back_to_prior_version_preserving_durable_state ... ok
test rollback_is_offline_when_prior_version_cached ... ok
test result: ok. 2 passed; 0 failed

##### host: registry_isolation_test #####
running 2 tests
test ws_b_principal_is_denied_at_the_registry_gate_for_ws_a ... ok
test ws_b_cannot_see_ws_a_cache_or_catalog_in_store ... ok
test result: ok. 2 passed; 0 failed
```

```
# UI
 ✓ src/features/registry/RegistryView.test.tsx (4 tests)
 Test Files  7 passed (7)   Tests  22 passed (22)
# tsc --noEmit: clean
```

Regression: every host `--test` binary passes (70 host integration tests), `cargo build --workspace`
green, `cargo fmt --check` clean, `scripts/check-file-size.sh` → all 211 source files ≤400 lines.

Totals after this slice: **145 Rust + 22 Vitest + 2 shell** green (was 124 + 18 + 2 at S6; +21 Rust
[10 `lb-registry` unit + 11 host registry], +4 Vitest).

## Debugging

None — nothing broke that warranted a `debugging/` entry. One minor test-authoring snag (picking a
deterministic off-curve byte fill for the malformed-key test) was resolved by probing dalek's
`VerifyingKey::from_bytes` for a value it rejects (`[0x02; 32]`); recorded here, not a product bug.

## Public / scope updates

- Promoted to [public/registry/registry.md](../../public/registry/registry.md) (the shipped truth) and
  added the S7 registry row to [public/SCOPE.md](../../public/SCOPE.md).
- Resolved the scope's "public catalog storage" open question to workspace-namespaced entries for this
  slice; refreshed the remaining open questions (publisher-key custody, rotation/revocation, cache GC,
  `registry.update` semantics, the pull-vs-publish outbox boundary) as the S7 follow-ups.

## Dead ends / surprises

- `[0xff; 32]` and `[0x01; 32]` are *valid* Ed25519 point encodings; only some fills are off-curve.
  `VerifyingKey::from_bytes` does validate, so the boundary-rejection claim holds — the test just needed
  a genuinely off-curve constant.
- `sha2` was used transitively but not declared in `[workspace.dependencies]`; added it.

## Follow-ups

- Open questions pushed to the scope doc: real HTTP `Source`/`registry-host` server; durable
  publisher-key allow-list + the admin trust-management flow; key rotation/revocation (needs the hub
  identity directory); cache eviction/GC (keep current + prior version); `registry.update` = install of
  a newer version with new caps re-approved; the public catalog read-only union.
- Gateway/Tauri wiring for `registry_*` (mirrors the S3 channel transport swap), like the S4/S5/S6 verbs.
- The next S7 slices: native Tier-2 supervisor; package the S6 workflow/github-bridge as installed wasm
  artifacts (now that the registry exists to install them through).
- STATUS.md updated: the registry slice is **shipped**; the S7 exit gate's first half (install from
  signed registry · run offline once cached · roll back) is **MET**; the native-sidecar half remains.
