# ui/

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
