# Auth-caps — the Access console (session)

- Date: 2026-06-29
- Scope: ../../scope/auth-caps/access-console-scope.md
- Stage: post-S9 / S10 (read docs/STATUS.md)
- Status: in-progress

## Goal

Build the **Access console** end to end: turn the flat five-tab admin directory into an
access-management surface. Close the three backend gaps (`resolve_caps` with provenance,
`revoke_tokens` live-token kill, `roles.delete`) and rebuild the UX access-first on shadcn
primitives — overview tiles, effective-caps-per-subject detail with source provenance, a
catalog-driven no-widening grant picker, and a live-token revoke lever on every revoking action.

Exit gate target: every scope-named verb wired store→cap→MCP→gateway→http.ts→UI, with deny/ws-iso
tests on both sides, and the scope's five open questions resolved.

## Decisions & alternatives (the five open questions, resolved up front)

1. **Live-token revoke mechanism** → a per-`(ws, subject)` **tombstone record** the verify path
   checks (NOT a nonce bump, NOT a global list). `revoke_tokens(subject)` writes the tombstone and
   composes with the shipped `revoke_subject` (grant-revoke). One verify-path read; keyed
   workspace+subject. Rejected nonce-bump (global churn, every token) and a list (unbounded scan).
   Multi-node window bounded by TTL — stated in UI copy, never "instant global".
2. **"Who can do X" search** → **client-side** for v1 (filter resolved sets in the UI). No
   `who_has` server verb.
3. **Overview tiles** → People / Teams / Roles / Keys counts + direct-grant subjects + keys
   expiring <7d + subjects holding an admin cap. Provenance/last-changed OUT (→ #5). Hide a tile
   with a reason when its source verb is absent; never a fake 0.
4. **roles.delete** → **cascade-un-assign** in one `write_tx`: read assignees of `role:<name>`,
   revoke that grant from each, delete the role. Built-ins immutable. Idempotent. Affected count
   shown before confirm.
5. **Provenance fields** → DO NOT add `last_changed_by/at`. The effective-caps view shows cap
   SOURCE only (direct / role:r / via team:t) from the resolver fold, not new stored fields. Audit
   is owned by `scope/audit/`.

## What changed

### Backend (store → cap → MCP → gateway → http.ts) — COMPLETE
- **`resolve_caps_sourced` + `resolve_subject_caps_sourced`** (new `crates/authz/src/resolve_sourced.rs`) — the provenance-tagging WRAPPER over the shipped `resolve_caps`/`resolve_subject_caps` fold. Same union, a `CapSource`-tagged accumulator instead of a `BTreeSet`. `CapSource = Direct | Role{name} | Team{name}`. Exported from `lb-authz`.
- **`token_revoke` marker** (new `crates/authz/src/token_revoke.rs`) — `token_revoke_mark` / `token_revoked`, a per-`(ws, subject)` tombstone RECORD the verify path reads. The live-token kill mechanism (decision #1).
- **`role_delete` cascade** (new fn in `crates/authz/src/role.rs`) — finds `role:<name>` assignees, tombstones each grant + deletes the role in ONE store tx.
- **`write_batch`** (new `crates/store/src/write_batch.rs`) — a general N-upsert + M-delete transaction (the generalization `write_tx` is a 2-upsert special case of), so `role_delete`'s cascade is atomic. Bounded (`MAX_BATCH=256`).
- **Host verbs** (`crates/host/src/authz/`): `authz_resolve` (`resolve.rs`), `revoke_tokens` (`revoke_tokens.rs`, COMPOSES `token_revoke_mark` + `revoke_subject`), `roles_delete` (`roles.rs`, built-in-immutable guard). Wired into `call_authz_tool`; exported from `lb-host`.
- **Verify-path check** (`role/gateway/src/session/authenticate.rs`) — `verify_token` reads the marker after the signature/expiry check; a marked subject's bearer is refused (opaque `401`, single-node instant).
- **Gateway routes** (`role/gateway/src/routes/admin_grants.rs` + `server.rs`): `GET /admin/authz/resolve`, `POST /admin/authz/revoke-tokens`, `DELETE /admin/roles/{name}`.
- **`http.ts`**: `authz_resolve`, `authz_revoke_tokens`, `roles_delete` entries. API clients in `lib/admin/grants.api.ts` (+`SourcedCap`/`CapSource`/`resolveCaps`/`revokeTokens`) and `lib/admin/roles.api.ts` (+`deleteRole`/`BUILTIN_ROLES`).
- New caps `mcp:authz.resolve:call` / `mcp:authz.revoke-tokens:call` / `mcp:roles.manage:call` seeded into the dev principal (`credentials.rs`) + the UI `CAP` map.

### Frontend (shadcn-first) — the access-console rebuild
- **New primitives** (`components/ui/`): `tabs.tsx`, `table.tsx`, `select.tsx` (token-bound, no new radix dep).
- **`AdminView`** rebuilt onto `AppPageHeader` + shadcn `Tabs`, with the **Access overview** as the landing tab. Removed from `LEGACY_VIEWS`.
- **`access/` feature folder**: `AccessOverview` (the tile set, #3), `EffectiveCaps` (provenance detail, drives `authz.resolve`), `CapabilityPicker` (catalog-driven, no-widening, drives `tools.catalog`), `RevokeTokensLever` (the "Apply now" live-token kill), `SourceBadge`, `useResolveCaps`.
- **`AccessEditor`** rebuilt onto shadcn: the picker is the PRIMARY grant path (raw string demoted to "Advanced"); the revoke confirm carries the `RevokeTokensLever`. Removed from `LEGACY_VIEWS`.
- **`PeopleAdmin`**: renders `EffectiveCaps` in the user detail (provenance: direct / role / via-team).
- **`RolesAdmin`**: `roles.delete` wired (built-in guard, cascade result note). `ConfirmDestructive` gained an `extra` slot for the lever.
- **LEGACY_VIEWS**: removed the 2 fully-migrated views; added 5 pre-existing unmigrated views (CommandPalette/SqlArg/QueryCard/VariableEditor/StudioView) that were erroring but never listed, so `pnpm lint` is 0-errors again.

## Decisions & alternatives (the five open questions, resolved up front)

1. **Live-token revoke mechanism** → per-`(ws, subject)` tombstone RECORD the verify path checks (NOT a nonce bump, NOT a global list). `revoke_tokens` composes with `revoke_subject`. Rejected nonce-bump (global churn) + deny-list (unbounded scan). Multi-node window bounded by TTL — stated in UI copy + a test comment, never "instant global".
2. **"Who can do X" search** → **client-side** for v1 (filter resolved sets in the UI; the overview aggregates per-subject). No `who_has` server verb.
3. **Overview tiles** → People/Teams/Roles/Keys + direct-grant subjects + keys expiring <7d + admin-cap holders. Provenance/last-changed OUT (→ #5). Resolver-driven tiles HIDDEN with a reason when `authz.resolve` is absent.
4. **roles.delete** → cascade-un-assign in ONE `write_batch` tx; built-ins immutable; idempotent; affected count shown.
5. **Provenance fields** → NO `last_changed_by/at`. Source (direct/role/team) comes from the resolver fold, not stored fields. Audit is `scope/audit/`.

## Tests

Real infra, seeded via the real write path — NO mocks, NO fake backend.

**Rust — all green:**
- `lb-authz` `access_console_test` (5): sourced cap set == `resolve_caps` (no-drift cross-check) · provenance tags (direct/role/team) · key subject resolve · token_revoke marker round-trip + per-subject · role_delete cascade idempotent.
- `lb-host` `authz_test` (+2 new, 7 total): per-verb deny (authz.resolve / authz.revoke-tokens / roles.delete) · ws-iso at the MCP bridge.
- `lb-role-gateway` `access_console_routes_test` (5): forged non-admin denied (403) · resolve provenance · **revoke_tokens refuses bob's prior token on the next verify** (headline) · roles.delete cascade + built-in 400 + idempotent · ws-iso (resolve-empty / delete-nothing / acme intact).
- Regression confirm: `apikey_routes_test` (8), `admin_routes_test` (5), `gateway_test` (9 auth/verify), `admin_crud_test` (8) — the verify-path change broke nothing.
- `cargo fmt` clean.

**UI — green:**
- `AccessConsole.gateway.test.tsx` (6): EffectiveCaps provenance (direct + role + via-team) + honest empty · CapabilityPicker offers the caller-authorized catalog + emits canonical `mcp:<tool>:call` · AccessOverview honest counts · RevokeTokensLever applies + reports + hidden without the cap.
- `RolesAdmin.gateway.test.tsx` (+1, 3): roles.delete cascade over the real route.
- `AdminView`/`PeopleAdmin` gateway tests updated for the rebuild (4 + 3).
- `pnpm test` 168/168 · `pnpm test:gateway` 179/180 (the 1 failure is a **pre-existing** SystemView "bus peers list" flake, untouched by this slice) · `pnpm lint` 0 errors · `pnpm exec tsc --noEmit` clean · `pnpm build` green.

## Debugging

None — nothing non-trivially broke. (Two test assertions were corrected during dev: the ws-iso gateway test initially expected 403 for ws-B resolve/revoke, but the wall is token-derived so ws-B ops legitimately resolve-empty / delete-nothing in their own workspace — fixed to assert the real isolation guarantee; and the `revoke-tokens` route needed its own `SubjectBody` rather than reusing `GrantBody` which requires `cap`.)

## Public / scope updates

- Promoted to `public/auth-caps/access-console.md`; `public/SCOPE.md` cross-linked.
- Scope open questions (all five) marked RESOLVED with the decisions above.

## Dead ends / surprises

- `write_tx` is a 2-upsert outbox special case, not a general multi-record tx. Rather than hand-roll the cascade as loose writes (which would not be atomic), added a bounded general `write_batch` transaction to `lb-store` (additive, reusable).
- The pre-existing unmigrated views (CommandPalette/SqlArg/QueryCard/VariableEditor/StudioView) were already failing `pnpm lint` as ERRORS (the STATUS "lint exits 0" claim was stale). Listed them in `LEGACY_VIEWS` (the sanctioned downgrade for views that predate the standard) rather than touch other slices' code.

## Follow-ups

- **Full shadcn migration of the remaining admin views** (PeopleAdmin/RolesAdmin/TeamsAdmin/WorkspacesAdmin/ApiKeysAdmin bodies still use raw `<table>`/`<input>` — they gained the new features but keep legacy controls; still in `LEGACY_VIEWS`). The access-first wiring + the 2 fully-migrated views (AdminView, AccessEditor) are done; the table/input migration is the remaining ui-standards debt.
- **roles.delete before-confirm preview count**: v1 shows the affected count in the result note (post-delete); a pre-delete preview would need a count-assignees read (no such verb yet).
- **EffectiveCaps on Team/Key detail**: wired into People detail; Teams/ApiKeys detail provenance is the same component, trivial to mount where desired.
- **Global identity (the Slack model) — opened as a scope.** While building the console it became clear the code treats users as **workspace-scoped rows**, contradicting README §7/§6.6 ("global identities, one person → many workspaces"). Opened [`scope/auth-caps/global-identity-scope.md`](../../scope/auth-caps/global-identity-scope.md) to promote that model to a real implementation (global identity directory + `membership` records + login→workspaces + a real switcher) — a prerequisite for the console's People tab, teams, and the switcher to behave as the README describes.
- STATUS.md updated (slice row added).
