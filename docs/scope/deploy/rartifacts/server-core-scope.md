# rartifacts scope — host + extension core (node, records, blobs, read API)

Status: scope (the ask). Slice 1 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md) (owns the
built-on-lb decision).

The load-bearing base: a **product-host binary embedding `lb-node`** (the
ems/rubix-ai pattern) plus the **native (Tier-2) `rartifacts` extension** that owns
all package logic — records in the node's SurrealDB, the content-addressed blob dir
on disk, the first `pkg.*` read tools, and the host-mounted REST projection that
rubixd's wire contract rides.

## Goals

- **Host** (`rartifacts/host/`, binary `rartifacts`): a thin boot shim exactly like
  `ems-node` — `main.rs` → `boot.rs` filling `BootConfig` from `RARTIFACTS_*` env
  (`HOME`, `STORE_PATH`, `GATEWAY_ADDR` default `0.0.0.0:9410`, `EXT_UI_DIR`,
  `SIGNING_KEY`, `WORKSPACE` default `fleet`), `lb_node::boot_full()`, then mounts
  the extra routes (the ems `ems_mount.rs` seam). Boot **self-publishes** the in-repo
  extension (build + signed-artifact publish, the ems `make pack`/publish flow
  automated at boot for the dev loop; release images bake the published artifact).
- **Extension** (`rartifacts/extensions/rartifacts/`, native tier): `extension.toml`
  requesting exactly the caps it needs (its own `pkg.*` tools; `store:pkg*:write`
  per-table grants; the blob dir path in `[native]` config); depends on the published
  SDKs only (`lb-ext-native` facade), never on lb crates directly.
- **Records** (workspace-walled, written via the host callback): `pkg` (name, owner,
  `visibility: public|private` default private), `pkg_artifact` ((name, version,
  arch), kind, sha256, size, signature, publisher_key_id, config schema, health spec,
  `yanked`), `pkg_channel` ((name, channel) → version), `pkg_event` (append-only
  audit rows).
- **Blob store**: `<RARTIFACTS_HOME>/blobs/<sha256>`, **owned by the native
  extension** (the sanctioned native-tier external resource — multi-GB archives are
  not store records): streamed writes to temp with hash-on-the-way-through, refused
  on digest mismatch, atomic rename; write-once, dedup free. Invariant: a
  `pkg_artifact` record is written **only after** its blob is durable; a startup
  integrity pass reports orphan blobs / missing blobs.
- **Tools**: `pkg.list` (keyset-paged, the lb cursor convention) and `pkg.get` —
  capability-gated MCP tools like any extension's (resource-verbs grammar).
- **Host-mounted read routes** — the plain-REST wire contract rubixd depends on:
  `GET /packages`, `GET /packages/{name}` (and `GET /health`, open). Thin
  projections: parse → `lb_mcp::call("pkg.…")` under the caller's principal → JSON.
  No logic in the route layer, ever — the tool is the single implementation.

## Non-goals

- No auth beyond what lb boots with (slice 2 adds claim/api-keys/anonymous), no
  publish (3), no resolution semantics/downloads (4), no UI (5). No S3/object store
  (the blob module boundary is where it would slot). No second HTTP server — the
  mounted routes live on the node's gateway.

## Intent / approach

Everything the standalone design hand-rolled — identity, store, caps, audit,
UI-serving — comes from the node; this slice's *own* code is just the extension's
domain logic + a blob module + thin route projections. The host stays a boot shim
with zero package logic (the ems discipline: domain 100% in the extension, reached
only via MCP). Alternative rejected: implementing `pkg.*` in the host binary — it
would bypass the capability wall and turn the host into a second core; the extension
seam is the whole point of the lb posture.

## How it fits the core

This *is* an lb node, so the platform checklist applies for real: workspace `fleet`
walls every record (isolation test mandatory); every tool capability-gated (deny test
mandatory); SurrealDB only for state; blobs are the native-tier escape hatch,
documented in the manifest; MCP is the contract (REST routes are projections); no
core crate is touched (rule 10 — the host names its own extension, which is the
allowed application-service pattern, reached only through generic seams).

## Example flow

1. `RARTIFACTS_HOME=/var/lib/rartifacts rartifacts` boots: workspace `fleet` seeded,
   extension published + running, gateway on `:9410`.
2. A test seeds a package via the extension's tools; `pkg.list` over MCP and
   `GET /packages` over REST return the same row.
3. The blob module round-trips a 100 MB file; kill-between-blob-and-record leaves an
   orphan the startup pass reports; the record never exists without its blob.

## Testing plan

A real spawned rartifacts node (embedded lb, mem or on-disk store — the lb
`test_gateway` discipline), no mocks:

- **capability-deny**: a principal without the `pkg.list` grant → 403 over MCP and
  the REST projection alike.
- **workspace-isolation**: `pkg` rows seeded in a second workspace are invisible to
  `fleet` callers.
- blob round-trip byte-identical (streamed, hash asserted both sides); declared-digest
  mismatch refused with temp cleaned; record-after-blob ordering under a kill hook;
  startup integrity report.
- keyset pagination on `pkg.list`; unknown name → typed 404; restart persistence.

## Risks & hard problems

- Boot self-publish of the extension (build→sign→install at boot) is new glue —
  keep it dev-mode only; release artifacts ship pre-published (the docker image
  bakes the store seed), or first-boot installs from the bundled artifact file.
- Native-ext blob ownership + gateway streaming (slice 4) must not copy through the
  MCP body path — the host route streams from disk directly after a tool-mediated
  authorization; design that seam now (tool returns the authorized path/digest, host
  streams).
- lb tag pin churn (the rubix-ai cadence) — accepted in the parent scope.

## Open questions

- `pkg_event` vs lb's audit ledger (`audit/` scope, unshipped): keep `pkg_event` now,
  fold into the platform ledger when it ships? Recommendation: yes, keep + fold later.

## Related

[`token-auth-scope.md`](token-auth-scope.md) (next) · parent scope (posture + what lb
buys) · `ems` (`rust/node/src/{boot.rs,ems_mount.rs}` — the pattern) · lb
`docs/scope/extensions/reference-extensions-scope.md` (native-tier doctrine) · lb
`docs/scope/datasources/page-chaining-scope.md` (cursor convention).
