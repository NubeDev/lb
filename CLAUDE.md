# CLAUDE.md

Guidance for Claude Code (and any AI agent) working in this repository.

## What this is

**Lazybones** is a reusable, extensible backend + frontend platform. A single
Rust core provides identity, a multi-model datastore, a real-time message bus, an
extension runtime, a capability system, durable workflow primitives, and a shared
UI shell. *Everything else is an extension* — chat app, coding-agent workplace,
flow tool, document store, etc.

## Current status: S8 shipped, building on a real workspace

The platform is **built and green**, not scaffolding. The Rust workspace
(`rust/`) and the React/Tauri UI (`ui/`) both compile and are exercised by real
tests. As of **S8 (data plane)** the following are shipped end to end: identity +
signed tokens, the SurrealDB store with workspace isolation, the Zenoh bus, the
WASM + native (Tier-2) extension runtime with a signed registry, the capability
system, durable workflow primitives (inbox/outbox/jobs), generic ingest + tagging,
the AI core (central agent + AI-gateway sidecar), the coding workflow
(issue → triage → approval → job → PR), and the collaboration UI over a real
session. `docs/STATUS.md` is the authoritative "where are we" — **read it first**;
this paragraph is a summary, not the source of truth.

**Real build/test commands** (no longer "do not invent"):
- Rust: `cd rust && cargo build --workspace`, `cargo test --workspace`, `cargo fmt`.
- UI unit: `cd ui && pnpm test`.
- UI against a **real** spawned gateway node: `cd ui && pnpm test:gateway`
  (`test_gateway` bin + `vitest.gateway.config.ts` — no `*.fake.ts`, per rule 9).
- Extensions ship their own `build.sh` (native sidecar + federated UI bundle).

The doc-site (Nextra) and the native desktop window (webkit toolchain) are the
remaining un-built pieces.

## Repository layout

- `rust/` — the Rust workspace: core crates + the `node` binary (see `rust/README.md`).
- `ui/` — the React/TypeScript frontend (see `ui/README.md`).
- `doc-site/` — the Nextra site that publishes the docs (see `doc-site/README.md`).
- `docker/` — container build + compose assets: one `node` image, role by config (see `docker/README.md`).
- `docs/` — all authored documentation (the source of truth).
- `README.md` — the **core stack scope** (the authoritative spec).

## Where docs live

- `README.md` — the core stack scope. Sections are referenced as `§6.5` etc. across
  the docs; keep those references accurate.
- `docs/ABOUT-DOCS.md` — how `docs/` is organized AND **the required rules for AI
  sessions** (see below): `scope/` (the ask) → `sessions/` (the working log) →
  `public/` (what shipped), plus `vision/` (the north star).
- `docs/key-stack.md` — the library/crate stack map (edge vs cloud roles).
- `docs/STAGES.md` — the staged build plan: what to build in what order, node posture,
  and exit gates. Read this to know which stage we're in and what's next.
- `docs/FILE-LAYOUT.md` — **read this before writing any code.** One responsibility
  per file; the project's most important rule for AI-assisted work.
- `docs/vision/` — numbered design notes and worked examples.
- `docs/scope/` — per-area scope docs (auth-caps, jobs, mcp, ai-gateway, …), including
  `scope/testing/` (how to test) and `scope/debugging/` (how to debug + the history system).
- `docs/SCOPE-WRITTING.md` — the playbook for turning a raw feature idea into a complete
  scope setup (doc + stubs + testing plan + index updates). Follow it to write any scope.
- `docs/HOW-TO-CODE.md` — the **coding-session playbook**: the execution counterpart to
  `SCOPE-WRITTING.md`. Follow it to build a scope into shipped code + tests + docs.
- `docs/STATUS.md` — the single **"where are we"** dashboard (current stage, slices in
  flight, next up). Read it at the start of a session; update it at the end.
- `docs/debugging/` — the append-only **working history**: every issue and how it was fixed.

## Every session writes docs, tests, and debug history (required)

Docs, tests, and debug history are deliverables, not optional. Before finishing any task:

- create/update `docs/sessions/<topic>/<name>-session.md`;
- **test the change** in the same session per `docs/scope/testing/testing-scope.md`
  (include the mandatory capability-deny and workspace-isolation tests) and show the
  green output;
- if anything broke, **log a `docs/debugging/<area>/<symptom>.md` entry**, fix it, add a
  **regression test**, and update `docs/debugging/README.md`
  (`docs/scope/debugging/debugging-scope.md`);
- promote anything shipped into `docs/public/` and keep the matching `docs/scope/` open
  questions current.

Full rules and templates: `docs/ABOUT-DOCS.md` → "Rules for AI sessions". A session that
changed things but left no docs/tests/debugging record is **incomplete**.

## Non-negotiable rules

These come from `README.md` §3 and `docs/FILE-LAYOUT.md`. Hold the line on them
in every doc and (later) every PR:

1. **Symmetric nodes.** One binary from shared crates; edge vs cloud is *config
   and role*, never a code branch. No `if cloud { … }` in core crates.
2. **One datastore.** SurrealDB only, embedded on every node. No SQLite/Postgres/
   separate blob service.
3. **State vs motion.** SurrealDB holds state; Zenoh moves messages. Don't use one
   as the other.
4. **Stateless extensions.** No durable state in an extension instance — it lives
   in SurrealDB or on the bus (this is what makes hot-reload safe).
5. **Capability-first security.** Nothing is reachable except through a
   host-mediated capability check. Workspace isolation is checked first, then
   capabilities within it.
6. **Workspace is the hard wall.** Every key is scoped by workspace (= tenant).
7. **MCP is the universal contract.** Capabilities are MCP tools; AI agents, the
   UI, and other extensions all call them the same way.
8. **One responsibility per file.** ≤400 lines hard, ~100 typical. One verb per
   file; folder-of-verbs over file-of-nouns. Never `utils.rs`/`helpers.ts`/
   `common`/`misc`. See `docs/FILE-LAYOUT.md` — it applies to `.rs`, `.ts`, and
   `.tsx` alike.
9. **No mocks, no fake backends.** Nothing that can run in-process gets mocked.
   SurrealDB (`mem://`), Zenoh, capabilities, the gateway, and extensions are
   exercised **for real** — in tests and demos alike. Need data? **Seed real
   records into the real store.** A parallel `*.fake.ts` (or any hand-written
   re-implementation of node behavior) is banned: it lets work *look* done while
   the real path is unbuilt, and an AI can't tell the fake from the truth. Fakes
   are allowed **only** for a true external you cannot run locally (a provider
   HTTP API, GitHub) — behind one trait, in one clearly-named file. See
   `docs/scope/testing/testing-scope.md` §0.

## Conventions for editing docs

- Keep `README.md` section numbers stable; many docs cross-reference them.
- New feature? Start a `docs/scope/<topic>/<name>-scope.md` (the ask). Log agent
  work in `docs/sessions/<topic>/`. Promote shipped truth to `docs/public/`.
- Match the existing voice: practical, architecture-scope friendly, decisive.
  Prefer a recommendation over an exhaustive survey.
- When you make an architectural decision, note the alternative you rejected and
  why — these docs are read to understand *why*, not just *what*.

## Stack (planned)

Rust core (SurrealDB, Zenoh, wasmtime, rmcp) + one React/TypeScript frontend
(Tailwind, shadcn/ui, Tauri v2 on desktop, SSE/HTTP to the browser). See
`docs/key-stack.md` for the full map and what is still `TBD`.
