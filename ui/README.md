# ui/

> ⚠️ **Reference copy — retained temporarily.** This in-tree shell is no longer the authoritative product
> UI. Product hosts **vendor** their own copy (e.g. `rubix-ai/ui`) and consume the shared `@nube/*`
> packages + `@nube/ext-ui-sdk`. This `ui/` is kept as the reference the vendored shells track, and will
> be removed once the migration is proven. See [`../MIGRATION.md`](../MIGRATION.md).

The frontend — one React + TypeScript codebase, delivered two ways.

- **Edge:** bundled in a Tauri v2 shell, talking to the local node over IPC.
- **Cloud:** served to browsers via the SSE/HTTP gateway.

Stack: React + TypeScript, Tailwind CSS, shadcn/ui + Radix, Tauri v2, Module
Federation for trusted extension UIs (Web Components / iframes for untrusted ones).
See `../docs/key-stack.md`.

Before writing code, read [`../docs/FILE-LAYOUT.md`](../docs/FILE-LAYOUT.md) — the
Frontend (React/TypeScript) section: one component per file, one hook per file,
barrels re-export only, API calls mirror the Rust `tools/<noun>/<verb>`.

Status: not yet scaffolded — architecture scope only.
