# Built-in role freshness — resolve unions live built-in caps

Status: scope + shipped (a small, durable correctness fix to the authz-grants resolver). Lives
alongside `authz-grants-scope.md` (which owns the roles/resolve model); this is the narrow
"built-in role rows must not go stale" invariant + its fix.

## The problem (the frozen built-in role row)

A built-in role (`viewer` / `member` / `workspace-admin`) is seeded idempotently:
`ensure_builtin_authz_roles` → `ensure_one` **writes a role row only when it is absent**. So a
workspace seeded *before* a new built-in cap was added keeps the stale row forever — its `member` /
`workspace-admin` records are frozen at the cap set the code held on first seed.

`resolve_caps` reads that **stored** record when expanding a `role:<name>` grant. So a new built-in
cap added to `AUTHOR_CAPS` / `ADMIN_ONLY_CAPS` never reaches an already-seeded workspace's tokens
until someone manually deletes + re-seeds the role rows. The viewer tier dodged this only because the
login floor (`role/gateway/src/session/credentials.rs`) calls the **live** `viewer_role_caps()`; the
author/admin caps ride the stored record, so they went stale.

This is exactly what blocked the reports demo: `mcp:report.save:call` / `mcp:report.export:call` /
`mcp:brand.save:call` were correctly in the code but missing from the dev store's seeded `member` /
`workspace-admin` rows, so an admin's token denied "create report" until a throwaway
`reseed_roles.rs` one-shot refreshed the rows. That one-shot was a symptom treatment, not a fix — the
trap repeats for every future built-in cap.

## The fix — union live built-in caps in the resolver

`resolve_subject_caps` / `resolve_caps` (in the pure `lb-authz` crate) now accept a `BuiltinRoleCaps`
callback. When a granted role name is a built-in, the resolver UNIONS the callback's live bundle **on
top of** the stored record. Host callers pass `LiveBuiltinRoleCaps` (the host impl that maps the three
names to `*_role_caps()`); pure-authz callers/tests pass `NoBuiltinRoleCaps` (the raw stored-row fold,
the pre-fix behaviour).

- `resolve_caps_with` / `resolve_subject_caps_with` / `resolve_caps_sourced_with` /
  `resolve_subject_caps_sourced_with` — the `_with` entry points take the callback.
- `resolve_caps` / `resolve_subject_caps` / the `_sourced` twins — the zero-arg entry points bake in
  `NoBuiltinRoleCaps` (unchanged behaviour; used by `lb-authz`'s own tests).
- Host entry points `resolve_caps_live` / `resolve_subject_caps_live` (`host/src/authz/resolve_live.rs`)
  bake in `LiveBuiltinRoleCaps` — the canonical host resolve used by every caller (the login mint,
  apikey auth/get, reminder fire, dashboard access_check, the access console).

So a new built-in cap takes effect the moment code ships — **no re-seed, no version bump, no
migration** — and the displayed access-console set matches the minted token set (the resolver↔mint
cross-check stays exact because both sides inject the same `LiveBuiltinRoleCaps`).

### Why union (not replace), and why custom roles are untouched

Union: an installed extension's `grant_assign(Subject::Role("member"), cap)` — the path by which an
ext's page tools reach every member without editing a built-in record — is still honoured. The live
bundle is a **floor** for built-in names; the stored record's additions stay. A **custom** role has no
live bundle (`live_caps` returns `None`), so it resolves from its stored record exactly as before —
the fix touches only the three built-in names.

## Alternative rejected — version the role records

(a) Add a `code_version` to built-in role records; bump a const when caps change; `ensure_one`
re-writes the row when `code_version > stored`. Rejected because:

- It needs a version const maintained alongside every cap change (easy to forget → silent re-stale).
- It stays stale between version bumps unless a login re-seeds (a login-time write on the hot path).
- It is a *repair* of the stored row, not a guarantee — the resolver still reads the row, so any path
  that skips the re-seed (a fresh store seeded by an older binary, a missed bump) re-exposes the bug.

The union is authoritative regardless of the stored row's state, needs zero maintenance, and mirrors
the already-working viewer floor. The stored row becomes non-load-bearing for built-in names (it can
still diverge — that's fine; the union closes the gap).

## Invariant going forward

**Editing `VIEWER_CAPS` / `AUTHOR_CAPS` / `ADMIN_ONLY_CAPS` is the only change needed** — adding a cap
**or removing one**. It reaches every workspace (seeded or not) on the next token mint. No re-seed
step, no version bump, no release note about "refresh your dev store." The regression test
`crates/authz/tests/builtin_role_freshness_test.rs` pins both directions.

> **Amended 2026-07-16 — union → replace.** As first shipped this said *adding* a cap was the only
> change needed, and it was union-based: the live bundle was a **floor** over the stored row. That
> silently made the invariant false for **removal** — a cap deleted from a built-in bundle kept
> resolving from every stale row, forever, because `ensure_one` is create-only and nothing ever
> rewrites the row. Nobody had needed to remove one, so nothing caught it until a *security* fix had to
> (the member bundle's `mcp:*.list:call` authorized ten admin-only caps — see
> `debugging/auth/member-wildcard-satisfies-admin-cap.md`). The removal would have been inert on
> precisely the deployments that had the bug.
>
> For a **built-in** name the live bundle is now authoritative and the stored record is not read at
> all. This keeps everything the union was for: a direct `grant_assign(Subject::Role("member"), cap)` —
> the extension-install path the union was chosen to protect — resolves through the role-subject
> **recursion**, not the role's record, so it is unaffected. The two were conflated;
> `live_builtin_caps_keep_direct_role_subject_grants` proves they are separable. Custom roles still
> resolve from their stored record exactly as before. The cost: a `roles.define("member", …)` no longer
> changes what a member resolves — the intended posture, since a built-in bundle is lb's policy and an
> admin widening `member` is the escalation this model exists to prevent (the supported path,
> `grant_assign(Subject::Role("member"), cap)`, still works).
>
> The lesson generalizes: **a "freshness" mechanism that can only add is a half-mechanism.** If the live
> definition is authoritative, it must be authoritative in both directions — otherwise the stored row
> stays load-bearing for exactly the changes that matter most: the ones that take power away.

## Related

- Debug entry: `docs/debugging/authz/builtin-role-row-frozen-stale-on-new-caps.md` (the live
  symptom + root cause + the regression note).
- Session: `docs/sessions/reports/reports-finish-session.md` (Task C).
- Parent scope: `authz-grants-scope.md` (the roles/resolve model this narrows).
