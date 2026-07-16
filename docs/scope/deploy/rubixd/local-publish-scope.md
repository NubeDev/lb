# rubixd scope — standalone local publish (rartifacts-optional)

Status: scope (the ask). Slice 8 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

What makes a single machine **100% independent of any rartifacts server**: rubixd
accepts a signed package **pushed to it directly over REST**, verifies it, caches the
blob content-addressed, and records it in a **local package index** that a bundle
resolves against with **no network**. rartifacts stays the answer for *fleets* (one
publish → N machines); a lone box never needs it.

This slice is the mirror image of rartifacts' publish path (`rartifacts/publish-scope.md`),
built on the same signed-artifact envelope — but landing in rubixd's own tiny local store
instead of a product-node extension. The wire contract (`POST /packages`) is deliberately
identical, so the same CLI/CI tooling publishes to either.

## Goals

- **Content-addressed blob cache** `<data>/blobs/<sha256>` — the single on-disk home for
  every artifact rubixd holds, keyed by digest. Shared by slice 6's remote-download poller
  and by local uploads, so an artifact that arrives both ways is stored **once**. A
  small `blob/` folder-of-verbs: `stage` (stream to temp), `verify`, `commit` (atomic
  temp→rename, the `install_dir.rs` pattern), `path_for`, `has`.
- **Local package index** in the ledger — table `pkg_local`, one row per
  (name, version, arch): parsed `Package` metadata + the blob digest it points at.
  This is the standalone equivalent of a remote's package list; resolution reads it first.
- **`POST /packages`** on the slice-2 axum server — streaming multipart (`metadata`:
  the package TOML; `blob`: the raw artifact). Bearer-gated by `fleet-auth` (admin or an
  agent token with a `publish` grant — same path as every other route, no new auth
  surface). Flow: stream blob → temp → parse+validate metadata → **verify SHA-256 +
  Ed25519 against `trusted_pubkeys` before anything is committed** → `blob/commit` →
  upsert `pkg_local`. Re-publishing an identical (name, version, arch, digest) is
  **idempotent** (200, no rewrite). A digest/signature/known-key failure → 422, temp
  file discarded, **nothing enters the cache or the index**.
- **`rubixd publish <metadata.toml> <blob>` CLI** — a thin wrapper that POSTs to the
  local server (reads the admin token the same way other authed CLI verbs do), so
  hand/CI pushes and the REST path share one code path.
- **Local-first resolution** — extend slice 6's resolver: consult `pkg_local` **before**
  any configured remote (declared order after that). A standalone box with zero remotes
  resolves entirely locally; a fleet box can pin a one-off local build over its channel.
  `remote: local` in a bundle **forces** the local index and errors if the package is
  absent (no silent fall-through to a remote).
- **`rubixd packages`** (CLI) lists the local index (name, version, arch, digest, size).

## Non-goals

- No local channel promotion in v1 (open question in the parent) — local publishes
  support exact/range pins; `stable`-style channels resolve against remotes only for now.
- No blob GC/retention policy here (a follow-up); v1 keeps every committed blob.
- No re-export: a standalone rubixd is not a mini-rartifacts server for *other* agents.
  It accepts pushes for **itself**; serving downloads to peers is the fleet-control-plane
  follow-up, not this slice.
- No new auth model — reuses the slice-2 Bearer/`fleet-auth` path verbatim.

## Intent / approach

- Files: `crates/rubixd/src/blob/{stage,verify,commit,path}.rs`,
  `.../publish/{route.rs (POST handler), local_index.rs (pkg_local upsert/query),
  cli.rs (rubixd publish)}`, and a resolver edit in `reconcile/resolve.rs` to check
  local-first. FILE-LAYOUT rules from the first file (one verb per file, ≤400 lines).
- **Verify-before-store is the load-bearing invariant.** The temp blob is verified while
  still in the temp dir; only a clean artifact is ever renamed into `blobs/`. This is the
  standalone restatement of the parent's trust wall — the same posture whether a blob
  came from a remote download or a local POST.
- Reuse `fleet-spec`'s digest/signing module (ported from lb `registry/src/digest.rs`) —
  the identical verifier the poller uses; no second implementation.
- The POST handler streams to disk (never buffers the whole blob in memory) — multi-GB
  docker archives publish locally the same as over rartifacts.
- Alternative rejected: a CLI-only `rubixd import <file>` with no REST route. Rejected —
  the ask is explicitly *over REST* (CI/operator push without shelling into the box), and
  matching rartifacts' `POST /packages` contract means one publish tool for both targets.

## How it fits the core

Not an lb node (parent records the translation). The wall here is **trust** (signature +
`trusted_pubkeys` verify, verify-before-store) plus the slice-2 Bearer auth
(unauthenticated publish → 401). SurrealDB embedded = the local index; blobs on disk =
the sanctioned large-artifact home (mirrors rartifacts' native-ext blob dir, one tier
down). One responsibility per file.

## Example flow

1. Operator builds `rubix-ai` 0.4.6 for this box's arch, signs it with a key in the
   machine's `trusted_pubkeys`.
2. `rubixd publish rubix-ai-0.4.6.toml rubix-ai-0.4.6` (or CI does the equivalent
   `POST /packages`) — rubixd streams, verifies, commits `blobs/<sha256>`, upserts
   `pkg_local`. No rartifacts anywhere.
3. A bundle pins `rubix-ai: "0.4.6"` (no `remote:`); `rubixd reconcile` resolves it from
   the local index, installs via the systemd backend, health-gates to `active`.
4. Re-running the publish for the same digest → 200, no-op. Pushing a tampered blob → 422,
   cache untouched.

## Testing plan

Per `testing-scope.md` — no mocks; real embedded store, real HTTP server, real signing.

- **Standalone install, zero remotes**: config with no `[[remote]]`; publish a signed
  tiny binary over `POST /packages`; a bundle referencing it reconciles to `active` with
  the network unreachable. Data-dir/ownership invariants from slices 3–4 still hold.
- **Verify-before-store (the deny/trust test, mandatory category)**: tampered blob,
  unsigned blob, and a blob signed by a key **not** in `trusted_pubkeys` each → 422; assert
  `blobs/` and `pkg_local` are unchanged after each. Unauthenticated `POST /packages` → 401.
- **Idempotence**: publish the same (name, version, arch, digest) twice → second is a
  no-op (no rewrite, same row); publishing a *different* digest for an existing
  (name, version, arch) is rejected (immutable version) unless `--replace`.
- **Resolution order (real ledger)**: a local 0.4.6 and a remote 0.4.6 both present →
  local wins; `remote: local` errors when the package is absent locally; a fleet box with
  a local one-off pinned over its channel installs the local build.
- **Streaming**: a large (multi-hundred-MB) blob publishes without an OOM (bounded memory).
- **CLI parity**: `rubixd publish` and a raw multipart `POST /packages` produce identical
  index rows + blob.

## Risks & hard problems

- **Shared blob cache invariants** — the poller (slice 6) and this route both write
  `blobs/`; commits must be atomic and digest-keyed so a concurrent download + local
  publish of the same artifact can't half-write. The `blob/commit` temp→rename verb is the
  single writer; both callers go through it.
- **Metadata trust** — the signature covers (metadata, payload) together (the lb envelope),
  so a client can't swap metadata onto a validly-signed blob; keep the verify over the
  length-prefixed digest of *both*, exactly as the poller does.
- **Immutable versions** — allowing `--replace` on a local index row is convenient for dev
  but breaks the "a version means one artifact" contract; gate it behind an explicit flag
  and never on the plain REST path.

## Open questions

- Local channel promotion (`rubixd promote <pkg> <channel> <version>` against `pkg_local`)
  so channel-pinned bundles work fully offline — parent open question; recommend a small
  follow-up slice rather than smearing it in here.
- Agent-token `publish` grant shape: a dedicated cap vs. admin-only local publish in v1.
  Recommendation: admin-only in v1 (a standalone box has one operator), widen later.

## Related

[`README.md`](README.md) roadmap · parent scope (the "Standalone local publish" goal +
"Local package index" design) · [`token-auth-scope.md`](token-auth-scope.md) (the reused
Bearer path) · [`bundles-scope.md`](bundles-scope.md) (slice 6 — the resolver + blob cache
this extends) · `../rartifacts/publish-scope.md` (the mirror-image server path) · lb
`registry/src/digest.rs` + `host/src/ext/install_dir.rs` (the reused conventions).
