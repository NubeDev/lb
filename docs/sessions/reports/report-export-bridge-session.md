# Reports — `report.export` over the MCP bridge (session)

- Date: 2026-08-21
- Scope: [`../../scope/reports/report-builder-scope.md`](../../scope/reports/report-builder-scope.md)
  (this repo's Typst/brand scope) and, for the ask this delivers,
  ext-ros `docs/scope/reports/report-builder-scope.md` **Track A**
- Stage: S3 — an existing surface reaches a caller that could not use it
- Status: done (code + tests). **Not yet released as a `node-v*` tag** — see Follow-ups.

## Goal

Make `report.export` reachable from an extension page.

It was deliberately absent from MCP dispatch: a gateway binary route, authenticating on
`Authorization: Bearer` only. A module-federated extension UI has no bearer token — its
`PageBridge` is `{call, setNav}` and the host withholds the credential on purpose — so an extension
could create, read, share and list reports and could not export one. Everything but the point.

This is the same caller, the same wall and the same answer as `media/read.rs`, whose module doc
already names the failure mode being prevented: extension authors lifting the session token out of
the host's `localStorage`, which voids the leash, bypasses the bridge's per-verb scope filter, and
breaks the day page extensions move behind an iframe sandbox.

## What changed

| File | Change |
|---|---|
| `rust/crates/host/src/report/export_media.rs` | **new** — the media-id envelope around `report_export` |
| `rust/crates/host/src/report/tool.rs` | the `"report.export"` dispatch arm; the "deliberately absent" module doc replaced with the shape and the measurement behind it |
| `rust/crates/host/src/report/error.rs` | `ReportError::Media(String)` — media-subsystem failures are their own variant, not folded into `Store` |
| `rust/crates/host/src/report/mod.rs`, `src/lib.rs` | module + re-exports (`report_export_media`, `REPORT_ORIGIN`) |
| `rust/crates/host/src/system/catalog/report.rs` | the catalog description now states the media-id shape |
| `rust/crates/host/src/tool_gate.rs` | `report_gate_tests` — pins the fall-through (no alias) |
| `rust/role/gateway/src/routes/report.rs` | the new error variant in the status mapper |
| `rust/crates/host/tests/report_export_media_test.rs` | **new** — 8 tests |
| `rust/role/gateway/tests/report_bridge_test.rs` | **new** — 5 tests, driven over `POST /mcp/call` |

The wire:

```text
1. snapshots up   — media.upload_begin → media.chunk_write × n → media.upload_commit  (shipped)
2. compose        — report.export { id, snapshotMediaId? } → { pdfMediaId, bytes, mime }
3. bytes down     — media.read { id, offset }                                          (shipped)
```

## Decisions & alternatives

**Media ids, not bytes — measured rather than chosen.** The obvious shape is snapshots in the
request and the PDF in the reply. It does not fit: `/mcp/call` carries a deliberate 2 MiB
blast-radius cap against this route's 32 MiB, and the comment beside it refuses exactly the widening
this feature would have asked for (*"ROUTE-scoped (rule 10): `/mcp/call` keeps its deliberate 2 MiB
blast-radius cap."*). Raising it to admit one feature is the thing that comment exists to prevent;
`/packs/upload` directly above it is the shipped precedent for the alternative.

Trading ids also **removes** a problem rather than moving one: the PDF is composed once and
*stored*, so slices 2..n are ordinary `media.read` calls and the "cache the bytes against a snapshot
digest with a TTL" problem the in-band shape creates simply does not arise. Resumability and
progress come free on the rhythm the upload half already uses.

**No second authorization path.** `report_export` authorizes `mcp:report.export:call` first and then
re-runs `dashboard_get`'s three gates under the same principal. The new function calls it and adds
nothing. The one `authorize_report` line at the top of `report_export_media` is that *same* gate run
early, so no media byte is read on behalf of a caller who is about to be refused — `report_export`
remains the authority, and a test asserts that removing either cap still denies.

**The PDF is stored under the CALLER's authority.** Through the ordinary `media_upload_begin` /
`chunk_put` / `commit` verbs, gated by `mcp:media.upload:call` — the grant the caller already needed
to put the snapshots up. *Rejected:* storing it with the host's own authority, which would mint a
record the caller may not be able to read back (`media_serve` re-checks `store:media/{id}:read` per
item) and would be the second authorization path this design exists to avoid.

**`snapshots` is a REQUIRED key in the uploaded bundle.** This was the one contract decision that
changed during the work. A caller who wants the report's skeleton omits `snapshotMediaId` entirely
and gets every cell as a titled error tile. A caller who uploaded a document that does not carry the
key has a *wire bug* — a renamed field, a half-written blob, the wrong media id — and
`#[serde(default)]` would answer that with a plausible-looking PDF of error tiles while every part
of the UI reported success. One serde attribute was the difference between a loud refusal and a
silently wrong document; the refusal names the honest way to ask for a skeleton.

**No `gate_tool_for` arm.** `report.export` falls through to its own concrete cap, which IS in
`AUTHOR_CAPS` beside `report.save`/`report.share` and is deliberately not covered by any
`mcp:*.*:call` wildcard (view-without-export is a real posture). *Rejected:* adding an alias
anyway — it would silently widen export to everyone who can read a report. The fall-through is
pinned by a test instead, because its absence is otherwise indistinguishable from nobody having
thought about it.

## Tests

**Mandatory categories, all present. Real store, real media store, real Typst compile, no mocks.**

- **Capability-deny over the BRIDGE** — `report_bridge_test.rs`, `POST /mcp/call`, with a passing
  negative control beside it. Non-negotiable, and it had to cross the outer gate: all four
  cap-aliasing incidents `tool_gate.rs` records were invisible to tests that called the host fn
  directly.
- **Reachability** — a principal minted through `provision_member`, i.e. whatever the *real* durable
  grant chain resolves to rather than a hand-written caps list, drives all three steps over the
  bridge and gets `%PDF-` bytes. **This is the test that would have caught every shipped-but-unusable
  verb in `tool_gate.rs`.**
- **Workspace isolation** — both at the function level and over the bridge, and the bridge one also
  asserts that a cross-workspace refusal is byte-identical to a missing id, so the status code is
  not an existence oracle for another workspace's records.
- **Parity** — the bridge verb and the gateway route compose **byte-identical** PDFs for the same
  `(id, snapshots)`. Two doors, one document.
- **Round trip through media**, **refusal of an ordinary dashboard**, **missing snapshots**, **the
  origin tag**, **a `data:`-prefixed snapshot refused by name**.

```
$ cd rust && cargo test -p lb-host --test report_export_media_test
running 8 tests
test a_snapshot_bundle_from_another_workspace_is_not_reachable ... ok
test a_snapshot_that_is_not_raw_base64_is_refused_by_name ... ok
test the_stored_pdf_is_tagged_as_report_output ... ok
test composing_with_no_snapshots_still_produces_a_pdf ... ok
test the_bridge_verb_is_workspace_isolated ... ok
test the_bridge_verb_round_trips_snapshots_and_pdf_through_media ... ok
test the_bridge_verb_denies_without_export_and_without_media_upload ... ok
test the_bridge_and_the_route_compose_byte_identical_pdfs ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-role-gateway --test report_bridge_test
running 5 tests
test an_ordinary_dashboard_is_refused_over_the_bridge ... ok
test the_export_does_not_bypass_the_dashboard_read_gate ... ok
test a_real_member_exports_a_report_over_the_bridge ... ok
test the_export_is_denied_over_the_bridge_without_its_own_cap ... ok
test the_bridge_export_is_workspace_isolated ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-host --test report_test --test report_export_test --test media_test
test result: ok. 29 passed   (report_test)
test result: ok. 3 passed    (report_export_test)
test result: ok. 8 passed    (media_test)

$ cargo test -p lb-role-gateway --test report_routes_test
test result: ok. 6 passed

$ cargo test -p lb-host --lib report_gate
running 2 tests
test tool_gate::report_gate_tests::export_gates_on_its_own_concrete_cap ... ok
test tool_gate::report_gate_tests::the_read_verbs_are_untouched ... ok
test result: ok. 2 passed
```

`cargo fmt --all` clean; `cargo clippy -p lb-host -p lb-role-gateway --all-targets` raises no new
warnings (the warnings it does raise are pre-existing and in other files).

## Debugging

None — no incident, so no `debugging/` entry. One test failure during development was a *design*
question rather than a defect (`{"hello":"world"}` composing a skeleton instead of being refused);
it is recorded under Decisions above and is now pinned by a test in both directions.

## Public / scope updates

- ext-ros `docs/scope/reports/report-builder-scope.md` Track A is satisfied by this change; its
  status line and issue #6 are updated from the ext-ros side.
- This repo's `docs/scope/reports/report-builder-scope.md` gains a note that the export now has a
  second door and what distinguishes it.
- **Not promoted to `doc-site/content/public/`**: the surface is not usable by anyone until a
  `node-v*` tag carries it, and a public page describing a verb no released node serves would be
  documentation of something that does not exist yet. It goes public with the tag.

## Skill docs

`docs/skills/reports/` exists and is empty. Not written this session: a SKILL.md is supposed to be
grounded in a live run, and this verb has not run on a live node yet (it needs the tag — see
Follow-ups). Writing one from the tests would be exactly the kind of ungrounded doc that directory's
convention exists to prevent.

## Dead ends / surprises

- **The catalog already listed `report.export`.** It has been advertised the whole time while
  falling through to `NotFound` on the bridge — a caller reading the catalog would have found it and
  been unable to call it. Now the listing is true.
- **The media store has no reaping seam at all.** `origin` is a pure provenance tag that nothing
  reads; `MediaStatus::Archived` (set by `media.delete`) is the whole of the lifecycle, and there is
  no media job or GC anywhere in the tree. The upstream scope's open question 1 asked for a reaping
  policy "before building"; the honest answer is that there is nothing to hang one on yet, so the
  PDFs are **tagged** (`origin = "report.export"`) and the sweep is named as housekeeping below.
  Tagging now is what makes that sweep a query rather than a migration.

## Follow-ups

1. **Cut a `node-v*` tag.** This is the release vehicle the upstream issue names, and nothing
   downstream can use the verb without it — an extension calling it against an older node gets
   `NotFound`, which reads as "export is broken" rather than "your node is old" (the kit already
   detects and re-words exactly that case). Deliberately **not** done in this session: tagging and
   publishing is an outward-facing release decision, not a code change.
2. **Bound the generated PDFs.** A report exported nightly leaves a PDF per run and nothing reaps
   them. The record carries `origin = "report.export"`, so a sweep is a query over one field —
   but the media module has no job seam to put it in, so this is its own small scope rather than a
   patch here.
3. **A live drive on a node, then the SKILL.md.** Both wait on (1).
4. **`docs/skills/reports/SKILL.md`** — see above.
