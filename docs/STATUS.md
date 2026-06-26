# STATUS — where the project is right now

The single **"where are we"** dashboard. One screen, always current. Read this first at the
start of any session; update it at the end of any session that changed state.

> This is a **living snapshot**, not a log. It is overwritten in place — it always describes
> *now*, never history. The history lives elsewhere, on purpose:
> - **per-feature narrative** → `sessions/<topic>/…` (the messy middle of each session)
> - **bug history** → `debugging/README.md` (append-only symptom → fix memory)
> - **what shipped** → `public/` (the trimmed source of truth)
>
> So there is **no `LOG.md`** — those three already are the log, each at the right altitude.
> STATUS.md just points at them and says "this is the front line."

---

## Current stage

**S2 complete → entering S3 — multi-node / sync / SSE** (see `STAGES.md`). The Rust workspace +
a React/Tauri UI exist and build; the messaging slice is proven end to end. No doc-site build and
no native desktop window (webkit toolchain) yet.

**S0 exit gate — MET.** `cargo build --workspace` green; CI runs (FILE-LAYOUT size check +
build wasm guest + test + fmt); the four forever decisions (SDK/WIT, capability grammar +
token, job-queue, extension manifest) are written as scope docs.

**S1 exit gate — MET.** A tool call routed through MCP succeeds *with* the grant and is
refused *without* it; a second workspace cannot see the first's data. Through the real WASM
component. See `sessions/core/s0-s1-spine-session.md`.

**S2 exit gate — MET.** Post a message in the UI and it appears (Vitest `ChannelView`); history
survives independent of the bus / a restart (the store keeps it); an extension version swaps live
(hello v1→v2) with state intact. **54 Rust tests + 6 Vitest + 2 shell tests** pass — incl. the
mandatory capability-deny, workspace-isolation (bus + store + inbox), and hot-reload categories.
See `sessions/bus/messaging-session.md` and `public/SCOPE.md`.

**Exit gate (S3):** a second node joins; data syncs edge↔hub; the browser reaches a node over
SSE/HTTP (replacing the S2 in-memory UI fake).

---

## Slices in flight

One row per vertical slice being built. State: `scoped` → `building` → `tested` → `shipped`.

| Slice | Topic | Stage | State | Scope | Session | Notes |
|---|---|---|---|---|---|---|
| Spine | core | S1 | **shipped** | [core](scope/core/core-scope.md) | [s0-s1-spine](sessions/core/s0-s1-spine-session.md) | host+store+bus+caps+mcp+runtime+1 WASM ext; 35 tests green |
| Messaging | bus | S2 | **shipped** | [bus](scope/bus/bus-scope.md) | [messaging](sessions/bus/messaging-session.md) | pub/sub + presence + inbox + channel svc + React/Tauri UI + hot-reload; 54+6+2 tests green |
| Sync / SSE | sync | S3 | scoped | [sync](scope/sync/sync-scope.md) | — | next: second node, edge↔hub sync, browser over SSE |

---

## Scopes authored (ready to build)

The `scope/<topic>/` docs exist for all areas (see `scope/README.md`). **Fully authored:** core,
auth-caps, mcp, crate-layout, extensions, jobs, bus, inbox-outbox, tenancy, frontend, testing,
debugging. **Promoted to `public/`:** core, auth-caps, mcp, crate-layout, bus, inbox-outbox,
tenancy, store, frontend (+ `public/SCOPE.md`). The remaining `public/` and `sessions/` files are
still stubs until their slice ships.

---

## Next up

1. **S3 second node + sync:** a second node joins; make the `mcp/dispatch` routing seam real
   (route to a remote node over a Zenoh queryable); data syncs edge↔hub per §6.8 authority/merge.
   Brings the first **offline/sync** mandatory tests.
2. **SSE/HTTP gateway:** the browser reaches a real node, replacing the S2 in-memory UI fake —
   only the `ui/src/lib/ipc/invoke.ts` seam should change. Push *others'* live messages + presence
   into the UI (`useChannel`'s `setItems` sink is already there).
3. **must-deliver outbox + bus message classification** — once there is a second node to deliver
   to (bus + inbox-outbox open questions).

---

## How to keep this current

Every session that changes state updates the relevant cell here as its **last step**
(`HOW-TO-CODE.md` §3 step 9). Keep it to one screen — if a section grows past a few rows,
the detail belongs in the per-feature docs, not here.
