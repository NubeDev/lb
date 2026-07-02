# Session — thecrew: scoping the lift into an LB extension

**Scope produced:** `rust/extensions/thecrew/docs/thecrew-extension-scope.md`
**Parent scope:** `scope/frontend/graphics-canvas-scope.md`
**Date:** 2026-07-02
**Outcome:** scoping session (docs-only, no code/tests changed). The playground —
already built and green (see `thecrew-session.md`) — was moved by the user from
`packages/thecrew/` to `rust/extensions/thecrew/` (app code under `ui/`, an empty
`src/` for Rust); this session scoped turning it into a real extension per
`SCOPE-WRITTING.md`.

## Decisions made (with the checked evidence)

1. **Yes, it needs Rust — but only a stub.** `lb-ext-loader` accepts exactly two
   tiers (`manifest.rs`: `wasm | native`) and `lb-registry`'s `Artifact` requires
   `wasm: Vec<u8>` through the verify-before-store publish path — there is no
   UI-only tier. Adding one would be core surface the graphics canvas refuses, so
   the answer is a **zero-tool wasm32-wasip2 component** (~20 lines, proof-panel
   minus the tool handlers) in `src/lib.rs`.
2. **Parent scope Open question 1 answered:** the `assets.*` surface is shipped —
   scenes = `assets.put_doc`/`get_doc`/`list_docs`, symbol packs (phase 4) =
   `put_asset`/`get_asset`. No `kv.*` fallback needed.
3. **Finding:** `crates/host/src/assets/put_doc.rs` has **no revision check**
   (last-writer-wins) — the parent scope assumed one. Raised as a generic
   `document-store/` ask (`expected_rev` on `put_doc`); interim mitigation named in
   the extension scope's Risks.
4. **The simulator does not lift.** It was the playground's one declared fake,
   legal only with no node present; inside the extension rule 9 applies in full —
   the bridge value source (`series.latest` + `series.watch`) replaces it and tests
   seed real series/docs against the real gateway.
5. **Extension id stays `thecrew`** (the directory the user chose); the rename to
   `graphics-canvas` was considered and deferred as Open question 1 of the new
   scope — decide before first publish, since the id leaks into the served UI route
   and install records.
6. **Out of the pnpm workspace is correct**: `pnpm-workspace.yaml` covers
   `ui` + `packages/*` only; an extension `ui/` is self-contained with its own
   lockfile (the proof-panel pattern) — READMEs updated to say so.

## Files written / updated

- **New:** `rust/extensions/thecrew/docs/thecrew-extension-scope.md` (the scope),
  `rust/extensions/thecrew/README.md`, this session doc.
- **Updated for the move + the new ask:** `rust/extensions/thecrew/docs/README.md`
  and `thecrew-scope.md` (status + `packages/thecrew` → new path),
  `rust/extensions/thecrew/src/README.md` (was a one-liner),
  `rust/extensions/thecrew/ui/README.md` (workspace-member instructions removed),
  `scope/frontend/graphics-canvas-scope.md` (status line, Related, OQ1 answered),
  `public/frontend/graphics-canvas.md` (co-location pointer), `STATUS.md`
  (slices-in-flight row, state `scoped`).

## Testing

N/A — docs-only session; no code or tests changed. The mandatory categories
(capability-deny, workspace-isolation, gateway integration, federation,
hot-reload) are named in the scope's testing plan for the implementing session.

## Open / next

- The implementing session builds per the extension scope: manifest + stub +
  `build.sh` + `mount.tsx`/`scene-io.ts`/`bridge-source.ts`, deletes
  `simulator.ts`, and re-greens the lifted vitest suite + the new gateway tests.
- Raise the `put_doc` `expected_rev` ask on `scope/document-store/`.
- Stale `packages/thecrew` references left as-is (correctly historical) in
  `sessions/frontend/thecrew-session.md`.
