# Session — dock session-restore (reopen the last conversation on refresh)

Status: done (2026-07-08)
Scope: [`scope/frontend/agent-dock-scope.md`](../../scope/frontend/agent-dock-scope.md) — a small
follow-up enhancement, not a scope open-question closure (the dock scope has none open).

## The ask

"can we in the ui always load/keep the last known agent in memory so we can load it on a page
refresh" — against `ui/src/features/agent-dock/`.

## What already persisted vs. what didn't

Audited the dock's state before changing anything (the gap was narrower than the ask implied):

| State | Where it lives | Survives refresh? |
|---|---|---|
| Dock open + width (`useDockChrome.ts`) | `localStorage` (`lb.agent-dock.open/.width`) | yes |
| Persona **pin** (`personaPin.ts`) | `sessionStorage`, per-tab + per-ws | yes (same tab) |
| Persona context-match (`usePersonaFocus.ts`) | derived live from the route | yes (re-resolves) |
| Active **runtime/definition** (`useAgentCatalog`) | server-side `agent.config.active_definition` | yes |
| **Dock session id** (`useDockSessions.ts`) | in-memory only — `mintDockId` on every mount | **no — refresh minted a fresh empty session** |

So the one real gap was the **session id**: a refresh landed on a brand-new empty dock channel,
even though every past session was still listed in the picker (durable server-side via the eager
`channel.create`). The persona and the active runtime already came back. The fix is to also
remember *which* session was last open.

## Decision — localStorage (the chrome shape), NOT sessionStorage (the persona-pin shape)

The persona pin is `sessionStorage` because **per-tab independence was that scope's headline** (two
members / two tabs are fully independent — `personaPin.ts`'s module doc, and the
`DockPersonaChip.gateway.test.tsx` "tab A never changes tab B" case). A dock **session** is the
opposite: it's a per-USER conversation thread (the channel id is `dock-{user-slug}-{ulid}`), and
sharing it across tabs / restarts is desirable — opening a new tab continues the same conversation,
and the user clicks "New session" to branch. So this mirrors the dock **chrome** shape
(`useDockChrome.ts`'s localStorage effect), not the persona-pin shape.

Rejected alternative: make the persona pin cross-tab too (one user asked "agent" and the pin is the
most "agent"-flavored state). Rejected because it reverses a deliberate, tested scope decision for
no new reason — and the user confirmed (clarifying question) that the session id is what they meant.

**Workspace + user wall:** the storage key is namespaced by BOTH workspace AND user-slug
(`lb.agent-dock.session.{ws}.{user-slug}`), so a shared browser never loads one user's dock
conversation under another's login. The restore also re-validates the id's own prefix
(`isOwnDockChannel`) before using it — belt and braces (the key is already scoped, and the id's
prefix must match too).

## What changed

Three files in `ui/src/features/agent-dock/`:

1. **`dockSessionStore.ts`** (new) — pure localStorage helpers (`readDockSession` /
   `writeDockSession` / `clearDockSession` / `sessionKey`), keyed by `ws` + `principal`. Mirrors
   `personaPin.ts`'s shape one-to-one; the differences are the storage backend (localStorage vs
   sessionStorage) and the added user-slug dimension. FILE-LAYOUT: storage helpers only — no React,
   no IPC.
2. **`dockSessionStore.test.ts`** (new) — 8 unit tests, including the workspace + user isolation
   analog (one user's/ws's session can't be read under another) and the cross-browser (separate
   storage) case. Mirrors `personaPin.test.ts`'s fake-storage discipline.
3. **`useDockSessions.ts`** (modified) — two changes:
   - **Restore on mount**: the `current` lazy-init now reads the store first and uses the stored id
     if it's still this user's own (`isOwnDockChannel`); otherwise mints fresh (the v1 happy path,
     plus the defensive fallback for a future delete or a corrupted value).
   - **Persist on change**: one `useEffect` writes `current` whenever it (or `ws`/`principal`)
     changes — catches the restore, `select`, and `newSession` uniformly (mirrors `useDockChrome`'s
     chrome-persistence effect). No per-call writes.

## Tests (green)

**New unit suite** (`dockSessionStore.test.ts`):
```
$ pnpm exec vitest run --config vite.config.ts src/features/agent-dock/dockSessionStore.test.ts
 ✓ src/features/agent-dock/dockSessionStore.test.ts (8 tests) 3ms
      Tests  8 passed (8)
```

**All agent-dock unit tests** (no regression in the feature):
```
$ pnpm exec vitest run --config vite.config.ts src/features/agent-dock/
 Test Files  10 passed (10)
      Tests  72 passed (72)
```
Includes the new file (8) and the existing `personaPin.test.ts` (7), `dockId.test.ts` (8),
`pendingRun.test.ts` (6), `dockRunState.test.ts` (10), etc.

**Real-gateway regression** (the dock's session-lifecycle + cap-deny + ws-isolation specs, against
a real spawned node — no fake, rule 9):
```
$ pnpm exec vitest run --config vitest.gateway.config.ts src/features/agent-dock/AgentDock.gateway.test.tsx
 ✓ src/features/agent-dock/AgentDock.gateway.test.tsx (10 tests) 1231ms
      Tests  10 passed (10)
```
The "new session mints a SECOND dock channel; the old stays reopenable" and "history restores after
a remount" cases both still pass — the persistence effect writes on every `current` change and the
gateway tests use a unique `ws` per test (`nextWs()`), so the ws-scoped storage key never collides
between tests (more isolated than the existing global chrome keys, which already worked).

**Typecheck + lint:** `tsc --noEmit` exit 0; `eslint` on the three touched files clean (the
pre-existing lint errors elsewhere in the tree are unrelated).

## Capability-deny / workspace-isolation (CLAUDE §5/§7)

This change has **no capability dimension** — it's pure client-side localStorage (no host surface,
no MCP tool, no transport). The workspace + user wall is enforced two ways and tested as the
isolation analog for this UI store: (a) the storage key encodes both `ws` and `user-slug`
(`sessionKey` test pins the namespacing); (b) the restore re-validates the id's own prefix
(`isOwnDockChannel`) before use. The `dockSessionStore.test.ts` "clear drops the session for that
workspace + principal only" case proves one user's/ws's session can't leak to another.

## Debugging

None — nothing broke.

## Follow-ups

- A future v that adds dock-session **delete** should call `clearDockSession` when the current
  session is deleted (v1 has no delete, so the stored id always resolves; the `isOwnDockChannel`
  guard is the defensive fallback).
- If cross-tab *divergence* ever becomes desirable (two tabs each on their own session, like the
  persona pin), the backend switch is one line — move from `localStorage` to `sessionStorage` in
  `dockSessionStore.ts`. Not wanted today (the user explicitly chose the session-id option, whose
  value is cross-tab continuity).
