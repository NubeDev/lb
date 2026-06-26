# Core scope

Status: scope. The reusable platform core — goals, non-goals, principles, and the **S1 spine**
this stage builds. This is the umbrella scope; the per-surface decisions live in
`../auth-caps/`, `../mcp/`, `../crate-layout/`, `../jobs/`, `../extensions/`.

> Read with: `../../README.md` (the authoritative spec — sections referenced as §N here),
> `../../STAGES.md` (S0/S1), `../crate-layout/crate-layout-scope.md` (the workspace).

---

## What the core is

A single Rust core providing identity, one multi-model datastore, a real-time bus, an
extension runtime, a capability system, durable workflow primitives, and a shared UI shell.
**Everything else is an extension** (README §1). One binary; edge vs cloud is config and role
(§3.1, §5).

## Goals

- One reusable core; product features arrive as extensions (§2).
- One stack, symmetric nodes — same crates everywhere (§3.1).
- Local-first, offline-capable on a single node; syncs to a hub for teams (§2, §6.8).
- Capability-first security as the *actual product* (§3.5, §11.1).
- Workspace as the hard isolation wall (§3.6, §7).

## Non-goals (v1)

- General multi-master replication (use §6.8 authority partition).
- An `org` tier above workspaces (§7 defers it).
- A microservice mesh — the node is a modular monolith of crates (§2).

## The principles (held in every PR)

The seven principles of README §3 and CLAUDE.md "Non-negotiable rules" govern all core work.
The two with teeth in S1: **capability-first** (nothing reachable except via a host-mediated
check) and **workspace-first isolation** (checked before capabilities). The S1 exit gate *is*
a test of both.

---

## The S1 spine (what this stage builds)

The thinnest vertical slice that proves the capability model end to end (STAGES.md S1):

```
caller ──> mcp ──> caps::check (ws-gate, cap-gate) ──> runtime ──> WASM ext (hello.echo)
                      ▲                                   │
              auth (principal)        store (SurrealDB)  bus (Zenoh)   ← embedded, in-process
```

- **`host`** boots embedded SurrealDB (`store`) and an embedded Zenoh peer (`bus`),
  constructs the `caps` checker over `auth` principals, mounts the `mcp` server, and loads the
  `hello` extension through `runtime`/`ext-loader`.
- **`store`** — one SurrealDB, in-memory namespace per test, `mem://` engine. Workspace =
  namespace (§6.1, §7).
- **`bus`** — one Zenoh peer; keys prefixed `ws/{id}/**` (§6.2). In S1 the bus exists and is
  reachable through a caps-gated publish, but the exit-gate slice is the MCP path; bus gets
  its own slice at S2 (messaging).
- **`caps` + `auth`** — see `../auth-caps/auth-caps-scope.md` (the grammar + token + two-gate
  check are the §13 forever decisions, now fixed).
- **`mcp`** — see `../mcp/mcp-scope.md` (`call → resolve → authorize → dispatch`).
- **`runtime` + `ext-loader`** — load `hello.wasm`, parse its manifest, expose `call-tool`.

**Exit gate (restated as acceptance):** `hello.echo` succeeds *with* `mcp:hello.echo:call`
and is refused *without* it; a workspace-B principal cannot see workspace-A's data. These are
the mandatory deny + isolation tests, present from day one (testing §2).

## How it fits the core

- **Symmetric nodes:** `host` reads config to pick roles; no `if cloud {…}` anywhere in
  `crates/`. S1 runs **solo** (N=1, own authority, offline).
- **One datastore / state vs motion:** `store` and `bus` are distinct, never substituted.
- **Stateless extensions:** `hello` holds no durable state; all state is in `store`/on the bus,
  so it can be killed/recreated (hot-reload safe — proven at S2).

## Testing plan

The mandatory categories (testing §2) anchor here because this is where the spine is wired:
- capability-deny (`mcp` + `caps`),
- workspace-isolation (`store` + `caps`, surfaced through `mcp`),
- (offline/sync and hot-reload categories: N/A in S1 — solo node, no live swap yet; arrive
  S2/S3).

## Open questions (core-level; surface-level ones live in the sub-scopes)

- Single shared SurrealDB instance vs per-workspace instances, and the trigger to split
  (README §13, §11.4) — S1 uses one shared instance with namespace-per-workspace isolation;
  revisit when noisy-neighbor bites (S2+).
- Resource fairness (wasmtime fuel/epoch caps per workspace, §11.4) — wired as a knob in
  `runtime` at S1 but not enforced/tuned until a real workload exists.
- Config format + role selection mechanics for the `node` binary — minimal in S1 (defaults to
  solo); formalize when the second role lands (S3).
