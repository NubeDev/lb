# thecrew extension UI build: `pnpm install` installs nothing (walks up to the repo-root workspace)

- Area: frontend (extension packaging / build.sh)
- Status: resolved
- First seen: 2026-07-02
- Resolved: 2026-07-02
- Session: ../../sessions/frontend/thecrew-extension-session.md
- Regression test: guardrail — `rust/extensions/thecrew/build.sh` (the install line + comment);
  the proof is `./build.sh` emitting `ui/dist/remoteEntry.js` (a plain `pnpm install` produces no
  local `node_modules/.bin/vite`, so the vite build step fails loudly).

## Symptom
Running the extension build (`rust/extensions/thecrew/build.sh`, the proof-panel pattern) — or a
plain `pnpm install` inside `rust/extensions/thecrew/ui/` — reports `Scope: all 3 workspace
projects … Done` but installs **none of thecrew/ui's own dependencies**: `node_modules/.bin/vite`
is absent, and the subsequent `vite build` fails. `pnpm typecheck` fails with `sh: tsc: not found`.

## Reproduce
```
cd rust/extensions/thecrew/ui
rm -rf node_modules
pnpm install --frozen-lockfile      # prints "Scope: all 3 workspace projects"
ls node_modules/.bin/vite           # -> not found
```

## Investigation
The extension `ui/` has its OWN `package.json` + `pnpm-lock.yaml` and lives at
`rust/extensions/thecrew/ui/` — deliberately OUTSIDE the repo-root pnpm workspace (the root
`pnpm-workspace.yaml` globs only `ui` and `packages/*`). But pnpm, run from anywhere under the repo,
discovers the nearest ancestor `pnpm-workspace.yaml` (at the repo root) and treats the install as a
workspace install of the 3 root projects — never installing THIS package. The "3 workspace projects"
line was the tell. proof-panel/ui has the same layout; its build works only because CI happens to run
its install in a context without the ancestor workspace on the path.

## Root cause
A standalone package nested under a directory that has an ancestor `pnpm-workspace.yaml` is captured
by that workspace by default. pnpm's workspace discovery walks UP; there is no implicit "I am my own
root" for a nested package. So `pnpm install` resolves against the root workspace, not thecrew/ui.

## Fix
`build.sh` installs with `--ignore-workspace`, which tells pnpm to ignore the ancestor workspace and
treat the cwd package as its own root:
```
pnpm install --ignore-workspace --frozen-lockfile || pnpm install --ignore-workspace || true
```
(`rust/extensions/thecrew/build.sh`.) The extension's `ui/` deps then install locally and
`./node_modules/.bin/vite build` emits `dist/remoteEntry.js`.

## Verification
`./build.sh` runs clean end to end: the wasm component builds, then `ui/dist/remoteEntry.js`
(1.9 MB, three.js bundled, React externalised) is emitted. `pnpm install --ignore-workspace` inside
`ui/` populates `node_modules/.bin/{vite,tsc,vitest}`; `pnpm test` (50/50) and `pnpm typecheck` pass.

## Prevention
The `--ignore-workspace` flag + an explaining comment in `build.sh` is the guardrail. A `.gitignore`
(`node_modules`, `dist`, the `pnpm-workspace.yaml` stub pnpm may write) was added so build artifacts
never get committed. Follow-up (not blocking): proof-panel/ui has the same latent trap — its
`build.sh` should adopt `--ignore-workspace` too before anyone runs it from inside the repo tree.
Lesson: an extension UI that is intentionally out of the repo workspace must install with
`--ignore-workspace`, or pnpm silently installs the wrong project.
