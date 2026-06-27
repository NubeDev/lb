# Collaboration — the real app over a real session

What shipped when the UI stopped being a single-screen demo on fakes and became a **real
collaboration app over a real node**. Built as five repetitions of the channel's proven move
(`lib/<x>/<x>.api.ts` → a gateway route → an already-capability-checked host verb → a
`features/<x>/` view + hook), identity first. Source of truth: `scope/frontend/collaboration-scope.md`
and `sessions/frontend/collaboration-session.md`.

## The session (identity is the keystone)

The gateway's demo principal is gone. A real, verifiable signed token now carries identity:

- `POST /login` (`role/gateway/src/routes/login.rs`) mints an `lb_auth` token for a principal +
  workspace + caps. The credential check is a **dev-login** for now (`session/credentials.rs`,
  `dev_claims`) — but the **token path is real** (`mint` + `verify`). A real IdP plugs in behind the
  same `verify` seam later.
- **Every** other route authenticates first (`session/authenticate.rs`): it reads the
  `Authorization: Bearer <token>` header, `lb_auth::verify`s it with the node key, and derives the
  principal — so the **workspace + caps come from the token, never the request** (the hard wall, §7).
  A missing/garbled token is `401`; a forged or expired token is `401`; an ungranted verb is `403`.
- The UI holds the token in `lib/session/` (`session.store.ts`, an observable read by the IPC layer;
  `useSession`). `App.tsx` no longer hardcodes `WS`/`CHANNEL`/`AUTHOR` — logged out shows the login
  screen, and switching workspace is a **re-login** (the workspace is the token's hard wall).
- The SSE stream authenticates by a `?token=` query param (`EventSource` cannot set a header); the
  stream route verifies it identically.

This makes the **workspace-isolation test real**: two genuine sessions on one node, a ws-B token sees
none of ws-A's channels / inbox / members / history / presence — proven across gateway + store.

## The surfaces

| Surface | Host verb (new) | Gateway route | UI |
|---|---|---|---|
| Workspaces | `workspace_list` / `workspace_create` | `GET\|POST /workspaces` | `features/workspace/` switcher |
| Channels (registry) | `channel_list` / `channel_create` (+ `register_on_post`) | `GET\|POST /channels` | `features/channel/ChannelList` |
| Members / teams | `list_members` / `add_team_member` | `GET\|POST /teams/{team}/members` | `features/members/` |
| Presence | (existing `watch` SSE) | `event: presence` on the stream | `features/channel/usePresence` + roster |
| Inbox (real) | `list_inbox` / `resolve_inbox` | `GET /inbox/{ch}`, `POST /inbox/{id}/resolve` | `features/inbox/` |
| Outbox status | `outbox_status` (read-only) | `GET /outbox` | `features/outbox/` |

- **Channels** are now first-class: a thin registry record per `(ws, channel)`, written on an
  explicit create AND on first post (additive — posting and history are unchanged). Reuses the
  channel `pub`/`sub` capability (creating = "may post", listing = "may read"), no new grammar.
- **Members** surface the S4 `lb_assets` membership edges through a dedicated `mcp:members.*`
  capability (the S4 files-scope follow-up). Minimal: list + add.
- **Presence is rendered**: the `event: presence` `{member, present}` feed (already streamed) folds
  into an idempotent roster (`mergePresence`) shown in the channel header.
- **Inbox** is the **real** `lb_inbox` durable queue — it replaces the workflow fake on the real path.
  Approve/Reject is the S6 approval gate as a real UI action (the resolution persists, actor = the
  session principal, host-forced).
- **Outbox status** is **read-only** (pending / delivered / dead-lettered) — the outbox is
  must-deliver infrastructure, never an editable queue.

## Capabilities the dev session grants

`bus:chan/*:pub|sub`, `mcp:members.list|add:call`, `mcp:inbox.list|resolve:call`,
`mcp:outbox.status:call`, `mcp:workspace.list|create:call`. A narrower principal proves each deny
path.

## Tested

Rust: `crates/host/tests/collaboration_test.rs` (cap-deny + ws-isolation for every new verb) and the
gateway suite (`role/gateway/tests/gateway_test.rs` + `gateway_routes_test.rs`) — session (issue /
verify / forged / expired / workspace-from-token), capability-deny, two-session workspace-isolation,
channel registry, real inbox list+resolve, outbox pending→delivered, and the live SSE push over a
real socket. UI: a Vitest view test per surface on the contract-identical fakes (kept for tests only,
never on the real path).

## Deferred

Token-on-the-bus for a routed cross-node caller (the in-process node trusts the gateway-verified
principal); a real IdP behind `verify`; the Tauri desktop command layer's session (this slice wired
the browser/gateway path); full team CRUD; workspace data provisioning (this makes a workspace
*listable*, not its namespace).
