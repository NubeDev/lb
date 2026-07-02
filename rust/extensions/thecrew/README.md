# thecrew — the graphics-canvas extension (plant graphics, floor plans, 3D)

A data-bound three.js scene canvas — the Niagara "PX page" class of UI — being
lifted from a proven standalone playground into a real Lazybones extension:
a federated `[ui]` graphics page + a `[[widget]]` scene cell, scenes persisted as
workspace docs (`assets.*`), live values through the host-mediated bridge.

- **Current ask:** `docs/thecrew-extension-scope.md` (the lift — start here).
- **Parent feature scope:** `docs/scope/frontend/graphics-canvas-scope.md` (repo root).
- **Playground scopes (done, kept as the visual/builder bar):** `docs/README.md`.

Layout:

- `ui/` — the React app (the lifted playground; becomes the federation remote).
- `src/` — the Tier-1 wasm stub component (zero tools; exists because the signed
  publish path requires component bytes — see the extension scope's Intent).
- `docs/` — co-located scopes + phase screenshots (`docs/shots/`).

Not yet buildable as an extension — `extension.toml`, `build.sh`, `Cargo.toml`, and
`src/lib.rs` land with the implementing session per the extension scope.
