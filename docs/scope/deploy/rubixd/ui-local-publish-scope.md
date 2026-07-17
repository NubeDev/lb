# rubixd scope — UI local publish (upload a package from the browser)

Status: scope (the ask). Slice 9 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

An **Upload page in the embedded UI** that lets an operator publish a package — say a
freshly-built `rubix-ai` binary — to a standalone box from a browser, instead of shelling
in to run `rubixd publish`. It is a **thin lens over slice 8's `POST /packages`**: the
same streaming multipart, the same verify-before-store, the same `pkg_local` row. The UI
adds **zero** server verbs (the slice-7 contract), and it opens **no** unsigned path — a
package that would be refused at the CLI is refused identically in the browser.

The ask behind this scope was "let me upload the rubix-ai binary and install it from the
UI". The honest version of that is two steps, and this scope keeps them separate:
**publish** (get a verified artifact into the local index — this slice) and **install**
(a bundle referencing it reconciles — already slices 3/4/6/8). The Upload page ends by
handing the operator to the second step; it does not fuse them.

## Goals

- **Upload page** (`/upload`, `upload.html` + the existing `app.js` helpers) — pick or
  drag **both** parts of the signed envelope: the metadata TOML and the artifact blob.
  It `POST /packages` with a streaming multipart body, shows progress, and renders the
  outcome verbatim (200 published · 200 idempotent no-op · 422 verify failed · 401).
- **Signed-only, no bypass.** The page requires the metadata TOML that carries the
  Ed25519 signature. Dropping a bare binary is a **client-side refusal** with the exact
  command to fix it — the page never invents metadata, never sends an unsigned body, and
  there is no `allow_unsigned` flag in this slice (see "Rejected alternatives").
- **Streaming upload, bounded memory** — the body is a `FormData` of `File` handles, so
  the browser streams from disk and the multi-hundred-MB blob never sits in a JS string.
  The matching server path already streams to a temp file (slice 8).
- **Post-publish continuity** — on success the page shows the published
  (name, version, arch, digest) and links to the two things the operator actually wants
  next: the **Packages** list and the bundle that would install it. It states plainly
  that publishing does **not** install.
- **Packages page** (`/packages`) — the local index (name, version, arch, digest, size),
  the browser mirror of `rubixd packages`. Read-only; it is the "did my upload land?"
  answer and the natural home for the upload entry point.
- **The 401/423 contract** the shipped UI already implements applies unchanged: a dead or
  stale token drops and bounces to `/claim`.

## Non-goals

- **No unsigned upload, no browser-side signing, no key handling in JS.** Private key
  material never enters the page. (Both were considered and rejected — see below.)
- **No "install this now" button.** Publishing puts an artifact in the local index;
  installing is a bundle edit + reconcile. Fusing them would invent a server verb
  (`POST /packages?install=true`) that neither slice 8 nor the CLI has, and would let one
  click both trust *and* run new code. Kept separate deliberately.
- **No bundle editing here** — the Apply page (bundle YAML → validation) is slice 7's
  open item and stays its own surface.
- **No blob GC / retention UI** — slice 8 defers retention; the UI cannot lead it.
- **No re-export** — this box accepts pushes for *itself*; it does not serve peers
  (parent's fleet-control-plane follow-up).
- **No new auth model** — the slice-2 Bearer path verbatim.

## Intent / approach

- Files: `crates/rubixd/ui/upload.html`, `crates/rubixd/ui/packages.html`, two route
  handlers added to the existing `src/server/ui.rs` (static-asset serving only), and
  helper additions to `ui/app.js` (a multipart POST + progress). One responsibility per
  file (FILE-LAYOUT); the UI stays framework-free, npm-free, vendored-only.
- **The page is a lens, not an authority.** Every byte it uploads goes through the same
  `POST /packages` a curl or `rubixd publish` uses, and every refusal comes from the
  server's verifier. The UI's client-side "you forgot the .toml" check is a *courtesy*,
  not a gate — the server would refuse the same body anyway. That ordering matters: the
  UI must never be the only thing standing between a bad blob and the cache.
- **Why `XMLHttpRequest` and not `fetch`** for the POST: upload progress. `fetch` has no
  upload-progress event; `xhr.upload.onprogress` does, and a multi-hundred-MB publish
  over a slow LAN with no progress bar reads as a hang. This is the one place the UI
  reaches past `fetch`; it is worth it and it is contained to one helper.
- Alternative rejected — **unsigned dev-mode upload** (`allow_unsigned_local_upload`):
  the parent scope's line is *"a blob whose digest or signature fails verification is
  never executed or loaded, period"*. A default-false flag is still a flag: it lives one
  typo from a prod box, and what it admits is not data but **executable code** that
  systemd will run as a service. The dev convenience it buys is already bought by
  `rubix-sign` with a throwaway dev key in `trusted_pubkeys` — a signed dev build is a
  one-line ergonomic difference and keeps exactly one trust path in the codebase.
- Alternative rejected — **sign in the browser** (paste a private key, sign in JS): it
  drags Ed25519 key material into the page, contradicts the "our own `app.js` only" XSS
  posture the token scope demands, and makes the browser a key-handling surface on a box
  whose whole security story is "loopback + one token". Signing stays where the key
  already lives: the operator's machine.
- Alternative rejected — **fold into slice 7**: slice 7 is shipped, and its exit gate is
  literally "no new server verbs added for the UI". This page is inert until slice 8
  lands `POST /packages`, so it cannot precede it.

## How it fits the core

Not an lb node (the parent records the translation). The relevant walls:

- **Trust:** unchanged and re-asserted. Verify-before-store is the server's; the UI adds
  no path around it. The scope's deny tests (below) prove the browser cannot do what the
  CLI cannot.
- **Capabilities / auth:** the slice-2 Bearer path. Unauthenticated `POST /packages` →
  401 whether the body came from curl or a browser. Admin-only in v1 (slice 8's
  recommendation); if the agent-token `publish` grant lands, this page inherits it with
  no change.
- **Placement:** local-only, loopback by default — the same posture as the rest of the UI.
- **Data (SurrealDB):** none new. Reads/writes exactly what slice 8 defines (`pkg_local`
  + the content-addressed `blobs/` cache).
- **One responsibility per file:** one page per file, one verb per handler.
- **MCP surface:** N/A — rubixd is not an lb node and exposes no MCP tools; the REST
  surface is its contract. **API shape:** no new verbs. The page uses slice 8's write
  (`POST /packages`) and slice 8's read (the local index behind `/packages`); the live
  feed and batch shapes are N/A (one artifact at a time, and the publish is a single
  bounded call whose progress the browser already reports).
- **Skill doc:** the drivable surface here is REST and already belongs to
  `skills/rest-claim-auth/SKILL.md` (auth) and slice 8's publish skill. This slice adds
  **no new agent-drivable verb** — an agent publishes with `rubixd publish` or a raw
  `POST /packages`, never by driving a browser. The implementing session extends the
  **repo's** `.claude/skills/verify/SKILL.md` with the upload flow instead (it already
  documents driving the UI in headless Chrome).

## Example flow

1. Operator builds `rubix-ai` 0.4.6 for the box's arch and signs it on their own machine
   with a key that is in the box's `trusted_pubkeys`, producing
   `rubix-ai-0.4.6` + `rubix-ai-0.4.6.toml`.
2. They open `http://<box>:9420/upload` (claiming or pasting their token first, as usual)
   and drop **both** files. The page shows the parsed name/version/arch from the TOML
   before sending, so a wrong-arch build is caught by eye.
3. Upload streams; a progress bar runs; rubixd verifies SHA-256 + Ed25519 **before**
   anything is committed, then commits `blobs/<sha256>` and upserts `pkg_local`.
4. The page shows **published** with the digest, and says: *this is now in the local
   index — it is not installed.* It links to `/packages` and to the bundle that pins it.
5. The operator pins `rubix-ai: "0.4.6"` in the bundle and reconciles; the systemd backend
   installs it and health-gates it to `active` (slices 3/4).
6. Re-dropping the same pair → **200, no-op** ("already published, identical digest").
   Dropping a tampered blob → **422**, and `/packages` is unchanged.
7. Dropping the bare binary with no TOML → the page refuses client-side with the
   `rubix-sign` command, and sends nothing.

## Testing plan

Per `testing-scope.md` — no mocks; real embedded store, real HTTP server, real signing,
real headless browser (the repo's verify skill already drives Chrome over CDP).

- **The deny/trust category (mandatory), driven from the browser**: a tampered blob, an
  unsigned blob, and a blob signed by a key **not** in `trusted_pubkeys` each → 422 with
  the error rendered verbatim; assert `blobs/` and `pkg_local` are **unchanged** after
  each. This is the load-bearing test of the slice: *the browser cannot do what the CLI
  cannot.*
- **Unauthenticated / stale token**: `POST /packages` with no bearer → 401; the page with
  a stale token (post-`reset-token`, so the box answers **423**) drops the token and
  bounces to `/claim` — the shipped 401/423 handling, re-asserted on the new page.
- **Surface parity (the exit gate)**: the same (metadata, blob) pair published via the
  browser and via `rubixd publish` produces an **identical** `pkg_local` row and an
  identical blob digest on disk. If these can diverge, the page is not a lens.
- **Route-table snapshot**: the UI added no new server verbs — `/upload` and `/packages`
  serve static assets only (the slice-7 test, extended).
- **Idempotence from the browser**: re-uploading the same pair → 200 no-op, one row, one
  blob, no rewrite.
- **Streaming / bounded memory**: a multi-hundred-MB blob uploads from the browser
  without an OOM on either side, and the progress bar advances (the reason for XHR).
- **Offline guarantee (slice-7 test, extended)**: no `http://`/`https://` URL in any newly
  embedded asset.
- **XSS**: package names/versions come from an operator-supplied TOML and render into the
  index table — the shipped `esc()` path covers it; seed a package whose name carries an
  HTML payload and assert it renders inert (the repo's verify skill already does this for
  bundle names).

## Risks & hard problems

- **The "just let me upload the binary" pressure is the whole risk.** The ask is
  ergonomic and the trust wall is the only thing standing against it. Every future
  request to soften this ("just for dev", "just on loopback", "just behind a flag") is
  the same request. The answer is a signed dev key in `trusted_pubkeys`, which costs one
  line and keeps one trust path. If this scope ever grows an unsigned branch, the deny
  tests above are what should fail first — keep them.
- **Two-file uploads are an ergonomic trap.** Operators will drop the binary alone and
  read the refusal as a bug. Mitigation: the page names both required files up front,
  and the refusal text is the literal `rubix-sign` command — not "invalid request".
- **Publish ≠ install is a genuine surprise.** The ask conflates them, so the UI must say
  so at the moment of success, not in a doc. A green "published" that leaves the box
  unchanged will otherwise read as a no-op.
- **Progress lies on fast loopback** — the browser will report 100% while the server is
  still hashing/verifying a large blob. The page must show a distinct "verifying…" state
  after upload completes, or a 10-second verify reads as a hang.
- **Multipart field order matters**: slice 8 parses `metadata` before streaming `blob`;
  the page must append the TOML to the `FormData` **first**. A silent reorder (or a
  library that sorts fields) would break streaming. Assert it in the parity test.

## Open questions

- **Arch mismatch: warn or refuse?** The TOML carries `arch`; the box knows its own. A
  wrong-arch publish is legal (a fleet box may cache for a peer — though we do not serve
  peers today) but is almost always a mistake on a standalone box. Recommendation: the
  page shows the parsed arch next to the box's arch and warns loudly before sending;
  refusing belongs to the server or nowhere, not to a page that is meant to be a lens.
- **Does `/packages` need the blob size and mtime?** Slice 8's CLI lists size. Cheap to
  include; recommendation: yes, plus published-at, since "did my upload land, and which
  one is newest" is the page's whole job.
- **`rubix-sign` is named here but not scoped anywhere.** Slice 8 assumes a signed
  envelope arrives and the parent describes the signing pattern, but no doc owns the
  operator-side signing tool. Recommendation: a small follow-up scope (or a section in
  slice 8's session) — this page's error message is useless if the command it names does
  not exist.

## Decisions resolved in implementation

These resolve the open questions above and the three unstated seams the scope left for
the implementing session. All three keep the trust wall intact and the page a lens.

- **The signed metadata envelope format** (the unstated seam behind "pick both parts of
  the signed envelope"). Slice 8's `POST /packages` takes **five** multipart fields
  (`metadata`, `blob`, `digest_hex`, `publisher_key_id`, `signature`); the page must send
  all five but the operator only drops **two** files (the TOML and the blob), and the page
  refuses to sign in the browser (rejected alternative, above). Resolution: the TOML the
  operator drops is a **signed metadata envelope** — the package metadata plus a trailing
  `[publish]` table carrying `digest_hex`, `publisher_key_id`, `signature`. The digest is
  computed over the metadata **without** the `[publish]` block (the block carries the
  signature, so including it would be circular), and the browser strips the block
  client-side before sending (simple text split on the `[publish]` header — TOML reserves
  table headers, so it cannot collide with package metadata). The server sees byte-for-byte
  what `rubixd publish` would have sent; the parity test asserts it.
- **`rubixd sign <metadata.toml> <blob> --signing-key <key> --key-id <id> --out <envelope.toml>`**
  (the `rubix-sign` open question). Slice 9 ships a `sign` CLI verb (sibling to `publish`)
  that produces the envelope file above using slice 8's existing `fleet_spec::signing::sign`
  — no new crypto, no POST. The page's "you forgot the .toml" refusal names `rubixd sign`
  verbatim (not the un-implemented `rubix-sign`). `publish` keeps its sign-and-POST shape;
  `sign` is the file-emitting twin for the browser path.
- **`GET /api/packages`** (the one necessary deviation from the slice-7 "no new server
  verbs" exit gate). The packages page mirrors `rubixd packages`, but slice 8 shipped only
  the `POST /packages` write — no read endpoint exists, and the page cannot render without
  one. Resolution: slice 9 adds `GET /api/packages` as a Bearer-gated, read-only JSON route
  returning the local index (the same rows `rubixd packages` prints). It is **generic** —
  not UI-specific — so any client (curl, monitoring) can read it; `/packages` (the HTML
  page) and `/api/packages` (the data) follow the existing `/status` + `/api/status`
  shape. This is the smallest read that satisfies the page, recorded here as a deliberate,
  documented exception to the slice-7 gate (the alternative — no packages page — violates
  a hard scope requirement). `/upload` and `/packages` themselves stay static-asset routes
  (OPEN) per the slice-7 contract; only `/api/packages` is gated.
- **Arch mismatch: N/A.** `fleet_spec::package::Package` carries no `arch` field; the
  server stamps `std::env::consts::ARCH` at publish time. There is nothing in the TOML to
  mismatch against, so the warn/refuse question dissolves. The page surfaces the server's
  `arch` in the published-outcome card (the server's own stamp, not a client claim).
- **`/packages` columns: yes, include size + published-at** (the second open question).
  Cheap, and "which one is newest" is the page's job.

## Related

[`README.md`](README.md) roadmap · [`local-publish-scope.md`](local-publish-scope.md)
(slice 8 — `POST /packages`, the blob cache, `pkg_local`, verify-before-store: **this
slice is inert without it**) · [`embedded-ui-scope.md`](embedded-ui-scope.md) (slice 7 —
the UI this extends, and the "no new server verbs" contract) ·
[`token-auth-scope.md`](token-auth-scope.md) (the Bearer/claim path, and the 401/423
states the page inherits) · [`bundles-scope.md`](bundles-scope.md) (slice 6 — the
resolver a published package resolves through) ·
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md) (the trust wall this
scope refuses to bypass) · `skills/rest-claim-auth/SKILL.md` (the auth contract) ·
rubix-fleet `.claude/skills/verify/SKILL.md` (driving the UI in headless Chrome).
