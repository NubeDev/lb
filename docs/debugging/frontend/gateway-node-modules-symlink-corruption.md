# `pnpm test:gateway` → `Cannot find module 'vite' / 'vitest'` after a `git worktree` compare

- Area: frontend (tooling / environment — NOT a product bug)
- Status: resolved
- Session: [`../../sessions/external-agent/invoke-default-runtime-session.md`](../../sessions/external-agent/invoke-default-runtime-session.md)

## Symptom

Mid-session, every `pnpm test:gateway <file>` began failing before any test ran:

```
Error: Cannot find module '/home/user/code/rust/lb/ui/node_modules/vitest/vitest.mjs'
...
Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'vite' imported from …/vitest.gateway.config.ts…
```

The Rust build was fine; only the UI test runner's module resolution broke. It looked like a flaky
test failure (the run exited non-zero), which nearly masked it as "the CommandPalette test regressed".

## Root cause

To compare behavior against the pre-change commit I ran `git worktree add <tmp> <sha>` and, to avoid a
second `pnpm install`, symlinked the worktree's `node_modules` at `ui/node_modules`
(`ln -s .../ui/node_modules node_modules` inside the worktree). pnpm then **hardened `ui`'s top-level
dependency symlinks to absolute paths that pointed *through* the temporary worktree**
(`…/scratchpad/pre-change/node_modules/.pnpm/…`). When I later `git worktree remove --force`d that
worktree, those 24 top-level links (`vite`, `vitest`, `react`, `tailwindcss`, …) became **dangling**.

`pnpm install` (even `--force`) reported "Already up to date" and did **not** repair them: the lockfile
hash matched and the `.pnpm` content-addressed store was intact, so pnpm saw no work to do — it does not
re-verify that every top-level symlink still resolves. Hand-repairing individual links was a trap
because the deleted worktree was at a **different commit** with a slightly different lockfile, so some
`.pnpm` dir hashes (e.g. `tailwindcss@3.4.19` vs the app's `@4.3.2`) didn't line up.

## Fix

A clean reinstall from the app's own lockfile:

```bash
cd ui && rm -rf node_modules && pnpm install
```

restores the correct top-level links (`tailwindcss@4.3.2`, `vite@5.4.21`, `vitest@2.1.9`), and
`pnpm test:gateway` runs again.

## Guard / lesson

- **Never symlink one checkout's `node_modules` into another** to skip an install — pnpm bakes absolute
  paths into the top-level links and a later worktree removal leaves them dangling.
- To compare a UI test against another commit, give the worktree its **own** `pnpm install` (or don't
  use a worktree for UI at all).
- When `pnpm test:gateway` fails with `Cannot find module 'vite'/'vitest'`, it is a broken `node_modules`,
  not a test regression: `rm -rf ui/node_modules && pnpm install`. `pnpm install` alone won't fix a
  dangling top-level symlink because it short-circuits on a matching lockfile.
