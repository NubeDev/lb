# A person invited into a team reaches none of the team's shares

- Date: 2026-10-06
- Area: auth-caps (`host/src/invites/accept.rs`, the share visibility checks)
- Status: fixed

## Symptom

A person accepted an invite that named a team. The team roster listed them, yet every menu and
board shared with that team was refused to them: `nav.resolve` fell back to the empty menu,
`dashboard.get` answered 403, and `authz.entity_scope` gave them no sites. A person added to the
same team from the team screen (`members.add`) reached everything. Reported by a reviewer on the
waste extension; reproduced on a rubix-ai scratch node (roster `["user:viewer@…", "viewer2@…"]`).

## Root cause

Two spellings of one person, and an exact comparison.

- `invite_accept` wrote the team `member` edge with the BARE email (`relate(…, "member", team,
  bare)`), and discarded the result (`let _ =`). `members.add` stores whatever it is sent; the admin
  console sends `user:<email>`.
- The share checks compared each edge to the principal's sub (`user:<email>`) with `==`:
  `nav/visibility.rs`, `dashboard/visibility.rs`, `assets/visibility.rs` (two places),
  `panel/visibility.rs`, `report/visibility.rs`. `dashboard/share_closure.rs` compared an edge to
  the owner the same way. A bare edge matched none of them.

The capability resolvers already accept both spellings (`lb_authz::edge_is_user`, lb#219); the
share checks never adopted it. Present since invites could name a team (2026-07).

## Fix

- `invite_accept` writes the full sub (`user:<email>`), the form `members.add` receives from the
  console, and returns a failure instead of discarding it. A failure fails the accept, which
  releases the claim, so the invite can be retried.
- Every share check compares through `lb_authz::edge_is_user`, so the bare edges already written by
  earlier accepts keep working with no data migration.

## Regression test

`host/tests/invite_team_share_test.rs`:
- `an_invitee_joins_the_team_as_their_full_sub_and_reaches_its_shares` — invite with a team,
  accept, then the roster holds `user:<email>` and the invitee reads the team's menu, board and
  site;
- `a_bare_member_edge_from_before_the_fix_still_reaches_the_shares`;
- `someone_outside_the_team_is_still_refused`.
