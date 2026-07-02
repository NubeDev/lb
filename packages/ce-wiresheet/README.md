# @nube/ce-wiresheet (vendored)

The Control Engine wiresheet editor, **vendored byte-identical** from upstream
`NubeIO/ce-wiresheet`. This package carries **zero LB-authored code** — it is a
snapshot, not a fork. This `README.md` is the only file LB owns here.

## Pin

- **Upstream:** `NubeIO/ce-wiresheet`
- **Branch:** `lb-transport`
- **Commit:** `d0bf28b3eaccf363f712843fa3e7ef36f9906d64`

The `lb-transport` branch carries S1's `EngineTransport`/`EngineStream` seam:
`CeEditor` takes an optional `transport?` prop, `DirectTransport` reproduces the
direct-to-CE behavior, and the LB MCP/Zenoh bridge (S7) is injected from outside
this package (in `rust/extensions/control-engine/ui/`) — never edited in here.

## Approval-gated edit rule (hold the line)

**No file in this package is ever edited in LB.** Changes go upstream first, then
the pin is bumped here. This is not a 3-way merge point; a sync is a full re-copy
at a new SHA. If you find yourself wanting to change a file under `src/`, stop —
the fix belongs upstream on `lb-transport`, and the LB-side bridge/transport work
belongs in the extension's own `ui/` folder.

## Sync procedure (verbatim)

1. Land the change on `NubeIO/ce-wiresheet` `lb-transport` (or `main` once merged).
2. In LB, **with approval**: re-copy the tree over `packages/ce-wiresheet` (same
   drop-list below), update the SHA above, run the package tests + `pnpm test:gateway`.
3. One commit, message `vendor: ce-wiresheet @ <sha>`. Never a partial/file-level sync.

## Drop-list (what we do NOT vendor)

Kept: `src/`, `package.json`, `tsconfig.json`, `vite.lib.config.ts`, `vitest.config.ts`.
Dropped (repo scaffolding / dev harness we don't consume):
`node_modules/`, `memory/`, `.git/`, `dist/`, `dist-app/`, `pnpm-lock.yaml`,
`pnpm-workspace.yaml` (we live inside LB's workspace), `index.html` + `vite.config.ts`
(the standalone app harness) and `src/standalone.tsx` (its entrypoint).

## Verbatim check (re-run on every sync)

```sh
# From the LB repo root, with an upstream checkout at the pinned SHA:
diff -rq <upstream-checkout>/src packages/ce-wiresheet/src   # only: Only in upstream/src: standalone.tsx
for f in package.json tsconfig.json vite.lib.config.ts vitest.config.ts; do
  diff -q <upstream-checkout>/$f packages/ce-wiresheet/$f
done                                                          # all identical
```

The only expected delta is the dropped `standalone.tsx` (dev-harness entrypoint).

## Accepted baggage: `@opencode-ai/sdk`

Upstream's `UiTabHost` side-effect-registers `ui/AgentChatPanel.tsx`, which pulls in
`src/lib/opencode/client.ts` → `@opencode-ai/sdk`. The panel is **dormant** in LB
(no opencode server is run) but it is reachable from `CeEditor`, so the SDK is NOT
tree-shaken out of the lib build. Per the S2 slice decision (slice-2-vendor-wiresheet.md,
"Trim/park the agent-panel baggage"), the preferred fix — excluding the panel from the
lib entrypoint — is an **upstream** change on `lb-transport`; until that lands, the dep
rides along as **accepted baggage**. It is inert at runtime. Do not strip it here (that
would violate the byte-identical rule); bump the pin once upstream excludes it.
