# rubixd scope — delete a package from the local index

Status: **SHIPPED (2026-07-21).** Follow-up to [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md). See
"Decisions resolved in implementation" at the end.

The **Local package index** page (slice 9's `/packages`) shows every artifact the box has
accepted and verified — name, version, arch, digest, size, signer — with an **Install**
button per row. There is no way to take a row **out**. An operator who fat-fingers a bad
build, publishes a wrong-arch binary, or just wants to reclaim disk for an old version has
no path short of hand-deleting rows from SurrealDB and blobs off disk (and getting the
shared-blob accounting wrong if they do). This scope adds the missing verb: **delete one
package version from the local index**, safely.

"Safely" is the whole scope. The index feeds resolution (`reconcile::resolve` reads
`pkg_local` local-first), the blob it points at is **content-addressed and shared** (slice
6's remote poller and a local publish store the same artifact **once**, keyed by digest),
and a running instance references a package by `(package, version)` that resolves *through*
this index. A naive `DELETE` orphans a running service, breaks a rollback target, or GC's a
blob another version still points at. The delete verb is defined by what it **refuses**.

Slice 8 explicitly deferred this: *"No blob GC/retention policy here (a follow-up); v1 keeps
every committed blob."* This is that follow-up, scoped as the operator-facing delete rather
than a background GC daemon — the operator names the version to remove; the box reclaims the
blob **only if** nothing else needs it.

## Goals

- **`DELETE /api/packages/{name}/{version}?arch=<arch>`** — Bearer-gated (the slice-2 path,
  same as `GET /api/packages` and every `/api/*` verb; unauthenticated → 401, unclaimed →
  423). Removes the `pkg_local` row for one `(name, version, arch)` and, **if no other index
  row references its `digest_hex`**, deletes `blobs/<digest_hex>`. Returns a JSON body
  reporting what happened: `{ removed: true, blob_reclaimed: bool, digest_hex, freed_bytes }`.
  `arch` defaults to the host arch when omitted (the common case: an operator deleting a row
  they can see, which is almost always their own arch).
- **The in-use wall (load-bearing).** A version that any instance currently references is
  **never** deleted. "References" = the instance's `version` **or** any entry in its
  `kept_previous` (a retained rollback target — deleting its blob would make a rollback
  install a missing artifact). Attempting it → **409 Conflict** naming the instance(s) that
  hold it, and **nothing** is removed (row and blob both intact). This is the delete twin of
  the `ForeignUnit` ownership wall the remove-bundle path already enforces: destructive verbs
  refuse when something depends on the target.
- **Shared-blob-safe reclamation.** The blob is deleted **only after** confirming no other
  `pkg_local` row (any name/version/arch) carries the same `digest_hex`. Two versions that
  happen to hash identically, or the same artifact published under two names, keep the blob
  alive until the last referrer is gone. The row is always removed; the blob's fate is
  conditional and reported (`blob_reclaimed`).
- **CLI twin — `rubixd delete-package <name> <version> [--arch <arch>]`.** The file-and-wire
  parity rule this repo holds everywhere: the CLI POSTs to the local server on the same code
  path (reads the admin token like the other authed verbs), so a hand/CI delete and the REST
  delete are one path. Prints the same removed / reclaimed / freed-bytes summary.
- **UI — a Delete action on the `/packages` row.** A trash affordance per row that calls the
  new verb, behind a **confirm** (it is destructive and, once the blob is gone, only a
  re-publish restores it). On 409 it surfaces the server's "in use by <instance>" verbatim —
  the page is a lens; the refusal is the server's. On success the row disappears and the
  freed bytes are shown. No new client trust logic: the server is the only gate.
- **Idempotent absence.** Deleting a `(name, version, arch)` that is not in the index → **404**
  (`not found`), not a 500 — a double-click or a stale page must read cleanly, and 404 for an
  absent row is not an oracle problem here (the index is operator data behind the same Bearer
  wall the read uses).

## Non-goals

- **No cascade / no "delete all versions".** One `(name, version, arch)` per call. Bulk
  delete invites deleting a version out from under a rollback target you forgot about; if it's
  wanted later it composes from this verb, but v1 is one row at a time so the in-use wall is
  evaluated per artifact.
- **No background GC daemon, no LRU retention, no size cap.** The operator names what leaves;
  the box does not decide on its own to evict. (A time/size retention policy is a separable
  future scope — it would build on this verb's shared-blob accounting, not replace it.)
- **No force-delete-while-in-use flag.** There is no `--force` that tears the blob out from
  under a running instance. The wall is the point; the way to delete an in-use version is to
  remove the bundle/instance first (slice-6 `remove-bundle`), then delete the package. A flag
  that skips the wall is one typo from bricking a running service (the same reasoning that
  kept `allow_unsigned` out of slice 9).
- **No remote deletion.** This deletes from **this box's** local index only. A standalone box
  owns its own cache; it does not reach into a rartifacts server (that is rartifacts' `yank`,
  a different trust domain).
- **No new auth model** — the slice-2 Bearer path verbatim, admin-only in v1 (a standalone box
  has one operator; widen with the publish grant later if it lands).
- **No un-delete.** Once the blob is reclaimed, recovery is a re-publish. The confirm and the
  freed-bytes report make the cost visible; there is no trash-can state to restore from.

## Intent / approach

- Files: `crates/rubixd/src/publish/local_index.rs` gains `delete_local_package` +
  `referrers_of_digest` (the shared-blob count) + `instances_referencing`
  (the in-use query over `InstanceState.version` ∪ `kept_previous`);
  `crates/rubixd/src/blob/` gains a `reclaim.rs` verb (the **only** deleter of
  `blobs/<digest>`, mirroring `commit.rs` as the only writer — one writer, one reaper);
  `crates/rubixd/src/server/packages_delete.rs` (the `DELETE` handler);
  `crates/rubixd/src/cli/delete_package.rs`; a Delete button + `apiDelete` helper in
  `ui/packages.html` / `ui/app.js`. One responsibility per file (FILE-LAYOUT, ≤400 lines).
- **The order of operations is the safety contract**, and it is fixed:
  1. Resolve the row (absent → 404).
  2. Query instances for any referrer (`version` or `kept_previous`) → **409, nothing
     touched** if found.
  3. Delete the `pkg_local` row.
  4. Count remaining referrers of `digest_hex` in `pkg_local`; if **zero**, `blob::reclaim`
     the file; else leave it and report `blob_reclaimed: false`.
  Steps 3→4 are row-first so a crash between them leaves an **orphan blob** (reclaimable,
  harmless — the safe direction), never a **dangling index row** pointing at a deleted blob
  (which would fail a resolve). This is the deliberate inverse of `commit`'s
  blob-before-row publish ordering: publish makes the blob durable first so no row ever
  points at nothing; delete drops the row first so no row is left pointing at nothing. Both
  choices bias crashes toward an orphan blob, never a broken pointer.
- **`blob::reclaim` is the single reaper**, exactly as `blob::commit` is the single writer.
  It refuses to delete unless handed the digest by the index layer (it never scans and
  guesses), tolerates a missing file (already gone → Ok, so a retry after a mid-delete crash
  is clean), and `fsync`s the parent dir after unlink so the removal survives a crash the same
  way commit's does.
- **The in-use query reads live ledger, not a cached count.** Instances are read at delete
  time, not from a maintained refcount — a refcount is a second source of truth that drifts
  the first time a transaction half-applies. The query is `list_instances` filtered in Rust
  (the population is tiny — a box runs a handful of instances), not a store-side join, to
  keep it in the same read path the rest of the daemon uses.
- Alternative rejected — **soft-delete (a `deleted` flag on the row).** It keeps the blob
  forever (defeating the reclaim-disk half of the ask), and a "deleted" row the resolver must
  learn to skip is a new way for resolution to go wrong. Hard delete with an in-use wall is
  simpler and the wall already prevents the only unsafe hard-delete.
- Alternative rejected — **background reference-counted GC.** A daemon that sweeps
  unreferenced blobs is more machinery than the ask (the operator wants a Delete button) and
  introduces a background actor that can race a publish. The operator-triggered path reclaims
  synchronously with the blob accounting evaluated once, under the request.

## How it fits the core

Not an lb node (the parent records the translation). The walls this touches:

- **Trust:** unchanged. Delete removes a *verified* artifact; it opens no unsigned path and
  no new write of executable content. The verify-before-store wall is upstream of anything
  this verb sees.
- **Ownership / in-use:** this scope's wall is the *read-side* twin of the `ForeignUnit`
  wall — remove-bundle refuses to tear down a unit rubixd doesn't own; delete-package refuses
  to remove an artifact an instance still depends on. Same principle: a destructive verb
  refuses when something depends on the target, and refuses **before** touching anything.
- **Capabilities / auth:** the slice-2 Bearer path. `DELETE /api/packages/*` is Bearer-gated
  exactly like `GET /api/packages`; admin-only in v1.
- **Data (SurrealDB + blobs):** deletes one `pkg_local` row and conditionally one blob file.
  No new table, no schema change. `blob::reclaim` is the sanctioned single deleter, the
  counterpart to `blob::commit`.
- **One responsibility per file:** one handler, one CLI verb, one reaper verb, the index
  queries beside the existing index writes.
- **MCP surface:** N/A — rubixd is not an lb node; the REST surface is its contract. **API
  shape:** one new verb, `DELETE /api/packages/{name}/{version}` — the destructive twin of
  slice 9's `GET /api/packages`, following the same `/api/*` shape and gate.
- **Skill doc:** extends the repo's `.claude/skills/verify/SKILL.md` (it already drives the
  `/packages` page in headless Chrome) with the delete + in-use-refusal flow. No new
  agent-drivable concept beyond the one REST verb.

## Example flow

1. Operator published `rubix-demo-app` 1.0.0 and 2.0.0 (both `x86_64`, distinct digests —
   the screenshot state). 1.0.0 is installed as instance `demo`; 2.0.0 is not installed.
2. On `/packages` they click **Delete** on the **2.0.0** row and confirm. rubixd resolves the
   row, finds **no** instance references 2.0.0, deletes the `pkg_local` row, finds no other
   row shares its digest, `reclaim`s `blobs/<b8c4…>`, and returns
   `{ removed: true, blob_reclaimed: true, freed_bytes: 1_198_… }`. The row vanishes; the page
   shows the space reclaimed.
3. They click **Delete** on the **1.0.0** row. rubixd finds instance `demo` references
   `1.0.0` → **409**, `nothing removed`, body names `demo`. The page shows *"in use by demo —
   remove the instance first"*; both the row and the blob are intact.
4. They remove the `demo` instance (slice-6 `remove-bundle`), then delete 1.0.0 → succeeds,
   blob reclaimed.
5. Deleting 2.0.0 again → **404** (`not found`); the page is already correct.
6. A box where 1.0.0 and 1.0.1 were published from the **same** binary (identical digest):
   deleting 1.0.0 removes its row but `blob_reclaimed: false` (1.0.1 still points at it);
   deleting 1.0.1 afterward reclaims the shared blob.

## Testing plan

Per `testing-scope.md` — no mocks; real embedded store, real HTTP server, real blobs on disk,
real headless browser for the UI leg.

- **The in-use wall (mandatory deny test).** Seed a package, install an instance on it, then
  `DELETE` that version → **409**, body names the instance, and assert **both** the
  `pkg_local` row **and** `blobs/<digest>` are unchanged. Repeat for a version held only in an
  instance's `kept_previous` (a rollback target, not the current version) — also 409, also
  untouched. This is the load-bearing test: *the box refuses to delete an artifact a running
  or rollback-reachable instance depends on.*
- **Shared-blob accounting.** Publish two versions with the **same** digest (same bytes,
  different version strings). Delete the first → row gone, `blob_reclaimed: false`, blob still
  on disk. Delete the second → `blob_reclaimed: true`, blob gone. Assert `freed_bytes` is
  reported only on the reclaiming delete.
- **Happy path + disk reclaimed.** Publish an un-installed package, delete it → 200,
  `removed: true`, `blob_reclaimed: true`, the `pkg_local` row is gone **and** `blobs/<digest>`
  is gone, `freed_bytes` == the payload size.
- **Idempotent absence.** Delete a `(name, version, arch)` that was never published, and
  delete an already-deleted one → **404** both times, no 500, nothing else touched.
- **Crash-safety ordering (unit).** Row-delete-then-reclaim: assert that a failure injected
  between the row delete and the blob reclaim leaves an **orphan blob** (no index row), never a
  **dangling row** (a row whose blob is gone) — the safe direction.
- **CLI ↔ REST parity.** `rubixd delete-package` and a raw `DELETE /api/packages/...` on
  equivalent state produce the identical index/blob outcome and the same summary — the file
  and the wire are one path.
- **Auth.** Unauthenticated `DELETE` → 401; on an unclaimed box → 423 (the shipped
  401/423 contract, re-asserted on the new verb). The `/packages` HTML page stays an open
  static route; only `/api/packages` (GET and DELETE) is gated.
- **Route-table snapshot.** The UI adds **no** new server verb beyond `DELETE /api/packages/*`
  — the Delete button drives the same REST verb the CLI does (the slice-7 "UI adds no verbs"
  posture, one documented exception, mirroring slice 9's `GET /api/packages`).
- **UI, driven from the browser.** Delete a row → it disappears and freed bytes render; delete
  an in-use row → the 409 message renders verbatim and the row stays. Confirm is required
  before the call fires.

## Risks & hard problems

- **The shared blob is the trap.** The single most likely bug is deleting a blob that another
  index row (or, worse, a computed refcount that drifted) still points at, turning a later
  resolve into a missing-artifact failure. Mitigation: reclaim reads **live** `pkg_local` for
  referrers at delete time (no cached count), `blob::reclaim` is the single reaper, and the
  shared-digest test above is mandatory. If reclaim ever grows a fast-path that skips the
  referrer count, that test is what should fail first.
- **In-use is broader than "the running version."** `kept_previous` is the non-obvious half:
  an instance on 2.0.0 with 1.0.0 retained will roll back to 1.0.0, so 1.0.0's blob must
  survive even though nothing is *running* it. Missing this makes auto-rollback (slices 4/10)
  install a deleted artifact — a bricked box discovered only under failure. The wall checks
  `version` ∪ `kept_previous`; the rollback-target test is not optional.
- **Delete-during-transaction.** A delete that races a reconcile installing that same version
  could pull the blob mid-install. Mitigation: the in-use query is read at delete time and a
  mid-flight install leaves a `tx_state`/instance row the query sees; the conservative outcome
  (refuse the delete) is correct under a race. Worst case is a spurious 409, never a pulled
  blob — the safe failure.
- **The `self` instance.** Slice 10 installs rubixd as a package under the reserved `self`
  instance; deleting the running rubixd's own blob would be self-sabotage. `self` is an
  ordinary `InstanceState`, so the in-use wall covers it for free — but a test should assert
  deleting rubixd's own current/kept version → 409, because "the wall covers self too" is
  exactly the kind of thing that's true until someone special-cases `self` elsewhere.
- **404 vs 409 must not leak.** An anonymous caller is already walled (Bearer-gated), so 404
  for an absent row is not an oracle. But keep 409 (in-use) distinct from 404 (absent) and
  200 (deleted) in the body so the operator and the UI can tell "there was nothing to delete"
  from "there was, and I refused."

## Open questions

- **`freed_bytes` when `blob_reclaimed: false`.** Report `0`, or the payload size that is
  *still* held by another referrer? Recommendation: `0` — the field means "disk this delete
  freed," and a shared delete freed none. The size is still visible on the surviving row.
- **Arch default vs. require.** Omitting `?arch=` defaults to the host arch (the common case).
  Should a box holding multiple arches for the same version force an explicit `arch` to avoid
  deleting the wrong one? Recommendation: default to host arch but have the UI always send the
  row's exact arch (it knows it), so the ambiguity only exists for hand-crafted CLI/curl calls
  where the operator is already explicit.
- **Audit trail.** rartifacts writes a `pkg_event` per publish/refusal; rubixd's local index
  does not audit publishes today. A delete is arguably more worth recording than a publish
  (it's destructive). Recommendation: out of scope for v1 to match the existing local-index
  posture (no publish audit either), but flagged — if a local audit log ever lands, delete is
  a first-class event.

## Decisions resolved in implementation (2026-07-21)

Built as scoped — both safety walls, the row-first ordering, the CLI/REST parity, no
`--force`/cascade/GC-daemon. The open questions resolved as recommended:
`freed_bytes` is `0` on a non-reclaiming delete; `?arch=` defaults to the host arch and the
UI always sends the row's exact arch; no audit trail in v1. One finding worth recording:

- **The shared-digest case is structurally near-impossible through the real publish path —
  the guard is defensive, not a live path.** The scope's example flow #6 ("1.0.0 and 1.0.1
  published from the same binary → identical digest → `blob_reclaimed:false` until the last
  referrer") cannot actually arise, because the digest is computed over `(metadata,
  payload)` and the metadata TOML carries the **version** (and name, and kind). Two distinct
  `(name, version, arch)` rows therefore have distinct metadata and so distinct digests;
  byte-identical metadata means the same record-id key, which is one row, not two. So no
  normal publish ever produces two index rows sharing a digest. The `digest_referrers`
  count-before-reclaim guard is kept anyway — it is cheap, it is the correct defensive shape
  if the digest scheme ever changes or a row is hand-inserted, and removing it would make the
  reaper assume uniqueness it cannot prove. The integration test consequently seeds the
  shared-digest state **directly at the ledger + blob layer** (two `pkg_local` rows pointed
  at one committed blob) rather than through the publish path, because the publish path
  cannot construct it. This tightens example flow #6 to "a defended edge, not an operator
  workflow".
- **In-use is read live, never refcounted** (as scoped): the handler reads `list()` and
  filters `version ∪ kept_previous` in Rust at delete time. The `self` instance falls out of
  this for free and is asserted (`delete_self_instance_current_version_is_refused`) so a
  future special-case of `self` elsewhere cannot silently un-protect it.
- **Files** (one responsibility each): `blob/reclaim.rs` (the single reaper),
  `server/packages_delete.rs` (the fixed 4-step handler), `cli/delete_package.rs` (the
  REST-client twin), `publish/local_index.rs` (+`delete_local_package`, +`digest_referrers`),
  router + CLI dispatch, `ui/{app.js,packages.html}` (`apiDelete` + confirmed Delete button).
  Verified: lib 232 (+5 unit), `package_delete_test` 7/7, clippy clean, and proven
  end-to-end on a live `demo/run-ux-box.sh` daemon (published 1.0.0+2.0.0, CLI-deleted 2.0.0,
  v2 blob confirmed gone off disk, v1 intact, re-delete idempotent).

## Related

[`README.md`](README.md) roadmap · [`local-publish-scope.md`](local-publish-scope.md)
(slice 8 — `pkg_local`, the content-addressed blob cache, `blob::commit` the single writer,
and the explicit *"no blob GC — a follow-up"* deferral this scope fulfills) ·
[`ui-local-publish-scope.md`](ui-local-publish-scope.md) (slice 9 — the `/packages` page and
`GET /api/packages` this adds the Delete twin to) ·
[`bundles-scope.md`](bundles-scope.md) (slice 6 — `remove-bundle` and the `ForeignUnit`
ownership wall this scope's in-use wall mirrors; the resolver that reads the index a delete
removes from) · [`rollback-health-scope.md`](rollback-health-scope.md) (slice 4 —
`kept_previous`, the rollback targets the in-use wall must protect) ·
[`self-update-scope.md`](self-update-scope.md) (slice 10 — the reserved `self` instance the
wall must also cover) · [`token-auth-scope.md`](token-auth-scope.md) (the Bearer/claim path
and the 401/423 states the verb inherits) ·
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md) (the trust wall, unchanged).
