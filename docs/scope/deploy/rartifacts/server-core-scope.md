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
- **`GET /health`** (open — the fleet health contract, decided in
  [`../containerize-scope.md`](../containerize-scope.md) §The health contract): `200
  {"status":"ok","version":…,"detail":{"store":…,"blobs":…,"ext":…}}` when the store is
  open, the blob dir is writable, and the extension is published; `503
  {"status":"degraded",…}` otherwise. **`/health`, never `/healthz`** — one route, no
  `/livez`/`/readyz` (503-vs-refused *is* the liveness/readiness split). Reads **in-memory
  state only** — no store query, no disk I/O — and **never blocks on a dependency**. This
  is the ALB target-group health check and the container probe; `detail` names which
  subsystem is down, never a path, DSN, or key.

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
- `GET /health`: 200 + `{"status":"ok"}` on a booted node, unauthenticated; **503 +
  `{"status":"degraded"}` when the blob dir is unwritable** (chmod it in the test — the
  process still answers, which is what distinguishes "de-register me" from "restart me");
  no path/DSN/key material in any body.

## Risks & hard problems

- Boot self-publish of the extension (build→sign→install at boot) is new glue —
  keep it dev-mode only; release artifacts ship pre-published (the docker image
  bakes the store seed), or first-boot installs from the bundled artifact file.
- Native-ext blob ownership + gateway streaming (slice 4) must not copy through the
  MCP body path — the host route streams from disk directly after a tool-mediated
  authorization; design that seam now (tool returns the authorized path/digest, host
  streams).
- lb tag pin churn (the rubix-ai cadence) — accepted in the parent scope.

## Decisions (no open questions)

- **`pkg_event` vs lb's audit ledger — decided: keep `pkg_event` now, fold later.** The
  `audit/` scope is unshipped; blocking the artifact plane's audit trail on it would trade
  a working append-only table for a dependency with no date. `pkg_event` is small and
  purpose-shaped, and folding it into the platform ledger when that ships is a migration,
  not a redesign. *Reopen when*: `audit/` ships — at which point `pkg_event` becomes a
  view over it rather than a table.
- **Boot self-publish of the extension — decided: dev-mode only, release images bake the
  published artifact** (already stated in §Risks; recorded here as the decision it is).
  This is the rule that keeps a Rust toolchain and a signing key out of the runtime image —
  see [`../containerize-scope.md`](../containerize-scope.md), which depends on it.

## Related

[`token-auth-scope.md`](token-auth-scope.md) (next) · parent scope (posture + what lb
buys) · [`../containerize-scope.md`](../containerize-scope.md) (this server's container
image — the `/health` body contract above, the `/data` volume for `store/` + `blobs/`, and
the AWS topology; it lands **with this slice**) · `ems`
(`rust/node/src/{boot.rs,ems_mount.rs}` — the pattern) · lb
`docs/scope/extensions/reference-extensions-scope.md` (native-tier doctrine) · lb
`docs/scope/datasources/page-chaining-scope.md` (cursor convention).

## Decisions resolved in implementation (2026-07-19)

Slice 1 shipped in `rubix-fleet`. Session doc:
`docs/sessions/deploy/rartifacts-server-core-session.md`; manual runbook:
`docs/testing/rartifacts-slice1-runbook.md`. These resolve the seams this scope left
open, and correct four places where the scope described something the platform does not
actually do. Everything below was found by running against a real booted node, not read
off documentation.

### Corrections to this scope

- **Tools cannot be named `pkg.*` — the wire names are `rartifacts.list` /
  `rartifacts.get`.** lb resolves a qualified tool by splitting on the FIRST `.`: the
  prefix is the extension id. `pkg.list` makes the host look for an extension called
  `pkg`, which does not exist, so every call fails "no such tool" regardless of the
  manifest or the grants. The `pkg.*` spelling above is the conceptual verb set; the
  routing name is `<ext_id>.<verb>`. **This changes the capability strings the later
  slices must grant**: `mcp:rartifacts.list:call`, never `mcp:pkg.list:call`.
- **`GET /rartifacts/health`, not `GET /health`.** lb's gateway already registers an
  unauthenticated `/health` at `node-v0.4.7`, and axum panics at startup on a duplicate
  path. The package-plane probe is mounted alongside it rather than shadowing it; the two
  answer different questions ("is the node up?" vs "is the package plane serving?"), and
  an ALB target group can point at either. Folding them into one route needs an lb-side
  seam to extend `/health`'s `detail`, which does not exist at this tag. *Reopen when*:
  lb ships that seam.
- **The health probe does one `stat`; it is not pure in-memory state.** The scope says
  "reads in-memory state only — no disk I/O". Store and extension status genuinely are
  in-memory flags, but the blob answer is one local `access(2)`-class check — which is
  precisely what makes "the volume went read-only", the condition this scope wants a 503
  for, detectable at all. A local stat cannot block on a network dependency (the property
  the rule exists to protect), but it IS disk I/O, so the code's doc states the narrower
  claim rather than this scope's phrasing. Consequence worth knowing: a FULL disk is not
  detected — permissions are writable, so the probe reports healthy and the failure
  surfaces on the publish path instead.
- **No boot self-publish; `install_native` directly.** The scope wants build→sign→publish
  at boot for the dev loop. The implementation uses `install_native` + `OsLauncher` (the
  ems `ems_mount.rs` pattern), the same path lb's own role mounts use. It needs no signing
  key in the process and no build step at boot, which makes the §Risks concern ("keep it
  dev-mode only") moot rather than merely managed. The "release images bake the published
  artifact" decision is unaffected.

### Seams the scope left open

- **The `pkg_artifact` record id is `package|version|arch`, `|`-separated.** The id must
  be injective or one release silently overwrites another. Semver permits `.`, `-` and
  `+`; package names permit `-` and `_`; so each of those admits a collision. `|` is in
  none of the three alphabets. The assumption is ENFORCED rather than trusted
  (`record::is_valid_component`): a `|` inside a component would let a publisher forge
  another release's id, which is a record-overwrite primitive.
- **A channel names a VERSION, not an artifact triple.** Promotion is a statement about a
  release, and a release spans architectures — an arch-keyed channel would make promotion
  a per-arch action and let a fleet split-brain across architectures after a partial
  promotion.
- **`pkg.list` filters AFTER paging, so a page may be short or empty while more pages
  remain — `next_cursor` is the only end-of-list signal.** Filtering before paging would
  require the visibility rule to live inside a SQL string built next to caller-supplied
  values; keeping it in one unit-tested Rust function is worth the client-side awkwardness.
  The cursor advances on EVERY row fetched, including hidden and unparseable ones, using
  the store's record id rather than anything read out of the record body — otherwise a
  fully-hidden page never advances (infinite loop) and a corrupt row wedges pagination
  permanently.
- **Its own cargo workspace, not a root member.** The lb git deps pull ~1000 crates;
  making them a root member would put a multi-minute build on the critical path of every
  `cargo test --workspace` the rubixd track runs, and would force one dependency
  resolution across two worlds that pin `surrealdb` differently. Cost, recorded as debt:
  CI must run `cd rartifacts && cargo test` explicitly.

### Platform behaviours the next slices will hit

Documented because each cost real debugging time and none is discoverable from the docs:

1. **The host does NOT strip the `<ext>.` prefix** before dispatching to a native child,
   though the SDK's `Tools::call` doc says it does. The child receives the qualified name
   at `node-v0.4.7`; the dispatcher accepts both spellings, stripping only its own prefix.
2. **`SELECT *` fails on any NON-EMPTY table.** SurrealDB's `id` serializes as a tagged
   enum and the `store.query` bridge rejects that shape — so reads 502 against a populated
   table while an empty one works perfectly. Every query must
   `SELECT *, meta::id(id) AS rid OMIT id FROM …`. The omission must be in the QUERY (the
   failure is host-side while decoding); the alias must be `rid`, because
   `SELECT * OMIT id, … AS id` does not parse — `OMIT` follows the field list.
3. **`store.write` wraps records in `{ data, rev }`.** Reads must unwrap it, or the verb
   layer reports "missing field `name`" for a record whose name is present.
4. **lb pulls zenoh, which rejects tokio's current-thread scheduler** — integration tests
   need `#[tokio::test(flavor = "multi_thread")]`, and a server spawned inside a test's
   runtime dies with it (making failures order-dependent), so a shared fixture must own a
   dedicated thread and runtime.

### Claims deliberately NOT made in the code's docs

Per the slice-11 lesson that a doc asserting a property the code lacks IS the
vulnerability:

- **`pkg_event` is append-only by construction, NOT tamper-proof.** No verb updates or
  deletes an event, but the table sits behind the same `store:pkg_event:write` capability
  as everything else, so a holder could overwrite a row by id. Real tamper-evidence needs
  a hash-chain or a create-but-not-update action; lb's store surface offers neither. This
  strengthens rather than changes the scope's "fold into the platform ledger later"
  decision.
- **Visibility is not yet per-principal.** Slice 1 enforces exactly two things: private
  packages are invisible to the ANONYMOUS tier, and a hidden package is indistinguishable
  from a nonexistent one (same variant, same message, same HTTP body). It does NOT narrow
  what one authenticated principal sees relative to another — that is meaningless until
  slice 2's principals exist.
- **The anonymous principal is a host-side `Principal::for_key`, not a store-backed
  identity.** Not revocable, no api-key record, no audit presence. Acceptable only because
  it holds exactly `mcp:rartifacts.list:call` + `mcp:rartifacts.get:call`. **Slice 2 must
  replace it** with the boot-minted anonymous principal this track's token-auth scope
  specifies.
- **Blob crash-durability is narrower than "survives power loss".** Data is fsynced and
  the rename is atomic, so a blob cannot have the right name and truncated content. The
  parent DIRECTORY is not fsynced, so power loss just after a rename can lose the rename —
  the safe direction (the integrity pass reports it; the record was never written). A
  deliberate durability/throughput trade, not an oversight.
- **The startup integrity pass REPORTS, never deletes.** At boot the server cannot
  distinguish "orphan from last week's crash" from "blob committed 30ms ago whose record
  write is in flight"; deleting on that ambiguity would destroy a live publish.
