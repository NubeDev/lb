# A `store:skill/*` grant can't reach a core skill — the `.` in `core.lb-cli` is a segment boundary

**Area:** auth (caps grammar) · **Status:** resolved · **Date:** 2026-07-03

## Symptom

Building the core-skills slice, every host test that granted or loaded a **core** skill under an
admin holding `store:skill/*:write` failed with `AssetError::Denied`:

```
grant_skill(&store, &admin, ws, "core.lb-cli") // admin has store:skill/*:write
  → Err(Denied)
```

The identical flow for a **user** skill (`grant_skill(.., "acme-runbook")`) passed. Only ids with a
`.` in them (the core namespace: `core.lb-cli`, `core.query`, …) were refused.

## Root cause — the caps grammar splits a resource on BOTH `/` and `.`

`authorize_skill` builds the request resource as `skill/{id}` — so for a core skill the resource is
`skill/core.lb-cli`. The caps grammar (`crates/caps/src/grammar.rs::segments_match`) splits a
resource into segments on **`/` OR `.`** (so the mcp surface's `<ext>.<tool>` names work under the
same wildcard rule). That means:

- pattern `skill/*` → segments `["skill", "*"]`
- resource `skill/core.lb-cli` → segments `["skill", "core", "lb-cli"]`

A single `*` matches **exactly one** segment (`core`), leaving `lb-cli` unmatched → no match →
`Denied`. A flat user id like `acme-runbook` is one segment, so `skill/*` covers it and the bug
never surfaced before core skills introduced dotted ids.

The shipped dev-login (`role/gateway/src/session/credentials.rs::member_caps`) granted
`store:skill/*:read|write`, so a **real** admin would have hit this too: they could neither grant
nor load any core skill — the served core-skill path was silently broken.

## Fix — grant the skill surface with `**` (recursive tail), not `*`

The grammar already has the right tool: `**` matches zero or more **trailing** segments. Pattern
`skill/**` → `["skill", "**"]` matches `skill/core.lb-cli`'s tail (`core`, `lb-cli`) AND a flat
`skill/acme-runbook`. So the fix is to grant the skill surface with `**`:

- `role/gateway/src/session/credentials.rs`: `store:skill/*:read|write` → `store:skill/**:read|write`.
- Tests that exercise core ids hold `store:skill/**:...`.

No grammar change (it is a fuzzed invariant — `*` never crosses a segment boundary is *correct*); the
fix is to use the wildcard that spans a multi-segment id. The authorize resource shape (`skill/{id}`)
is unchanged — a dotted id is legitimately multi-segment, and `**` is exactly for that.

## Verification

- `crates/host/tests/core_skills_test.rs` — all 11 tests green (grant/load/catalog for `core.*`).
- `crates/host/tests/core_skills_mcp_test.rs` — the served surface, `store:skill/**` caps.
- Existing `store:skill/*` tests for flat user ids still pass (no regression — `*` still covers a
  one-segment id).

## Prevention

- Any grant that must span an id containing `.` (or `/`) uses `**`, not `*`. The rule: `*` is one
  segment; a dotted/slashed id needs `**`.
- Lesson: the caps resource path is segment-structured on `/` **and** `.`; an id with a `.` in it is
  multi-segment, so a `*` grant silently under-matches it. Grant the *tail* (`**`) when the resource
  embeds a compound id.
