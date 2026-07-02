# Slice 2 — vendor `ce-wiresheet` into `packages/ce-wiresheet`

Status: scope slice (S2). Depends on: S1 (the seam branch exists). Parent:
`control-engine-scope.md`.

Copy the `lb-transport` branch of `NubeIO/ce-wiresheet` into this repo as
`packages/ce-wiresheet`, a `workspace:*` package (the `packages/nav-rail` /
`packages/panel` precedent), **byte-identical to the pinned upstream commit**. Because
S1 put the seam upstream and S7 puts the bridge transport in the extension's own UI
folder, this package carries **zero LB-authored code** — vendoring is a snapshot, not
a fork.

## Deliverables

- `packages/ce-wiresheet/` = the branch tree minus repo scaffolding we don't consume:
  keep `src/`, `package.json`, `tsconfig.json`, `vite.lib.config.ts`, `vitest.config.ts`;
  drop `node_modules/`, `memory/`, `index.html`/`standalone.tsx` dev harness,
  `pnpm-workspace.yaml` (we're inside LB's workspace), `.git`.
- `packages/ce-wiresheet/README.md` (LB-authored, the ONE file we own here): the pinned
  upstream commit SHA + branch, the re-sync procedure, and the **approval-gated edit
  rule** — no file in this package is edited in LB; changes go upstream first, then the
  pin is bumped.
- LB workspace wiring: `pnpm-workspace.yaml` already globs `packages/*` (verify);
  package builds and its vitest suite runs under `cd ui && pnpm test` or as its own
  workspace target — pick whichever the other `packages/*` do and match it.
- Trim/park the **agent-panel baggage**: upstream depends on `@opencode-ai/sdk`
  (`ui/AgentChatPanel.tsx`, the `opencode` script). Decide here, once: keep the dep
  (dormant, tree-shaken out of the lib build) or exclude the panel from the lib
  entrypoint upstream. Preference: **exclude upstream on the branch** (the lib build
  shouldn't drag an agent SDK into every consumer); do it as part of the S1 PR if
  cheap, else record as accepted baggage in the README.

## Sync procedure (goes in the package README, verbatim rule)

1. Land the change on `NubeIO/ce-wiresheet` `lb-transport` (or `main` once merged).
2. In LB, with approval: re-copy the tree over `packages/ce-wiresheet` (same drop-list),
   update the SHA in the README, run the package tests + `pnpm test:gateway`.
3. One commit, message `vendor: ce-wiresheet @ <sha>`. Never a partial/file-level sync.

*Rejected:* git submodule (fights the pnpm workspace resolver, lets upstream drift in
unreviewed — per the parent scope's decision). *Rejected:* npm/git dependency (the
package is unpublished/UNLICENSED and we want the source in-tree for review and for the
federated build).

## Testing / exit gate

- The vendored package's own vitest suite (including S1's `MockTransport` test) green
  inside the LB workspace.
- `git diff --no-index <upstream-checkout> packages/ce-wiresheet` shows only the
  drop-list and the LB README — proven once in the session doc, and the check command
  recorded in the README so every future sync can re-run it.
- Nothing outside `packages/ce-wiresheet` imports its internals (`src/lib/*`) — only
  the package root exports. (Grep-test now; it becomes load-bearing in S7.)
- **Exit gate:** LB CI green with the package present; README pin + rule in place.
