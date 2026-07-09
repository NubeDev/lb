# Flows debug panel renders "unavailable — source file missing from repo"

**Area:** flows debug panel (UI) + repository hygiene (`.gitignore`)
**Date:** 2026-07-08
**Symptom:** The canvas debug drawer shows "Debug panel unavailable (source file missing from
repo)" instead of the debug sidebar. The panel that shipped with debug-node-scope simply isn't
in the tree — only a stub that unblocks the vite module graph.

## Root cause

The repo root `.gitignore` had a **bare `debug` pattern** (line 3, meant for Cargo's build output),
which matches *any* path segment named `debug` anywhere in the tree. When debug-node-scope created
`ui/src/features/flows/debug/DebugPanel.tsx`, git silently ignored the whole directory: the commit
(`9260f1a`) referenced the panel from `FlowCanvas.tsx`, review read the import as real, but the file
never entered the repo. A later session found the dangling import and left a clearly-marked stub.

Two failures compounded:
1. **The over-broad ignore pattern.** `debug` (unanchored, no trailing `/` scoping) is a foot-gun —
   the Cargo output it targeted already lives under the ignored `target` directory anyway.
2. **Nothing verified the committed tree.** The session tested against the working copy, where the
   file existed; `git status` never listed it (ignored files are invisible by default).

## Fix

- `.gitignore`: replaced the bare `debug` with the anchored `/rust/target/debug/` (redundant with
  `target`, kept as documentation of intent) so a source dir named `debug/` can never be swallowed
  again.
- Rebuilt the panel (flow-ui-polish session): `debug/DebugPanel.tsx` (the sidebar, per-node filter +
  Clear), `debug/useDebugStream.ts` (SSE tail via `openFlowDebugStream`, 200-message ring),
  `debug/DebugMessageRow.tsx` (json/text/markdown type-aware render + `collapseBytes` auto-collapse
  + the `dropped` governor sentinel) — now the right dock's Debug tab.

## Regression guard

`git check-ignore ui/src/features/flows/debug/DebugPanel.tsx` must exit non-zero (it did not,
before the fix). The flows unit suite imports the real panel, so a re-swallowed file fails the next
clean checkout's build — the vite module graph is the tripwire.

## Lesson

After committing, verify **the commit**, not the working tree: `git show --stat HEAD` (or
`git status --ignored` on new directories). A bare directory-name ignore pattern in a monorepo root
is always a bug waiting for a same-named source dir.
