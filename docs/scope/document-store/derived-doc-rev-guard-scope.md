# Document-store scope — derived-doc rev guard + fork-to-editable (stub)

Status: scope stub (the ask). Surfaced by the doc-extraction implementation session
(2026-07-13) as a named follow-up, not worked around.

> Read with: `doc-extraction-scope.md` (the seam this hardens), `document-store-scope.md`
> (the doc `rev` this guards), README §3 rule 10.

## The gap

`docs.extract` writes derived docs at a **stable** id and **re-derivation overwrites** them
(a version bump or a changed source re-runs `put_doc` on the same id). That is correct for
regenerable output — but a user *will* hand-edit a derived doc (it is an ordinary `doc:{id}`
row the doc verbs let them write), and the next extraction silently clobbers those edits.

v1's answer is convention-only ("derived docs are read-only by convention"), which the
implementation honored but did not enforce. This stub is the honest backstop.

## The ask (v1 of the guard)

- **A `rev`-conflict guard on re-derivation.** Record the derived doc's `rev` in the
  extraction ledger at write time. On re-derivation, if the doc's current `rev` moved since
  the recorded one (someone edited it), **refuse to overwrite** — return the item as a new
  outcome (`conflict`, carrying the doc id) instead of clobbering. The ledger already exists;
  this adds a `doc_revs` field beside `doc_ids`.
- **A `derived` flag on the doc record** (open question from the extraction scope) so the UI
  and doc verbs can visibly mark a doc as regenerable output (badge it, warn on edit). Additive,
  serde-default — no migration (mirrors the `content_type` add).
- **A fork-to-editable-copy verb** — the escape hatch: `docs.fork { id } -> new editable doc`
  that snapshots a derived doc into a normal, non-derived doc the user owns and may edit freely
  (breaking the `derived_from` link, or keeping a `forked_from` edge). This is what a user does
  when they *want* to edit; the guard is what protects them when they didn't mean to lose work.

## Non-goals

- Merge/3-way reconciliation of edits against a re-derivation (that is a document-history
  feature, far beyond this).
- Blocking edits to derived docs outright (too rigid — the fork verb is the pressure valve).

## Testing plan

- A derived doc edited (rev bumped), then re-extracted → `conflict`, edits preserved (regression
  for the exact clobber this prevents).
- `docs.fork` produces an editable copy whose later edits survive a re-extraction of the origin.
- The mandatory workspace-isolation + capability-deny tests on the new verb.

## Related

- `doc-extraction-scope.md` — Risks §"Derived-doc identity vs hand edits" names this exact guard.
- `document-store-scope.md` — the `rev` and doc-history machinery this builds on.
