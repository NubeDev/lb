# Pack upload scope — a pack as ONE `.zip`, over the wire (`POST /packs/upload`)

Status: **SHIPPED** — 2026-07-29 (`crates/packs/src/zip.rs` + `role/gateway/src/routes/packs.rs`;
session: `docs/sessions/packs/pack-upload-session.md`). Owning repo: **lb (core)** — this is the
upstream half of the downstream ask **U-pack-upload** (NubeIO/rubix-ai#57, consumer scope
`docs/scope/frontend/pack-upload-scope.md` in rubix-ai). The engine, the manifest format and the
refusal matrix are NOT touched here — they are `docs/scope/packs/pack-core-scope.md`, unchanged.

> **Not released.** No `node-v*` tag has been cut for this work and rubix-ai has NOT bumped its
> `lb-node` pin — and it is not merged either: everything below is a working tree in this checkout,
> green locally (see the session doc's test block), unreviewed and un-CI'd. No consumer can see it yet.
> The downstream drop-zone UI is blocked on that tag (`docs/WORKFLOW-LB.md` §4).

A pack is *distributed* as one file. A customer is handed `ems.zip`; an author downloads a repo as
a ZIP; a browser drop-zone yields a `File`. But the only pack surface core offered was `/mcp/call`
with a hand-assembled `{manifest, files}` JSON bundle — a shape only a program that already knows
the manifest's file references can build. Every embedder that wanted "drop the zip here" therefore
had to unzip in the client and re-assemble the bundle: the same reader, written again, in a second
language, drifting against the node's rules. And it could not even be made to work at the sizes that
matter, because the transport was *narrower than the engine* (below). This scope makes the archive
itself a first-class transport envelope, unpacked ONCE, in core, by the rules the node enforces.

## Goals

- **`bundle_from_zip(bytes) -> Result<Bundle, String>`** in `lb-packs` — pure, I/O-free, the whole
  archive contract in one file (`crates/packs/src/zip.rs`), exercised without a node. A zip in, an
  ordinary `Bundle` out; nothing downstream can tell how the bundle arrived.
- **`POST /packs/upload`** on the gateway — `multipart/form-data`, one file part,
  `?verb=validate|apply` (default `validate`) `&ts=`, dispatching through the SAME
  `lb_host::call_tool_on_node` chokepoint to `pack.validate` / `pack.apply` and returning the verb's
  own JSON verbatim.
- **A transport that cannot be narrower than the engine.** The route's body limit is DERIVED from
  `MAX_BUNDLE_BYTES`, and a unit test asserts the inequality rather than trusting two constants to
  be edited together.
- **Hostile archives die at the door**, each rejection naming the offending member so an author can
  fix the archive rather than guess: zip-slip, a non-text member, a zip bomb, no root `pack.yaml`.
- **`curl` is a first-class client**: `curl -F pack=@ems.zip 'http://…/packs/upload?verb=apply'`
  installs a pack with no ceremony and no client-side unzip.

## Non-goals

- **No new pack semantics.** The archive is a transport envelope and is discarded. The refusal
  matrix, the clobber rule, receipts, per-object caps — all pack-core, all untouched. If a change
  would alter what an *applied* pack means, it does not belong in this scope.
- **No `pack.upload` verb.** The upload is a *transport* affordance, not a new capability: it
  dispatches to the two verbs that already exist, under the caps that already gate them. Adding a
  verb would have created a third thing to authorize for zero new authority.
- **No CLI `pack-apply --pack foo.zip`** (see Open questions — resolved, deliberately not done).
- **No archive authoring/signing.** A pack zip is a plain zip. Signed pack artifacts, if they ever
  happen, ride the extension-artifact toolchain scope, not this one.
- **No UI.** The drop-zone, the progress and the error rendering are rubix-ai's half of the ask.

## Intent / approach

**The archive rules, and why each one is a rule.** All of them live in `zip.rs`, all of them name
the member in the error:

- **Zip-slip** — a member with an absolute path or a `..` component is refused. The guard is the zip
  crate's own `enclosed_name()`, but its `None` is *restated* with the member name, because "which
  member, and why" is the entire value of the error to whoever built the archive.
- **Non-UTF-8 member** — packs are declarative text. A binary member means the wrong thing was
  zipped, and saying `logo.png is not UTF-8 text` is a fix; "invalid bundle" is not.
- **The zip bomb** — the budget is the **total inflated** byte count against `MAX_BUNDLE_BYTES`, and
  it is enforced *while inflating*, member by member, via a `take(budget + 1)` read. A zip that
  declares 4 KB and expands to a gigabyte must die **against** the budget, not after it has already
  cost the memory. The rejected alternative — check the compressed size, or inflate then measure —
  is respectively wrong (compression ratio is attacker-chosen) and useless (the memory is already
  spent).
- **The single top-level folder** is stripped, because `zip -r ems.zip ems/` and GitHub's "Download
  ZIP" both produce it and refusing them would make the affordance useless. But **only when
  unambiguous**: the archive must not already be pack-rooted, and every member must share that one
  segment. Two top-level folders is not guessed at — it falls through to the honest "no `pack.yaml`
  at the root" error. Guessing here would silently install the wrong half of an archive.
- **`__MACOSX/`, `.DS_Store`, `Thumbs.db`** are dropped *first*, before any other rule, so a
  macOS-zipped pack is not rejected as "binary member `__MACOSX/._pack.yaml`" — a rejection that is
  technically true and completely unactionable.

**`MAX_BUNDLE_BYTES` 8 MiB → 32 MiB.** The 8 MiB figure predated packs carrying their own schema
plus a structured seed; a real product pack now runs past it while still being nothing but text.
The **doctrine has not moved** — a multi-hundred-MiB seed still belongs in a generator script, and
the over-cap error says exactly that — the ceiling merely stopped being the thing honest authors hit
first.

**The inverted ceiling, which is the real bug this scope fixes.** `/mcp/call` carries axum's 2 MiB
default body limit while the engine's bundle cap was 8 MiB. A pack between the two was refused by
the *transport*, with a bare `413` and nothing to act on, before any handler ran — the engine would
have happily taken it. The fix is not a number but a **derivation**: `upload_body_limit() =
MAX_BUNDLE_BYTES + UPLOAD_LAYER_MARGIN`, so raising the engine cap raises the transport for free and
the two can never invert again.

*Rejected alternative: raise `/mcp/call`'s limit globally.* That limit is a deliberate blast-radius
cap on the **generic** verb transport; fattening every verb to carry one verb's payload is the wrong
trade, and it is also the rule-10 smell — a named verb's need leaking into the generic path.
`/mcp/call` deliberately keeps its 2 MiB, and the new limit is **route-scoped**.

**The margin, and why the route re-checks the length itself.** The layer's limit carries 1 MiB of
headroom over the semantic cap. That headroom is not slop: it covers multipart framing, and it lets
a *just*-oversized upload actually reach the handler, which returns a descriptive `413` naming the
size **and** the limit **and** the way out, instead of the layer's bare "length limit exceeded".
Anything past the margin the layer bounces — that is the real memory guard. This mirrors the
posture already shipped on `/extensions`.

**The route is transport and nothing else.** It authenticates, inflates, and hands the bundle to
`lb_host::call_tool_on_node` — the same chokepoint `/mcp/call` uses. Caps wall, workspace wall,
telemetry, audit and undo therefore fire exactly once, in the one place they already live. There is
no second authorization path to keep in sync, because there is no second path.

**The default verb is the safe one.** No `?verb=` means `pack.validate`. An upload that silently
applied would turn a fat-fingered `curl` into a workspace mutation. The dispatchable set is a closed
enum of two, so an unknown `?verb=destroy` is a serde parse failure — never a fallback to apply.

**Multipart field naming is lenient, arity is strict.** Clients disagree on the field name (`curl -F
pack=@x.zip`, a browser `FormData`, a hand-rolled body), so the route takes the first part carrying
a filename, else the first part named `pack`. But two archives in one request is a `400`: silently
taking the first would install something the caller did not name.

**The knowingly-duplicated contract.** rubix-ai's browser-side reader
(`ui/src/lib/packs/readZip.ts`) enforces the same rules so a bad archive is rejected before it
wastes an upload. Two implementations of one contract is a drift risk, taken with eyes open, on the
non-negotiable that **the node never trusts the client to have done it** — the browser's copy is an
optimization, the node's copy is the wall. The module doc in `zip.rs` names the sibling explicitly
so a change to either is visible from the other.

## How it fits

- **Capabilities.** No new cap. `?verb=validate` needs `mcp:pack.validate:call` (read-tier);
  `?verb=apply` needs `mcp:pack.apply:call` (admin-tier), and then every object inside the apply is
  re-checked under the caller's principal by the ordinary host seam, exactly as pack-core specifies.
  The upload grants **nothing** `/mcp/call` does not — proven by the deny test, not asserted.
- **Workspace isolation.** The workspace comes from the **token**, never from the archive or the
  query string. A pack applied with a ws-A token is invisible to ws-B, and ws-B uploading the same
  archive is a *first* apply — the integration test pins both halves.
- **Rule 10.** No pack is named anywhere in `zip.rs`, the route, or the router entry. An archive is
  data. The dispatchable verb set is closed and explicit, so an upload cannot be piped into an
  arbitrary tool by naming one in the query. The body limit is route-scoped, not a global bump for
  one feature's benefit.
- **Symmetric nodes.** Nothing role-gated: the route is registered on the ordinary gateway router
  like every other route, and a pack uploads wherever the workspace lives.
- **API shape.** `POST /packs/upload?verb=validate|apply&ts=<epoch-seconds>`,
  `multipart/form-data`, one file part. Response is the verb's own JSON **verbatim** — the dry-run
  report for `validate`, the apply result for `apply` — so a client that already renders
  `pack.validate` output over `/mcp/call` renders this with no new type. `401` unauthenticated,
  `403` without the verb's capability (opaque), `400` for an archive that is not a pack bundle,
  `413` over the limit with size and limit named. `ts` absent ⇒ the node's own clock, so a `curl`
  install needs no ceremony.

## Testing plan

The house bar (rule 9): a real node, a real router, real tokens, no mocks.

- **`crates/packs/src/zip.rs` — 13 unit tests**, pure: the manifest hoist, the single-root strip, the
  refusal to guess between two roots, zip-slip, an absolute member, a binary member named, no
  manifest, the inflate-past-cap bomb (asserting the *archive itself* is small — that is the point),
  the cap as a total across members, macOS noise ignored, non-zip bytes. The closing test is the
  claim of the whole file: an inflated bundle `resolve()`s identically to a hand-assembled one.
- **`role/gateway/tests/pack_upload_test.rs` — 7 integration tests** on `Node::boot_as(Hub)` +
  the real router, with the multipart body **hand-built** so it pins the exact wire `curl -F` emits:
  validate → apply → re-upload-is-noop; the **mandatory deny case** (a validate-only token previews
  and gets `403` on apply); unauthenticated `401`; the **mandatory isolation case** (ws A's pack
  invisible to ws B, and ws B's identical archive is a first apply); zip-slip `400` with nothing
  applied; a binary member `400` naming it; and a no-archive `400` that tells the caller
  `curl -F pack=`.
- **3 route unit tests**: the derived limit is never below the engine cap, the default verb is the
  read-only one, and an unknown `?verb=` fails to parse.

## Risks & hard problems

- **Two readers, one contract** (browser + node). Mitigated, not eliminated: the node's copy is
  authoritative and the browser's is an optimization, and each module names the other. If the rules
  grow, they grow in `zip.rs` first.
- **A raised cap is a raised memory floor.** 32 MiB of inflated text is held to build the bundle.
  The running budget bounds it, and the route limit bounds the compressed body, but the honest
  statement is that a large pack costs the node that memory for the duration of the call. The
  standing doctrine — big seed = generator script — is the mitigation, not the constant.
- **Apply is still not a transaction.** Uploading changes nothing about that: a partial apply is a
  first-class outcome recorded on the receipt (pack-core), and a `400` at the door means *nothing*
  was applied, which is a different and stronger guarantee the tests pin.
- **Multipart leniency.** Taking "the first part with a filename" is a heuristic. It is bounded by
  the strict arity rule (two archives is a `400`), but a client that sends an unrelated file part
  first would be surprised. Judged the right trade against demanding one exact field name that
  browsers and `curl` do not agree on.

## Open questions

Both resolved in the implementing PR, recorded here because the alternatives were live:

1. **Content type: multipart vs a raw `application/zip` body.** **Chosen: multipart.** It is what a
   browser form and `curl -F` produce with no ceremony, and it leaves room for additional named
   parts later (a signature, a target name) without a second content type to support forever. A raw
   `application/zip` body is marginally simpler on the wire and was rejected for exactly that
   inflexibility, plus the client-side awkwardness of building a raw-body upload from a `File`.
2. **A CLI `pack-apply --pack foo.zip`.** **Deliberately NOT done here.** The ask (U-pack-upload) is
   the *wire* surface, and the CLI already installs packs from a directory. Adding an archive flag
   would be a second, node-local unpack path with its own rules to keep aligned, for a caller who by
   definition has the unzipped directory to hand. Named as a future ask, not an omission.

## Related

- `docs/scope/packs/pack-core-scope.md` — the engine, the manifest format, the refusal matrix, the
  caps tiers. This scope adds a transport in front of it and changes none of it.
- NubeIO/rubix-ai#57, `docs/scope/frontend/pack-upload-scope.md` (in rubix-ai) — the **consumer
  scope**: the drop-zone UI, the browser-side `readZip.ts` pre-check, and the pin bump. It is blocked
  on a `node-v*` tag that does not exist yet.
- `docs/sessions/packs/pack-upload-session.md` — what the implementing session decided and measured.
- `docs/scope/extensions/pack-toolchain-publish-scope.md` — the unrelated *extension artifact*
  packager (`lb-pack`), named only to keep the two "pack" meanings apart, as pack-core does.
