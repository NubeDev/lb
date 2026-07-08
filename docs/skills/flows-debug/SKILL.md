---
name: flows-debug
description: >-
  Tail a Lazybones flow's debug stream over the node gateway — open the live SSE feed a `debug` node
  publishes onto, and watch wire messages stream past (json/text/markdown, attribution + a collapse
  hint). Use when a task says "watch a flow's debug output", "tail the debug stream", "see what a
  debug node is emitting over the API", or "call flows.debug.watch over MCP/REST". Covers the SSE
  route + the event shape + the publish-governor sentinel, grounded in a live run.
---

# Tailing a flow's debug stream over MCP / REST

A `debug` node dropped on a wire publishes each message as **motion** onto a workspace-walled,
per-flow subject. The browser debug panel tails it; an agent or script can tail the same stream over
the HTTP gateway. This is the Node-RED debug-sidebar posture over our durable per-node engine — see
`scope/flows/debug-node-scope.md`.

The gateway exposes the stream one way:

- **A live SSE route** — `GET /flows/{id}/debug/stream?token=<jwt>`. Open once with `EventSource`
  (or `curl -N`); receive one `event: debug` frame per published message. **Deltas-only in v1**
  (motion-only — no snapshot, no replay): a late opener sees messages from attach onward.

The stream is **not** a JSON dispatch verb — there is no `POST /mcp/call` form (a live feed has no
single JSON answer). The workspace + principal come from the `?token=` query param (EventSource
can't set headers). Required capability (held by the default **member** cap set):
`mcp:flows.debug.watch:call`. The `debug` node itself needs **no** cap to publish — it runs inside a
`flows.run` (already gated by `mcp:flows.run:call`).

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

## 2. Author a flow with a `debug` node

A `debug` node is a built-in sink (`kind = sink`, one `payload` in, no out — a terminal observer).
Drop one on any wire; it never gates a subtree. Author the flow through `flows.save`
(see `skills/flows-mcp/SKILL.md`), then run it:

```bash
curl -s -X POST http://127.0.0.1:8080/flows/cooler/run \
  -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{}'
# → {"run_id":"01H…"}
```

Each firing of the debug node publishes the wire message onto `flow_debug:{ws}:cooler`.

## 3. Tail the stream

```bash
curl -N "http://127.0.0.1:8080/flows/cooler/debug/stream?token=$TOKEN"
```

Each frame is an SSE `event: debug` whose data is one JSON message:

```jsonc
// a real wire message
{
  "kind": "debug",
  "node": "d-2",                 // the debug node id
  "runId": "01H…",               // the run that fired it (attribution; the stream is per-flow)
  "ts": 1752000000,
  "format": "json",              // json | text | markdown — resolved HOST-SIDE from the node's config
  "value": { "temp_c": 23.4 },   // the wire payload (always full; collapse is presentation-only)
  "label": "scaled temp",
  "collapseBytes": 2048          // the node's hint to the renderer
}

// the publish-governor sentinel (a hot source exceeded `rate_limit`)
{ "kind": "dropped", "node": "d-2", "runId": "01H…", "ts": 1752000002, "label": "scaled temp",
  "dropped": 7 }
```

## 4. Notes an agent must know

- **Deltas-only.** Attaching after a message was published means you missed it — there is no replay.
  Persistence-to-disc is a named follow-up (it will reuse the `debug:{ws}:{flow}:{node}` series
  substrate). For a captured history, seed runs after you attach.
- **Format is host-resolved.** `auto` sniffs: object/array (or a JSON-string of one) → `json`; a
  markdown-marked string → `markdown`; else `text`. The author's explicit `format` config is
  authoritative. Don't re-sniff client-side.
- **The governor.** A per-node sliding-1s window caps real messages at `rate_limit` (default 50/s);
  over-budget messages collapse into one `dropped: k` sentinel at the window's close. If the stream
  goes quiet under load, look for the sentinel, not a hang.
- **Per-flow, not per-run.** Subscribe once to a flow; messages from every run of it land here, keyed
  by `runId`. There is no workspace-wide aggregate in v1.
- **Workspace wall.** A token for workspace B cannot subscribe to workspace A's flow — the `?token=`
  resolves the ws, the cap gate runs, and the bus subject is ws-prefixed. A denial is opaque
  (`403`, indistinguishable from "absent").

## Related
- `skills/flows-mcp/SKILL.md` — the whole `flows.*` CRUD + run/lifecycle surface.
- `scope/flows/debug-node-scope.md` — the ask + Decisions (motion-only, per-flow, format host-resolved).
- `sessions/flows/debug-node-session.md` — the working log.
