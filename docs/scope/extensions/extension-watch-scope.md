# Extension-watch scope — extensions can contribute a live `watch` tool

Status: scope (the ask). Promotes to `public/extensions/` once shipped.

Today only **core** surfaces stream: `series.watch`, `bus.watch`, and the channel feed each have a
gateway SSE route and a `bridge.watch` path. An **extension** can contribute request/response tools
(`<ext>.<tool>` over the frozen `tool.call`) but has **no way to contribute a live feed** — its only
option is to bridge data onto the core `series` plane via `ingest.write` and lean on `series.watch`.
That asymmetry quietly breaks rule 7 ("MCP is the universal contract" — extensions are equal
citizens) and pushes *motion* through the *state* plane (rule 3) purely to stream it. This scope
closes the gap: a **generic** contract by which an extension declares a tool as **streaming**, the
host relays it as SSE, and `bridge.watch`/the CLI/agents consume it — with the WIT boundary
**unchanged** and the host holding **zero** per-extension knowledge. It is the streaming twin of the
one-shot extension tool, and the primitive `control-engine`'s `ce.watch` (and future device/presence
extensions) build on.

## Goals

- Let any extension declare a `[[tools]]` entry as **streaming** and have the platform surface it as
  a live feed over the **same** two consumers core watch tools already use: the gateway SSE route
  and the federated-UI `bridge.watch`.
- **No new WIT world / no ABI change.** Streaming rides the **bus**, not the tool return — the tool
  call returns a *subscription descriptor*; frames flow on a host-allocated, workspace-scoped subject
  the extension publishes to (arm-on-subscribe / disarm-on-idle, the `mqtt` precedent).
- **One generic mechanism**, no per-extension host code: a single SSE relay + a single subject
  allocator, keyed by `(ws, ext, tool, args)` — identical for every extension, works across the
  routed cross-node hop (so a remote appliance's feed relays transparently).
- Gate the feed with the extension tool's own cap (`mcp:<ext>.<tool>:call`, read semantics),
  workspace-first — the same authorize-before-dispatch chokepoint as a one-shot call.

## Non-goals

- **No persistence.** A watch tool is motion only; history/replay stays the `series` plane's job
  (an extension that wants both bridges to `series` *and* streams — orthogonal).
- **No new streaming WIT ABI.** We deliberately avoid a component-model stream type on the stable
  boundary; the feed is bus motion + a descriptor, so the frozen `tool.call` world is untouched.
- **Not a pub/sub free-for-all.** An extension cannot publish to arbitrary subjects; it gets exactly
  the one host-allocated subject for the armed subscription, and only while subscribers exist.
- Not changing how core watch tools (`series.watch`, `bus.watch`) work — this generalizes their
  shape, it doesn't reimplement them.

## Intent / approach

**A watch tool is a one-shot tool that returns a subscription, and streams on the bus.** An extension
marks a tool streaming in the manifest (`[[tools]] kind = "watch"`). Calling it does **not** return a
value payload — it returns a **subscription descriptor** `{ subject, codec }` where `subject` is a
workspace-scoped bus key the **host allocates** for `(ws, ext, tool, args-hash)`. On first subscriber
the host tells the extension to **arm** (open its upstream, start publishing frames on `subject`); on
the last unsubscribe it tells it to **disarm** (the `mqtt` `arm`/`disarm` lifecycle, generalized).
Two consumers relay that subject, reusing the shipped machinery:

- **Gateway SSE** — a single generic route `GET /ext/{ws}/{ext}/{tool}/stream?args=…` authorizes
  `mcp:<ext>.<tool>:call` (workspace-first), resolves/allocates the subject, subscribes, and relays
  frames as SSE — mirroring `series_stream.rs`, but parameterized by `(ext, tool)` instead of a
  hardcoded series id.
- **`bridge.watch`** — the federated-UI bridge already exposes `bridge.watch(tool, args)`; today it
  only accepts core watch tools. It gains the ability to watch an `<ext>.<tool>` marked streaming,
  gated by the page's install scope ∩ grant (the same `ui_decl::narrow` filter as `bridge.call`).

**Why bus-not-return (the key idea).** Returning a stream through the WIT tool call would force a new
streaming world on the stable ABI — a forever cost (README §11.2). Returning a *descriptor* and
streaming on the bus keeps the ABI frozen, reuses Zenoh for motion (rule 3), and makes the **routed
cross-node hop free**: a remote extension arms a publisher on a workspace key expression Zenoh already
carries to the cloud gateway, so a remote feed relays with no extra plumbing. The extension never
touches a socket the host doesn't mediate.

**Rejected — a streaming WIT ABI** (component-model `stream<T>` on the tool boundary): permanent ABI
surface, no cross-node story, and every extension author pays for it. **Rejected — status quo
(bridge-to-series-only):** persists motion as state, adds write churn, and leaves extensions second
class. **Rejected — let extensions open raw SSE/WebSockets:** a non-MCP, un-gated transport hole (the
same reason `control-engine` rejected a raw CE proxy).

## How it fits the core

- **Capabilities.** The feed is gated by the extension tool's own `mcp:<ext>.<tool>:call` (read
  semantics), checked workspace-first at the SSE route open **and** re-checked when the subject is
  allocated — authorize-before-dispatch, identical to a one-shot call. A denied caller gets no
  subject and no frames.
- **Tenancy / isolation.** The subject is `ws/{id}/ext/{ext}/{tool}/{sub}` — workspace-walled by
  construction; a ws-B caller can neither open the route nor guess a ws-A subject (a mandatory
  isolation test).
- **Symmetric nodes.** The arm/publish happens on whichever node owns the upstream (local or a routed
  appliance); the relay happens on whichever node the browser hit. No `if cloud` — the subject is the
  same key expression on every node, and Zenoh routes it.
- **MCP surface (API shape §6.1 — the live-feed verb).** This *is* the live-feed category made
  available to extensions. Manifest: a `[[tools]] kind = "watch"` marker (default one-shot). Host: a
  new generic gateway SSE route + the subject allocator/arm-disarm supervisor. No new *tool* verbs —
  the extension's own declared tool is the verb.
- **Data (SurrealDB).** None by default (motion only). Arm/disarm state is runtime, not durable
  (stateless-extension rule): the subscriber count lives in the host's live map, not the store.
- **Bus (Zenoh).** The whole feed: fire-and-forget frames on the allocated subject. Must-deliver work
  is still the outbox's job — a `watch` is explicitly best-effort live (a late subscriber sees frames
  from *now*, not a replay; replay is `series`).
- **SDK/WIT impact.** **The point is that there is none** on the WIT world — but this **does** add
  generic host surface (the manifest `kind` field, the SSE route, the arm/disarm supervisor). Flag it:
  the manifest schema and the `ext-loader` gain a streaming marker; the gateway gains one route.

## Example flow

1. An extension manifest declares `[[tools]] name = "watch" kind = "watch"` (gated
   `mcp:<ext>.watch:call`).
2. A federated page calls `bridge.watch("<ext>.watch", { … })`. The shell opens
   `GET /ext/{ws}/<ext>/watch/stream?args=…`.
3. The gateway authorizes `mcp:<ext>.watch:call` (workspace-first), allocates
   `ws/{ws}/ext/<ext>/watch/{h}` for the args-hash `h`, and — first subscriber — calls the extension's
   `watch` tool with an `arm` intent. The extension opens its upstream and starts publishing frames on
   the subject.
4. The gateway subscribes to the subject and relays each frame to the browser as an SSE event.
5. The page unmounts → the SSE closes → last subscriber gone → the host calls the extension to
   **disarm**; the upstream closes. No leaked publisher.
6. A **remote** upstream (an appliance) is identical: step 3's `arm` routes to the appliance node,
   which publishes on the same key expression; the cloud gateway's subscription (step 4) receives it
   over Zenoh with no extra code.

## Testing plan

Per `scope/testing/testing-scope.md`:

- **Capability deny (mandatory).** Open the SSE route / `bridge.watch` a streaming ext tool without
  `mcp:<ext>.<tool>:call` → denied, no subject allocated, no frames.
- **Workspace isolation (mandatory).** A ws-B caller cannot open ws-A's feed nor receive its frames;
  the subject is unguessable and the route re-checks the workspace claim.
- **No mocks (CLAUDE §9).** Exercise a **real** streaming extension (extend the `proof-panel` or
  `echo-sidecar` reference with a `kind="watch"` tool that emits a counter) against a **real** spawned
  gateway node: `pnpm test:gateway` asserts frames arrive over real SSE; a two-node test asserts the
  **routed** arm + cross-node relay works on a real bus.
- **Arm/disarm lifecycle.** First-subscriber arms exactly once; last-unsubscribe disarms; a mid-stream
  subscriber drop doesn't kill other subscribers; assert no leaked publisher after disarm.
- **Hot-reload.** Subscriber count is runtime state — a sidecar restart re-arms on the next subscribe;
  no durable state lost.

## Risks & hard problems

- **Backpressure / fan-out.** A high-rate feed with a slow SSE client — define the drop policy
  (latest-wins vs bounded buffer) at the relay, per subscriber. The bus already lossy-drops; make the
  SSE relay match, and document it.
- **Arm/disarm race.** Rapid subscribe/unsubscribe churn must not thrash the upstream; debounce
  disarm (a short linger) so a page remount doesn't re-arm from scratch.
- **Args-hash keying.** Two callers with identical args must **share** one armed upstream (one CE COV
  subscription, N SSE relays); different args get different subjects. Get the canonical args hash
  right or you fan out upstreams needlessly.
- **The manifest `kind` marker is stable surface.** It's a small addition to the forever-ish manifest
  contract (`extensions-scope.md`) — specify it once, carefully.

## Open questions

- **Marker shape:** `[[tools]] kind = "watch"` vs a boolean `stream = true` vs a separate
  `[[watch]]` block. Lean `kind = "watch"` (extends the existing tool entry, one list to read).
- **Descriptor vs implicit:** does the tool return an explicit `{ subject, codec }` the client passes
  back, or does the gateway route hide the subject entirely (client only ever names `<ext>.<tool>`)?
  Lean *hidden* — the client names the tool; the host owns subject allocation end to end.
- **Codec negotiation:** JSON frames only in v1, or a `codec` field (JSON | binary) for high-rate
  feeds (CE COV is binary upstream)? Lean JSON v1, `codec` reserved.

## Related

- README `§3` rule 3 (state vs motion), rule 7 (MCP is the contract), `§6.13` (gateway SSE),
  `§11.2` (the WIT ABI is a forever boundary — why we keep streaming *off* it).
- `scope/extensions/extensions-scope.md` (the manifest contract this adds `kind` to),
  `ui-federation-scope.md` (`bridge.watch`), `native-tier-scope.md` (arm/disarm lifecycle via `mqtt`).
- `scope/query/` / `series.watch` + `bus.watch` (the core watch tools this generalizes).
- **Consumer:** `rust/extensions/control-engine/docs/control-engine-scope.md` — `ce.watch` is the
  first real tenant (live CE COV); its scope names this primitive as the primary live-feed path with
  a series-bridge fallback.
