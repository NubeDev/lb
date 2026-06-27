# Workspace - standalone docs backfill (session)

- Date: 2026-06-28
- Scope: ../../scope/workspace/workspace-scope.md
- Stage: S7 collaboration/admin CRUD docs backfill
- Status: done

## Goal

Create first-class workspace docs because the shipped code had only broad collaboration/admin docs.
Document the implementation as it exists: session workspace from token, reserved namespace directory,
switcher create/list, admin archive/rename/purge, and tests.

## What changed

- Added `scope/workspace/workspace-scope.md` as the topic brief reconstructed from the shipped code.
- Added `public/workspace/workspace.md` as the durable shipped source of truth.
- Added this session note to record the code review and doc-only change.
- Updated indexes/status/vision to make the topic discoverable.

## Decisions & alternatives

- Split workspace into its own topic instead of leaving it under frontend collaboration because the
  code now spans host services, gateway routes, UI switcher, and admin lifecycle.
- Called out the reserved `_lb_workspaces` namespace explicitly. It is the main distinction from
  normal workspace-owned data.
- Documented purge as a tombstone, not immediate namespace garbage collection, because that is what
  the code ships.

## Tests

Docs-only change. No code tests were run.

Code reviewed for the doc:

- `rust/crates/host/src/workspaces/`
- `rust/role/gateway/src/routes/workspace.rs`
- `rust/role/gateway/src/routes/admin_workspaces.rs`
- `ui/src/lib/workspace/`
- `ui/src/lib/admin/workspaces.api.ts`
- `ui/src/features/workspace/`
- `ui/src/features/admin/WorkspacesAdmin.tsx`
- `rust/crates/host/tests/collaboration_test.rs`
- `rust/role/gateway/tests/gateway_test.rs`
- `ui/src/features/admin/WorkspacesAdmin.gateway.test.tsx`

## Debugging

None.

## Public / scope updates

Promoted to `public/workspace/workspace.md`. Added cross-links from the docs indexes, `STATUS.md`, and
the coding-agent workplace vision note.

## Dead ends / surprises

The previous durable docs were accurate but bundled workspace into `public/frontend/collaboration.md`.
That made the code hard to discover for backend or admin work.

## Follow-ups

- Workspace provisioning is still separate from directory creation.
- Real IdP work remains behind the existing session seam.
- Store-level garbage collection after purge remains a follow-up.
