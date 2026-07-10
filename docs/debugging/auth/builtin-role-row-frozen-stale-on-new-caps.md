# Built-in role row frozen stale on new caps (the reports-demo "create report → denied" footgun)

**Area:** auth (authz-grants) · **Status:** fixed · **Date:** 2026-07-10

## Symptom

The reports feature shipped `mcp:report.save:call` / `mcp:report.export:call` / `mcp:brand.save:call`
correctly in the built-in `AUTHOR_CAPS` bundle, the host + gateway tests were green, but on the live
dev store an admin (`user:ada` in `acme`) opened **Reports → New report** and the save/export verbs
were **denied** — "create report" silently failed. Decoding the login token showed only the VIEWER-tier
report caps (`report.get`/`report.list`/`brand.get`/`brand.list`); the AUTHOR-tier caps
(`report.save`/`report.delete`/`report.share`/`report.export`/`brand.save`/`brand.delete`) were
missing entirely. The `report_test` deny/isolation tests passed (they seed the role rows fresh from
current code), so the gap was invisible to the suite.

## Root cause

**A stale persisted built-in role record + an asymmetry in how `resolve_caps` reads it.**

- `ensure_builtin_authz_roles` → `ensure_one` is **idempotent on absence**: it writes a `role` row
  only when no row exists for that name. The `acme` dev store was seeded BEFORE the `report.*` caps
  were added, so its `member` / `workspace-admin` rows are frozen at the old cap set and are never
  overwritten on a restart (the seed is a no-op once the row exists).
- `resolve_caps` / `resolve_subject_caps` (the fold that mints a token's caps) expands a `role:<name>`
  grant by reading that **stored** record (`role_caps(store, ws, name)`). So a member/admin's author/
  admin caps ride the stale row — and a newly-added built-in cap never reaches an already-seeded
  workspace's tokens.
- The viewer tier **dodged this** only because the login floor (`role/gateway/src/session/credentials.rs`)
  calls the **live** `viewer_role_caps()`; the author/admin caps have no live path — they come solely
  from the stored record.

The throwaway `rust/node/examples/reseed_roles.rs` (delete + re-seed the three rows while the node is
stopped) was a symptom fix for the demo — the trap repeats for every future built-in cap, and it needs
the node stopped (the embedded SurrealKV store is locked by a running node) plus a user re-login
(the browser token is a cached projection).

## Fix (durable) — union live built-in caps in the resolver

The resolver now UNIONS the live built-in bundle on top of the stored record for a granted built-in
role, so a new built-in cap takes effect the moment code ships — no re-seed, no version bump. Because
the resolver lives in the pure `lb-authz` crate (which must not depend on `lb-host`, where the live
`*_role_caps()` bundles live), the live bundles are injected via a `BuiltinRoleCaps` callback:

- `resolve_caps_with` / `resolve_subject_caps_with` / `resolve_caps_sourced_with` /
  `resolve_subject_caps_sourced_with` take the callback (the `_with` entry points).
- The zero-arg `resolve_caps` / `resolve_subject_caps` / `_sourced` twins bake in `NoBuiltinRoleCaps`
  (the raw stored-row fold — unchanged behaviour, used by `lb-authz`'s own tests).
- `host/src/authz/builtin_caps.rs::LiveBuiltinRoleCaps` maps the three built-in names to their
  authoritative `*_role_caps()`. `host/src/authz/resolve_live.rs` exposes
  `resolve_caps_live` / `resolve_subject_caps_live` (the canonical host entry points baking in
  `LiveBuiltinRoleCaps`), and every host caller — the login mint, apikey auth/get, reminder fire,
  dashboard access_check, the access console — goes through them.
- **Union, not replace:** an installed extension's `grant_assign(Subject::Role("member"), cap)` (how
  an ext's page tools reach every member) is still honoured — the live bundle is a floor for built-in
  names; the stored record's additions stay. A **custom** role has no live bundle, so it resolves from
  its stored record exactly as before.

The scope note: `docs/scope/auth-caps/builtin-role-freshness-scope.md` (the invariant + the rejected
alternative). The throwaway `reseed_roles.rs` + its `examples/` dir are deleted — the fix makes them
obsolete.

## Alternative rejected — version the role records

Add a `code_version` to built-in role records; bump a const when caps change; `ensure_one` re-writes
the row when `code_version > stored`. Rejected: it needs a version const maintained alongside every cap
change (easy to forget → silent re-stale), stays stale between bumps unless a login re-seeds, and is a
*repair* of the stored row rather than a guarantee — any path that skips the re-seed re-exposes the bug.
The union is authoritative regardless of the stored row's state and mirrors the already-working viewer
floor; the stored row becomes non-load-bearing for built-in names.

## Regression test

`rust/crates/authz/tests/builtin_role_freshness_test.rs` — pins both halves:
- `live_builtin_caps_union_closes_the_frozen_role_row`: a STALE stored `member` row (missing
  `mcp:report.save:call`) + `resolve_caps` (no builtins) → the cap is MISSING (the pre-fix bug); the
  same store + `resolve_caps_with` (+ live member bundle) → the cap IS resolved (the fix).
- `no_builtin_role_caps_equals_raw_resolve_caps`: the zero-arg path equals `+ NoBuiltinRoleCaps`.
- `custom_role_unaffected_by_builtin_union` + `live_union_keeps_direct_role_subject_grants`.

The existing `sourced_cap_set_equals_resolve_caps_no_drift` cross-check (access-console test) still
passes — the sourced + unsourced folds stay in exact parity (both inject `LiveBuiltinRoleCaps` on the
host path).

## Lesson

Two halves of one role-resolution path went stale at different rates: the viewer tier was live (the
login floor reads the function), the author/admin tiers were frozen (the resolver read the seeded row).
A cap-bundle defined in code but read from a seeded-then-frozen record is a time bomb — the moment a
new built-in cap lands, every already-seeded workspace silently misses it until a manual re-seed. The
durable fix is to make the resolver **authoritative** for built-in names (read the live bundle), not to
repair the stored row — the stored row is a cache that's allowed to drift as long as the live bundle
closes the gap. And a role-GRANT is inert without the role-RECORD it resolves against, *and* that
record must not be the sole source of truth for caps the code defines.

## Related

- Scope: `docs/scope/auth-caps/builtin-role-freshness-scope.md`
- Session: `docs/sessions/reports/reports-finish-session.md` (Task C)
- Prior same-class: `desktop/full-seed-user-missing-admin-caps.md` (a role grant without the role
  record resolves to nothing — the grant-vs-record half of the same model)
