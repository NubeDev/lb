# rartifacts scope — publish path (upload, verify, immutability)

Status: scope (the ask). Slice 3 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

The authenticated write path: a publisher uploads `(metadata TOML, blob)` through the
host-mounted streaming route; the **`pkg.publish` tool re-hashes and re-verifies
everything itself** — declared digest, Ed25519 signature, the uploading publisher's
registered keys — before anything becomes visible. Released artifacts are
**immutable**: a `(name, version, arch)` can be yanked but never replaced.

## Goals

- **Host route** `POST /packages` (publisher api-key): multipart — part 1 metadata
  TOML, part 2 blob (absent for `docker-image` kind, reference-only). The route
  streams the blob into the extension's blob store (hash-on-the-way-through, temp +
  atomic rename; MCP bodies are not the place for 8 GiB), then calls **`pkg.publish`**
  with the metadata + computed digest under the caller's principal. A tool-side
  failure cleans the staged blob (unless another artifact already references it).
- **`pkg.publish` verifies, trusting nothing declared**: computed digest must equal
  the declared one (`422`); Ed25519 signature over the length-prefixed digest must
  verify against a pubkey **registered to the authenticated publisher** (foreign or
  unknown key → `422`); metadata parses and validates (kind, semver, arch allow-list,
  config schema well-formed, `[health]`/`[preserve]` shapes; `bundle` kind runs the
  shared `fleet-spec` validator — the same code rubixd runs).
- **Immutability + idempotency**: same `(name, version, arch)` + same digest → `200`
  no-op (CI retries free); different digest → `409` — release a new version.
  `pkg.yank {name, version}` (owner or admin) hides from resolution without deleting
  bytes.
- **Ownership**: first publish of a name sets `pkg.owner = publisher`; another
  publisher → `403` (admin can reassign).
- **Visibility**: first publish may set `visibility: public | private` (default
  **private**); changed only by explicit `pkg.set_visibility` (owner or admin) —
  never silently flipped by a re-publish. Public opens *download*; publish stays
  publisher-gated regardless.
- **Audit**: a `pkg_event` row per accepted *and* refused publish/yank/visibility
  change (who, what, digest, verdict) — the UI activity feed + forensics.

## Non-goals

- No blob deletion on yank (append-only store; GC of fully-unreferenced blobs is a
  later admin job). No server-held signing keys — the server verifies, publishers
  sign (CI holds keys). No upload UI here (slice 5 rides this route).

## Intent / approach

Refuse-before-visible: no `pkg_artifact` record until blob durable + every check
green (the slice-1 ordering rule). Verification reuses `fleet-spec`'s digest/signing
module — the same code rubixd uses on download, so publish-accepted ⇒
agent-verifiable by construction. The route/tool split keeps authority in one place:
the route owns *streaming*, the tool owns *every decision* — a caller with raw MCP
access and a pre-staged blob gets identical verdicts. Alternative rejected:
accept-then-scan (async quarantine) — a window where agents can fetch unverified
bytes is the wrong default for a deploy plane.

## How it fits the core

The trust wall, server side, behind the lb capability wall (`mcp:pkg.publish:call`).
Deny paths are the feature: 401 (no key), 403 (agent key / foreign publisher), 409
(digest conflict), 413 (over limit, aborted early), 422 (hash/signature/metadata) —
each typed, each tested. Workspace-walled records; blob dir is the native-tier
resource.

## Example flow

1. CI builds rubix-ai 0.4.6 (x86_64 + aarch64), signs with the `ci` key, publishes
   both with the `publisher:ci` api-key → two artifacts visible.
2. CI retries the x86_64 upload after a network blip → `200` idempotent.
3. A compromised laptop overwrites 0.4.6 with different bytes → `409`; pushes 0.4.7
   signed with an unregistered key → `422`; both in `pkg_event`.
4. 0.4.6 goes bad in the field → `pkg.yank` → resolution stops offering it; blob
   still fetchable by digest for rollback forensics.

## Testing plan

Real spawned node, real multipart uploads, real keys generated in-test:

- happy path per kind (`systemd`, `docker-archive`, `docker-image` metadata-only,
  `bundle` through the shared validator).
- deny matrix: bit-flipped blob 422; declared≠computed digest 422; unregistered key
  422; publisher B onto A's package 403; **agent api-key on publish 403 (the
  capability-deny test)**; oversize 413 with temp cleaned; dup-same-digest 200;
  dup-different-digest 409.
- visibility: default private; set at first publish; `pkg.set_visibility` by owner
  200 / by other publisher 403; re-publish never flips it.
- yank: hidden from default list/resolve, visible with `?yanked=true`, blob still
  fetchable by digest.
- crash between blob and record (test hook) → publish fails, retry succeeds, no
  orphan record; `pkg_event` rows for accept and refuse.

## Risks & hard problems

- Multipart streaming + early abort (metadata part first — a disqualified publish
  must not read 8 GiB before refusing).
- The route→tool handoff for the staged blob (path/digest passed, tool authorizes and
  adopts it) is the trickiest seam of the lb posture — design it in slice 1's blob
  module, prove it here.
- "Versions only move forward" is channel policy (slice 4's promote), not publish
  policy — publishing an older version stays legal; don't smear the check.

## Open questions

- Max blob size default: 8 GiB proposed.
- `docker-image` references: require a pinned registry digest (`image@sha256:…`)?
  Recommendation: yes — tag-only references are a supply-chain hole.

## Related

[`server-core-scope.md`](server-core-scope.md) (blob module + ordering rule) ·
[`token-auth-scope.md`](token-auth-scope.md) (publisher keys) ·
[`resolve-scope.md`](resolve-scope.md) (what becomes offerable) · parent scope
§package model + signing envelope.
