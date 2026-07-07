# nav share-roster session — add/remove team shares (`nav.unshare` + `nav.list_shares`)

Status: **SHIPPED (2026-07-07)** — backend + UI green; 3 new Rust tests + 1 new UI gateway test
added under the existing nav suites.

## Why

The nav builder (`ui/src/features/admin/nav/NavAdmin.tsx`) could **set** a nav's visibility tier and
write **one** `share` edge via `nav.share`, but had no way to **enumerate** the live team shares or
**revoke** one — the backend exposed no `unshare`/`list_shares` verbs, and the UI therefore rendered
a single free-text `team:ops` field with no roster and no remove button. This closed that gap end to
end.

## What shipped

**Backend** (`rust/crates/host/src/nav/`):

- `unshare.rs` → `nav_unshare(store, principal, ws, id, team, now)` — owner-only, idempotent;
  calls the shipped S4 `lb_assets::unrelate` to tombstone the `nav -[share]-> team` edge. Bumps
  `updated_ts` so an LWW peer observes the revoke (state is append-style, §6.8). Gated
  `mcp:nav.share:call` — the **inverse write under the same cap**, no new grant.
- `list_shares.rs` → `nav_list_shares(store, principal, ws, id) -> Vec<String>` — owner-only;
  returns the live `share` edge targets via `lb_assets::list_related` (the exact set gate-3 walks).
  Gated `mcp:nav.share:call`.
- Wired both into the MCP bridge (`tool.rs`: `nav.unshare`, `nav.list_shares`), the host re-exports
  (`lib.rs`), and the gateway (`routes/nav.rs`: `POST /navs/{id}/unshare`,
  `GET /navs/{id}/shares`; registered in `server.rs`).

**Frontend** (`ui/src/features/admin/nav/`):

- `nav.api.ts` → `unshareNav(id, team)`, `listNavShares(id)`; routed through `lib/ipc/http.ts`.
- `useNavs.ts` → tracks `shares` state, exposes `unshare` + `reloadShares`; reloads after every
  share/unshare so the builder's roster stays in sync with the resolver.
- `NavAdmin.tsx` → renders a **share roster** section under the editor: every team the nav is
  currently shared to, each with a **Remove** button. Empty-state copy distinguishes "no teams at
  the Team tier" from "not at the Team tier". Clears on Back/New, loads on Edit/Save.

## Decisions

- **One cap, not two.** `nav.unshare` and `nav.list_shares` both gate on `mcp:nav.share:call`
  (the existing write cap), not a new grant. Rationale: `unshare` is the *inverse write* of `share`
  (you don't grant someone the right to share but not unshare), and `list_shares` is the read the
  builder needs to render the very form `share`/`unshare` drive — splitting it would force authors
  to hold three caps to manage one roster. Rejected: a separate `mcp:nav.unshare:call` — needless
  cap-surface growth, no security gain (same owner-only check inside both verbs).
- **Owner-only `list_shares`.** A same-workspace peer who can *read* a team-shared nav already sees
  the nav in `nav.list`; exposing *which other teams* it's shared to a peer editor would leak team
  existence. So `list_shares` (like `unshare`) checks `nav.owner == principal.sub()` after the cap
  gate. Tested.
- **No per-user share tier.** A nav's visibility model stays `private | team | workspace` — there is
  no `user` axis and this work adds none. To give one user a nav, share to a team and put them in it
  (managed via the existing `teams.add_member`/`teams.remove_member`, already wired in `TeamsAdmin`).
  This preserves the design intent (nav scope non-goal: "No new authorization system") and avoids
  widening the asset's ACL substrate.
- **No "flip tier on last unshare".** Revoking the last team share leaves the nav `visibility:team`
  but edgeless — the resolver then denies everyone but the owner, exactly like a dashboard with no
  live `share` edges. The owner flips the tier explicitly via `nav.share` if they want a different
  default reach. Auto-flipping would be a surprising side effect of a remove.

## Tests (real, no mocks — rule 9)

**Rust** (`crates/host/tests/nav_test.rs`, +3 tests, 14 total green):

- `share_roster_lists_and_revokes_team_shares` — share to two teams, roster shows both, unshare
  one, surviving team's member still resolves, revoked team's member falls through to the fallback
  + direct `nav.get` denied; re-unshare idempotent.
- `unshare_and_list_shares_denied_without_cap` — both verbs denied without `mcp:nav.share:call`;
  the share edge survives (no mutation on deny).
- `list_shares_and_unshare_owner_only_and_workspace_walled` — a same-ws non-owner is denied
  (`Denied`); a cross-ws caller reaching into ws-A reads as `NotFound` (no existence signal); ws-A's
  share is untouched.

**UI** (`ui/src/features/admin/nav/NavAdmin.gateway.test.tsx`, +1 test, 5 total green) —
`lists and removes team shares through the builder`: drives the real `/navs/{id}/unshare` and
`/navs/{id}/shares` routes through the builder UI — two teams seeded, roster renders both, Remove
button clicked, surviving team's member resolves, revoked team's member falls to fallback.

## Files

Backend:
- `rust/crates/host/src/nav/unshare.rs` (new)
- `rust/crates/host/src/nav/list_shares.rs` (new)
- `rust/crates/host/src/nav/mod.rs`, `tool.rs` (wiring)
- `rust/crates/host/src/lib.rs` (re-exports)
- `rust/role/gateway/src/routes/nav.rs`, `routes/mod.rs`, `server.rs` (routes)
- `rust/crates/host/tests/nav_test.rs` (tests)

Frontend:
- `ui/src/lib/nav/nav.api.ts` (client)
- `ui/src/lib/ipc/http.ts` (routing)
- `ui/src/features/admin/nav/useNavs.ts`, `NavAdmin.tsx`, `NavAdmin.gateway.test.tsx`

## Verification

- `cd rust && cargo test --package lb-host --test nav_test` → 14 passed.
- `cd ui && pnpm test:gateway src/features/admin/nav` → 5 passed.
- `cd rust && cargo build --workspace` → green.
- `cd ui && pnpm build` → green.
- `cd rust && cargo fmt` → applied.

## Related

- Scope: `docs/scope/nav/nav-builder-scope.md` (the ask — `nav.share` was the only share verb
  named; this adds the missing inverse + the roster read the builder needed).
- Public: `docs/public/nav/nav.md` (the shipped truth — to be updated separately to name
  `nav.unshare`/`nav.list_shares`).
