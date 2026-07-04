# Dotted `dock.` session ids silently 403 on create-on-post for ordinary members

- Area: frontend / caps
- Date: 2026-07-05
- Session: ../../sessions/frontend/agent-dock-session.md
- Scope: ../../scope/frontend/agent-dock-scope.md

## Symptom

The agent dock's first send did nothing visible: `AgentDock.gateway.test.tsx` sat on
"No messages yet" and every send-then-assert timed out. No error surfaced in the UI — the
post just never landed in the session's history. This reproduced for an ordinary member
signed in with the standard dev-login cap set (which includes `bus:chan/*:pub`).

## Root cause

The dock minted session channel ids as `dock.{user-slug}.{ulid}` (dotted), per the scope's
example. The channel `post` gate checks `bus:chan/{cid}:pub` via the **capability grammar**
(`rust/crates/caps/src/grammar.rs`), which **splits a resource path on BOTH `/` and `.`**:

```
grant   bus:chan/*:pub        pattern segments: ["chan", "*"]
resource chan/dock.ada.01H…   resource segments: ["chan", "dock", "ada", "01H…"]
```

A single `*` matches **exactly one** segment, so it matched `dock` and left `ada`, `01H…`
unmatched → `Decision::Denied`. The create-on-first-post was refused for every ordinary
member (only a `bus:chan/**:pub` recursive grant, which members do NOT hold, would match).
`useChannel.postBody` folds the 403 into `error` and never appends the item — hence the
silent "nothing happened".

## Fix

Make the dock id a **single cap segment**: switch the separator from `.` to `-`
(`dock-{user-slug}-{ulid}`). `-` is not a grammar delimiter, so `chan/dock-ada-01H…` is
`["chan", "dock-ada-01H…"]` and the existing `bus:chan/*:pub` grant matches. No new/wider
cap; the reserved-prefix, UI-only convention is unchanged. `dockId.ts` owns the grammar;
the scope's `dock.` examples were updated to `dock-` (scope "Resolved during implementation"
#1).

Files: `ui/src/features/agent-dock/dockId.ts` (`DOCK_PREFIX = "dock-"`, `mintDockId`,
`dockPrefixFor`), and the picker label split (`DockSessionPicker.tsx`).

## Regression test

`ui/src/features/agent-dock/dockId.test.ts` — `mintDockId` asserts the minted id **carries
no `.` or `/`** ("else `bus:chan/*:pub`'s single `*` can't match the resource segment"). And
`AgentDock.gateway.test.tsx`'s "first send CREATES the dock channel" now passes end to end
against the real gateway for an ordinary member — it would fail (silent 403) for a dotted id.

## Lesson

A channel id is a **capability resource path**, and the grammar treats `.` as a segment
delimiter just like `/`. Any UI-minted channel id that must be covered by a single-`*` member
grant has to stay one segment — no `.`/`/` in the id body.
