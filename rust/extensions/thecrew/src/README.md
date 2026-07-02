# src/ — the Tier-1 wasm stub component

Yes, the extension needs (a little) Rust: the loader has only `wasm | native` tiers
and the signed publish path (`lb-registry` `Artifact.wasm`, verify-before-store)
requires component bytes — there is no UI-only tier, and adding one would be core
surface the graphics canvas explicitly refuses.

So this is a **zero-tool wasm32-wasip2 component** (~20 lines): it implements
`lazybones:ext/extension@0.2.0`, declares no `[[tools]]`, and does nothing at
runtime. proof-panel (`rust/extensions/proof-panel/src/lib.rs`) is the template —
copy its world bindings, drop the tool handlers.

All real behavior lives in `../ui/`. See `../docs/thecrew-extension-scope.md`.
