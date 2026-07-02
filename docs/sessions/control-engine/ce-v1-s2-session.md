# Session: control-engine v1 — S2 (vendor `ce-wiresheet`)

Branch `ce-v1`. Slice doc: `rust/extensions/control-engine/docs/slice-2-vendor-wiresheet.md`.

## What shipped

Vendored `NubeIO/ce-wiresheet` @ `d0bf28b3eaccf363f712843fa3e7ef36f9906d64` (branch
`lb-transport`) into `packages/ce-wiresheet` as a `workspace:*` sibling (the
`packages/nav-rail` / `packages/panel` precedent). **Byte-identical** to the pinned
upstream commit for every kept file; zero LB-authored code inside the package except
the LB-owned `README.md` (pin + sync procedure + approval-gated edit rule + verbatim
check command) and a `.gitignore` (dist/node_modules, mirroring nav-rail).

**Kept:** `src/`, `package.json`, `tsconfig.json`, `vite.lib.config.ts`, `vitest.config.ts`.
**Dropped (drop-list):** `node_modules/`, `memory/`, `.git/`, `dist/`, `dist-app/`,
`pnpm-lock.yaml`, `pnpm-workspace.yaml`, `index.html` + `vite.config.ts` (standalone app
harness) and `src/standalone.tsx` (its entry).

## Verbatim proof

```
diff -rq ~/code/ce/ce-wiresheet/src packages/ce-wiresheet/src
  → Only in ~/code/ce/ce-wiresheet/src: standalone.tsx   (the only delta — dropped harness)
package.json / tsconfig.json / vite.lib.config.ts / vitest.config.ts → all identical
```

## Tests / exit gate

- `cd packages/ce-wiresheet && pnpm test` → **21 files, 145 passed** (incl. S1's
  `src/lib/transport.test.tsx` MockTransport — the seam renders + applies a frame with no
  `fetch`/`WebSocket` globals touched).
- `pnpm build:lib` → green (ESM+CJS+css+types emitted; chunk-split warnings are upstream
  advisories, not errors).
- Grep: nothing outside the package imports its internals (`@nube/ce-wiresheet/src` /
  `.../src/lib`) — becomes load-bearing in S7.
- `pnpm install` → 5 workspace projects resolved (package picked up by the `packages/*` glob).

## Open questions (resolved in-slice)

- **Agent-panel baggage (`@opencode-ai/sdk`):** `UiTabHost` side-effect-registers
  `AgentChatPanel`, which pulls `lib/opencode/client.ts` → `@opencode-ai/sdk`; it is
  reachable from `CeEditor`, so NOT tree-shaken. Byte-identical rule forbids stripping it
  in LB. **Decision:** record as **accepted baggage** (dormant at runtime — no opencode
  server is run); the preferred exclusion is an upstream change on `lb-transport`, then a
  pin bump. Documented in `packages/ce-wiresheet/README.md`.

## Notes for S7

The lib entry (`src/index.ts`) already exports `CeEditor` + the transport seam
(`EngineTransport`, `EngineStream`, `DirectTransport`, …). S7's LB `BridgeTransport` is an
`EngineTransport` implemented in the extension's own `ui/` folder and injected via
`CeEditor`'s `transport` prop — never edited into this package.
