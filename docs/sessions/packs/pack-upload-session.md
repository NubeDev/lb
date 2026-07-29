# Session — a pack as ONE `.zip` (`POST /packs/upload`)

Date: 2026-07-29 · Scope: `docs/scope/packs/pack-upload-scope.md`
Downstream ask: **U-pack-upload** — NubeIO/rubix-ai#57 (`docs/scope/frontend/pack-upload-scope.md`)

> **Nothing is released.** This is on lb `main` and green; **no `node-v*` tag has been cut** and
> rubix-ai has **not** bumped its `lb-node` pin. The downstream drop-zone cannot see any of it yet.

## What shipped

The archive became a first-class transport envelope. A pack is distributed as one file, and now it
can be *installed* as one file — `curl -F pack=@ems.zip '…/packs/upload?verb=apply'` — with the
unpacking done once, in core, by the rules the node enforces, instead of re-implemented in every
client that wants a drop-zone.

- **`crates/packs/src/zip.rs`** (new) — `bundle_from_zip(bytes) -> Result<Bundle, String>`, pure,
  zero I/O, 13 unit tests. Exported from `lib.rs` alongside `MANIFEST_FILENAME`.
- **`role/gateway/src/routes/packs.rs`** (new) — `POST /packs/upload`, multipart,
  `?verb=validate|apply&ts=`, dispatching through `lb_host::call_tool_on_node`.
- **`role/gateway/src/server.rs`** — the route registration with its own derived body limit.
- **`role/gateway/src/routes/mcp.rs`** — `tool_error_status` widened to `pub(crate)` so the upload
  route maps a tool error to a status the *same* way `/mcp/call` does. Reuse, not a second mapping:
  a divergence here is how a `403` quietly becomes a `500` on one path only.
- **`crates/packs/src/bundle.rs`** — `MAX_BUNDLE_BYTES` 8 MiB → 32 MiB.
- **`role/gateway/tests/pack_upload_test.rs`** (new) — 7 integration tests, real node, real router.

**No new verb, no new capability, no envelope change.** Every existing pack path is byte-identical.

## The decisions this session made

**1. Transport, not a verb.** The route does three things — authenticate, inflate, dispatch — and
then it is out of the way. It calls the SAME `call_tool_on_node` chokepoint `/mcp/call` calls, so
the caps wall, the workspace wall, telemetry, audit and undo fire once, in the place they already
live. A `pack.upload` verb was considered and rejected: it would have created a third thing to
authorize for exactly zero new authority.

**2. Multipart, not a raw `application/zip` body.** Multipart is what a browser form and `curl -F`
produce with no ceremony, and it leaves room for a second named part later (a signature, a target
name) without committing to supporting two content types forever. The raw body is simpler on the
wire and was rejected for that inflexibility plus the client-side awkwardness of posting a `File` as
a raw body. Field naming is deliberately **lenient** (first part with a filename, else the part
named `pack`) because clients genuinely disagree; arity is **strict** (two archives is a `400`)
because silently taking the first would install something the caller did not name.

**3. The default verb is `validate`.** An upload that silently applied would turn a fat-fingered
`curl` into a workspace mutation. The dispatchable set is a closed two-variant enum, so
`?verb=destroy` is a serde parse failure and never a fallback to apply.

**4. The cap: 8 MiB → 32 MiB, and the doctrine did not move.** The 8 MiB figure predated packs
carrying their own schema plus a structured seed; a real product pack now runs past it while still
being nothing but text. A multi-hundred-MiB seed still belongs in a generator script — the over-cap
error says so, in both the route and the inflater — the ceiling just stopped being the thing honest
authors hit first. The reasoning is written into the constant's doc comment, where the next person
to consider moving it will actually read it.

**5. The limit is DERIVED, because the real bug was an inverted ceiling.** `/mcp/call` carries
axum's 2 MiB default while the engine's bundle cap was 8 MiB: a pack between the two was refused by
the *transport*, with a bare `413`, before any handler ran — the engine would have taken it happily.
So `upload_body_limit() = MAX_BUNDLE_BYTES + UPLOAD_LAYER_MARGIN`, and a unit test asserts
`upload_body_limit() >= MAX_BUNDLE_BYTES`. Raise the engine cap and the transport follows for free;
forget this route and nothing breaks.

*The rejected alternative was raising `/mcp/call` globally.* That limit is a deliberate blast-radius
cap on the **generic** verb transport, and fattening every verb to carry one verb's payload is both
the wrong trade and the rule-10 smell. `/mcp/call` keeps its 2 MiB; the new limit is route-scoped.

The 1 MiB margin is not slop: it covers multipart framing and lets a *just*-oversized upload reach
the handler, which returns a descriptive `413` naming the size, the limit, and the way out, rather
than the layer's bare "length limit exceeded". Past the margin the layer bounces it — that is the
actual memory guard. Same posture already shipped on `/extensions`.

## The archive rules, and why each one is a rule

Every rejection **names the offending member**. That is the difference between an error an author
can act on and one they have to bisect the archive to understand.

- **Zip-slip** — `enclosed_name()` is the guard (absolute path, `..`, or a non-UTF-8 name), but its
  `None` is *restated* with the member name rather than passed along.
- **Non-UTF-8 member** — packs are declarative text; a binary member means the wrong thing was
  zipped. `logo.png is not UTF-8 text` is a fix; "invalid bundle" is not.
- **The zip bomb** — the budget is the total **inflated** bytes, enforced *while inflating*, member
  by member, with `take(budget + 1)`. A zip that declares 4 KB and expands to a gigabyte must die
  against the budget, not after it. Checking the compressed size instead is wrong (the ratio is
  attacker-chosen); inflating then measuring is useless (the memory is already spent).
- **The single top-level folder** is stripped, because `zip -r ems.zip ems/` and GitHub's "Download
  ZIP" both produce it — but **only when unambiguous**: not already pack-rooted, and every member
  sharing that one segment. Two top-level folders falls through to the honest "no `pack.yaml` at the
  root" instead of being guessed at, because guessing silently installs half an archive.
- **`__MACOSX/`, `.DS_Store`, `Thumbs.db`** are dropped *first*, before every other rule — otherwise
  a macOS-zipped pack is rejected as "binary member `__MACOSX/._pack.yaml`", which is true and
  completely unactionable.

## The knowingly-duplicated contract

rubix-ai's browser reader (`ui/src/lib/packs/readZip.ts`) enforces the same rules so a bad archive
is rejected before it wastes an upload. Two implementations of one contract is a drift risk taken
with eyes open, on one non-negotiable: **the node never trusts the client to have done it.** The
browser copy is an optimization; the node copy is the wall. `zip.rs`'s module doc names the sibling
so a change to either is visible from the other, and the rules grow in `zip.rs` first.

## Rule 10

No pack is named in `zip.rs`, in the route, or in the router entry — an archive is data. The
dispatchable verb set is closed and explicit, so an upload cannot be piped into an arbitrary tool by
naming one in the query string. The body limit is route-scoped, never a global bump for one
feature's benefit.

## No authority smuggled — proven, not asserted

The upload grants nothing `/mcp/call` does not, and the tests are the evidence rather than the
prose:

- `a_caller_without_the_apply_cap_previews_but_cannot_apply` — a token holding `pack.validate` but
  not `pack.apply` gets its `200` dry-run report and an opaque `403` on `?verb=apply`. Same wall,
  because it is the same chokepoint.
- `a_pack_applied_in_one_workspace_is_invisible_in_another` — the workspace comes from the **token**;
  neither the archive nor the query can reach across, and ws B uploading the identical archive is a
  *first* apply, not a no-op.
- `a_zip_slip_archive_is_refused_before_any_verb_runs` — a `400` at the door means nothing was
  applied, which is a stronger guarantee than a partial receipt.

The multipart body in that suite is **hand-built** rather than produced by a client crate, on
purpose: it pins the exact wire `curl -F pack=@ems.zip` sends, so the test fails if the route stops
speaking it.

## Test status (all green)

```
cargo test -p lb-packs                                  → 80 passed
cargo test -p lb-role-gateway --test pack_upload_test   →  7 passed
cargo test -p lb-role-gateway --lib                     → 41 passed (incl. 3 new routes::packs::tests)
cargo test -p lb-host --test pack_test                  → 19 passed, 3 ignored
cargo fmt --all --check                                 → clean
```

The 3 `lb-host` ignored tests are the pre-existing demo-oracle cases that need the federation
sidecar built (`-- --ignored`); unchanged by this work.

## Follow-ups (named, not done)

- **Cut a `node-v*` tag and bump rubix-ai's pin.** Until then the downstream drop-zone
  (NubeIO/rubix-ai#57) is blocked. Not pushed or tagged in this session.
- **CLI `pack-apply --pack foo.zip`** — deliberately out of scope. The ask was the wire surface, and
  the CLI already installs from a directory; an archive flag would be a second node-local unpack
  path with its own rules to keep aligned, for a caller who has the unzipped directory to hand.
- **The two readers** — if the archive rules grow, `zip.rs` moves first and the browser follows.
