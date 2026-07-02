# Slice 1 — the `EngineTransport` seam in `ce-wiresheet` (upstream branch)

Status: scope slice (S1 of the control-engine build plan). Repo: **NubeIO/ce-wiresheet**,
NOT this repo. Parent: `control-engine-scope.md`.

The wiresheet today hardwires its transport: `lib/rest.ts` holds a module-level `BASE`
(`setEngineBase()`) and calls `fetch` directly; `lib/ws.ts` owns the WebSocket (session
resume via `sessionStorage`, cross-tab `BroadcastChannel` ownership, reconnect/backoff)
and decodes binary frames via `lib/wire.ts`. There is **no seam** — you cannot point the
editor at anything that isn't a live CE REST origin. This slice cuts that seam **upstream**,
on a branch of `NubeIO/ce-wiresheet`, so the LB bridge transport (S7) becomes an *injection*,
not a fork of the package's core.

## Why upstream, not in the vendored copy (the fork decision, resolved)

We own `NubeIO/ce-wiresheet`. So instead of vendoring `main` and carving the seam inside
LB's copy (a permanent, ever-rebasing divergence — the scope's original "biggest lift"),
we cut the seam as a **branch of the upstream repo** (`lb-transport`), written to be
mergeable into `main` (it is a pure refactor + one new optional prop; standalone behavior
is byte-identical). The LB-specific `BridgeTransport` does **not** live in this branch —
it lives in the LB repo (S7), implementing the exported interface. Consequences:

- the vendored `packages/ce-wiresheet` (S2) stays **byte-identical to an upstream commit**
  — re-sync is a plain re-copy, review is a plain upstream diff;
- upstream `main` can merge the seam whenever it wants (it benefits ce-studio too:
  a mock transport for tests, a recorded-session player, …);
- the approval-gated-edit rule gets easy to hold: LB never edits the vendored files,
  it only bumps the pinned commit.

*Rejected:* carving the seam inside LB's vendored copy — permanent divergence, every
upstream sync is a 3-way merge through our biggest refactor. *Rejected:* a hard GitHub
fork under another org — we own the repo; a branch is the same isolation with none of
the remote juggling.

## The interface (design)

One type, exported from the package root, at the **protocol** altitude (not the HTTP
altitude): callers already speak "typed request + decoded stream", so the seam sits
exactly where `rest.ts`'s `http()` and `ws.ts`'s message dispatch sit today.

```ts
// lib/transport.ts (new)
export interface EngineTransport {
  // request half — replaces rest.ts's internal http(). Typed wrappers
  // (readNodes, addComponent, patchComponent, …) stay in rest.ts and call this.
  request(req: {
    method: "GET" | "POST" | "PATCH" | "DELETE";
    path: string;              // "/nodes/uid/42" — relative to /api/v0
    body?: unknown;
    session?: string | null;   // was header X-CE-Session
    actor?: number | null;     // was header X-Actor-Id
  }): Promise<unknown>;        // the unwrapped `data`; throws RestError

  // stream half — replaces ws.ts's socket ownership. Messages arrive DECODED:
  // wire.ts binary decode is a DirectTransport concern, not a consumer concern.
  openStream(handlers: {
    onSchema(msg: SchemaMessage): void;
    onTopology(msg: TopologyMsg): void;
    onFrame(frame: DecodedFrame): void;          // values/status (was binary)
    onStatus(s: "connecting" | "open" | "closed"): void;
  }): EngineStream;
}

export interface EngineStream {
  setSubscriptions(uids: Set<number>): void;  // the visible-uid diff (subscriptions.ts)
  setTickHz(hz: number): void;
  close(): void;
  readonly sessionId: string | null;          // engine session (undo/redo attribution)
}
```

- **`DirectTransport`** (new file, `lib/transport-direct.ts`): today's behavior, verbatim —
  `fetch` against `setEngineBase`'s origin, the binary WS with session resume, reconnect
  backoff, BroadcastChannel tab-ownership, `wire.ts` decode. All of that machinery is
  *direct-mode-specific* and moves behind this class; a bridge transport needs none of it
  (the LB bridge has its own reconnect story).
- **`CeEditor`** gains an optional prop: `transport?: EngineTransport` (default
  `new DirectTransport()` — zero-change for existing consumers, including standalone dev).
- `setEngineBase` / `wsUrlFromBase` stay exported (they configure `DirectTransport`)
  but are documented as direct-mode-only.

## What must move behind the seam (grounded in the current source)

| Concern | Today | After |
|---|---|---|
| REST origin + `http()` unwrap of `{data}\|{error}` | `rest.ts` module state | `DirectTransport.request` |
| `X-CE-Session` / `X-Actor-Id` headers | `rest.ts` module state | `request.session/actor` fields |
| Socket, reconnect, STABLE_MS backoff | `ws.ts` | `DirectTransport` stream |
| Session resume + BroadcastChannel dup-tab guard | `ws.ts` | `DirectTransport` (direct-only) |
| Binary frame decode | `wire.ts`, called from `ws.ts` | inside `DirectTransport` (stays in `wire.ts`, only the *call site* moves) |
| Subscription set diff → WS subscribe msgs | `subscriptions.ts` + `ws.ts` | `EngineStream.setSubscriptions` |
| Presence (`presence.ts`, rides the WS) | `ws.ts` messages | stream handler `onTopology`-adjacent; **direct-only in v1** — the bridge drops presence (open question in S7) |

`store.ts`, `routing.ts`, `rfbuild.ts`, all components: **unchanged** — they already
consume decoded messages and typed REST wrappers.

## Deliverables

- Branch `lb-transport` on `NubeIO/ce-wiresheet` with: `lib/transport.ts` (interface),
  `lib/transport-direct.ts` (extraction), `rest.ts`/`ws.ts` refactored onto it,
  `CeEditor` prop, exports from `index.ts`.
- A `MockTransport`-driven vitest proving `CeEditor` renders a tree + applies a frame
  with **no network** (this doubles as the interface's conformance spec, and is a real
  in-repo consumer — not an LB fake; it lives upstream where a test transport is a
  legitimate library feature).
- Upstream's existing vitest suite green, `pnpm build` (app + lib) green.
- PR open against upstream `main` (merge when convenient; LB pins the branch commit
  either way).

## Testing / exit gate

- `pnpm test` + `pnpm typecheck` + both vite builds green on the branch.
- Standalone harness (`ce-studio/run.sh`, engine on `:7979`) behaves identically —
  manual smoke: tree loads, drag persists, COV streams, undo works, duplicate-tab
  session guard still kicks in.
- **Exit gate:** `CeEditor` accepts an injected transport; a test renders the editor
  against `MockTransport` with zero `fetch`/`WebSocket` globals touched.

## Open questions (resolve in-slice)

- Does the schema message arrive only on the WS today (bootstrap), and must `request`
  also expose `GET /schema` for the palette? (Both, per `ControlEngine::get_schema` —
  confirm which one the palette actually reads.)
- Undo/redo + changelog endpoints (`X-Actor-Id` addressed): confirm the full list of
  rest.ts wrappers so `request` covers every call site — grep `http(` on the branch.
