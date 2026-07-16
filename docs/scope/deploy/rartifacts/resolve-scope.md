# rartifacts scope — resolution, channels, downloads

Status: scope (the ask). Slice 4 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

The read path rubixd's poller/fetcher lives on: **`pkg.resolve`** maps
`(name, version-spec, arch)` to one concrete artifact — exact pin, semver range, or
**channel** (`stable`, `beta`, … — movable pointers) — and the host-mounted blob
route streams downloads with HTTP Range support. Anonymous for `public` packages,
agent api-key for `private`.

## Goals

- **`pkg.resolve`** (tool; wire projection `GET /packages/{name}/{version}?arch=`):
  `{version}` is an exact semver, a range (`>=0.4, <0.5` — the `semver` crate's
  grammar), or a channel name. Returns the artifact record (version, arch, sha256,
  size, signature, publisher_key_id, config schema, health spec) + `blob_url`.
  Excludes yanked; **arch-strict** (an armv7 box never gets an aarch64 blob "close
  enough"). Visibility enforced in-tool: anonymous principal reaches `public` only
  (same-401 no-leak for private).
- **Channels**: `pkg.promote {name, channel, version}` (owner publisher or admin;
  wire `POST /packages/{name}/channels/{channel}`): promote/demote — pointers move
  both directions on purpose (incident demotes); optional `reason` stored;
  promote-of-yanked → `422`; every move is a `pkg_event` row (rollout forensics).
  Channel names free-form; `stable` is the convention rubixd defaults to.
- **Downloads**: host route `GET /blobs/{sha256}` streaming **directly from the
  ext-owned blob dir** after tool-mediated authorization (the tool authorizes
  digest + visibility under the caller's principal; the host streams the file — no
  copy through MCP). `Range`/`Accept-Ranges` (resume for multi-GB archives),
  `ETag = digest`, `If-None-Match` → `304`. A blob referenced by any public artifact
  is anonymously fetchable; private-only blobs need an agent key (same bytes —
  public wins).
- Resolution is **deterministic and total**: same catalog + same query → same
  answer; no match → `404` with a machine-readable reason (`no-such-package`,
  `no-version-in-range`, `no-artifact-for-arch`, `all-yanked`) — rubixd surfaces
  these verbatim in `status`.

## Non-goals

- No delta/patch downloads, no mirror/replica protocol, no client-side version-list
  choosing (the server resolves to exactly one artifact — thin agents, fat server).
  No bundle-level "release trains" v1.

## Intent / approach

Resolution is one pure function over the artifact set (`resolve.rs` in the
extension), unit-tested as a truth table; the MCP tool and the wire route are thin
shells over it — the UI (slice 5) and any future fleet console call the same tool.
Range serving via `tower-http`'s tested machinery on the host route. Alternative
rejected: client-side resolution in rubixd — N agent versions × resolution bugs is
the apt/npm lesson; keep policy on the server.

## How it fits the core

The visibility gate (anonymous ↔ agent api-key, enforced at the capability wall +
in-tool) and arch strictness are the walls. Determinism = the honesty rule: "why did
I get this version" must be answerable from the catalog alone. State (records) in
SurrealDB; the blob stream is the sanctioned native-tier resource read.

## Example flow

1. rubixd at site-alpha polls `GET /packages/rubix-ai/stable?arch=x86_64` with its
   agent key → `0.4.5`.
2. CI publishes 0.4.6 and promotes `stable → 0.4.6` → next poll resolves 0.4.6; the
   fetcher downloads with two Range resumes over bad site wifi; ETag matches the
   digest it verifies locally.
3. 0.4.6 misbehaves → operator demotes `stable → 0.4.5` (reason: "site-alpha
   rollback storm"); after `pkg.yank 0.4.6`, resolution never offers it again.
4. A public demo package resolves + downloads with no token at all.

## Testing plan

Real spawned node + seeded real artifacts:

- resolution truth table (pure fn): exact hit/miss; range picks highest-in-range;
  channel follows pointer; yanked excluded everywhere; arch strict; each 404 reason.
- channel ops: promote, demote(+reason), promote-yanked 422, non-owner publisher
  403, **agent key on promote 403 (capability-deny)**, events recorded.
- downloads: full-body hash equals digest; single + multi Range resume to a
  byte-identical file (kill-and-resume); `If-None-Match` 304.
- visibility: public resolves/downloads anonymously; private → 401 anonymous, 200
  with agent key, 401 after that key is revoked mid-suite.
- cross-crate E2E: a real rubixd fetcher (slice-6 code) against this node — public
  via a token-less remote, private via an agent key — the contract test both tracks'
  CI runs.

## Risks & hard problems

- Concurrent promote + poll: promotes are single-row writes; the list-then-download
  pair tolerates races because rubixd downloads **by digest** — worst case is a
  slightly stale version, never a corrupt mix. State the invariant in code.
- The authorize-in-tool / stream-from-host seam (shared with publish) — one design,
  two users; do not fork it.

## Open questions

- Demote reason mandatory instead of optional? Recommendation: optional, but the UI
  always asks.

## Related

[`publish-scope.md`](publish-scope.md) (what becomes resolvable) ·
[`../rubixd/bundles-scope.md`](../rubixd/bundles-scope.md) (the consuming
fetcher/poller) · [`web-ui-scope.md`](web-ui-scope.md) (promote/demote buttons).
