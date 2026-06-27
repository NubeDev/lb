# Frontend scope — make collaboration real (identity · workspaces · channels · messaging · inbox/outbox)

Status: scope (the ask). Promotes to `public/frontend/` once shipped. Target stage: **S9 — real
collaboration UI** (see STAGES.md). This is the named slice that
takes the UI from a **single-screen S2 demo bolted to fakes** to a **real collaboration app over a real
node**, and resolves the open questions left in `frontend-scope.md` (presence rendering; the demo
principal / dynamic workspace). It finishes **both ends as needed** — the gateway routes + a real
session on the backend, the views + navigation on the UI.

Today only **channel messaging** is wired end-to-end (Tauri `channel_post`/`channel_history`; gateway
`/channels/*`); everything else runs on in-memory fakes (`ui/src/lib/ipc/*.fake.ts`). Crucially, the
**backend for the target surfaces already exists and is tested** — `lb-inbox` (list/record/resolve/
approved), the S4 membership layer (`assets::{add_member,authorize,visibility}`), channel presence
(Zenoh liveliness), the outbox — they are simply **not exposed over the gateway and have no UI**. So
this slice is mostly **transport wiring + missing views + one genuinely-missing piece: a real identity
session** (the hardcoded `AUTHOR = "user:me"` in `App.tsx` is the single biggest blocker, because every
verb is workspace- and principal-scoped).

## Goals

- **A real login → token → principal session** (replacing the gateway's demo principal). The UI obtains
  a token, stores it, and sends it on every request; the gateway **verifies it per request** and derives
  the principal + workspace from it. This makes the **workspace dynamic** (the UI's `ws` arg is currently
  plumbed but unused) and is the groundwork for token-on-the-bus. Minimal but **real** — a verifiable
  signed token, not a hardcoded string.
- **Workspace setup** — list / select / (create) the workspaces a principal may see, bound to the
  session. No more hardcoded `acme`.
- **Channels as first-class** — a thin **channel registry** (list + create) so the UI shows a channel
  list / switcher / create, instead of one hardcoded `general`. Posting still works as today.
- **Users / teams / members** — surface the existing S4 membership backend: list members, see teams, add
  a member to a team — over a new gateway route, with a UI.
- **Messaging between real people** — two **real** principals message in a channel and see each other's
  messages live; **presence is rendered** (a `usePresence` hook beside `useChannel`) so you see who is
  online. The data already streams (`event: presence`); this renders it.
- **Inbox UI** — a real `features/inbox/` view backed by `lb-inbox` over a new `inbox_*` gateway route
  (list + resolve/approve). This **replaces the workflow fake's simulated inbox** with the real one.
- **Outbox status** — a **read-only** delivery view (pending / delivered / dead-lettered), not a CRUD
  surface (the outbox is must-deliver infrastructure, not a user-edited object).
- **Drop the fakes from the real path** for these surfaces — keep the fakes for **tests only**, matching
  the real route contracts one-to-one (the channel pattern).

## Non-goals

- **No full identity provider.** A minimal **real** session (issue + verify a signed principal token)
  now; password DBs / OIDC / SSO are a later, pluggable scope. The token path is real even if the
  credential check starts as a dev-login.
- **No native desktop window packaging** (webkit2gtk). The command layer stays headlessly tested; the
  browser-against-gateway path is the demo. Packaging is a separate step.
- **No outbox CRUD.** Read-only status only.
- **No design-system overhaul.** Reuse the existing Tailwind tokens / component shapes (`frontend-scope.md`
  visual direction). Add shadcn primitives only as the control set grows.
- **No changes to the agent / workflow / registry / native UIs.** They stay on their fakes until their
  own slice — out of these four focus areas.
- **No SDK/WIT change.** New gateway routes are thin HTTP over **existing** host verbs (+ the small
  channel-registry and session additions).

## Intent / approach

**Repeat the one move the channel already proves, four more times.** The channel surface is the
template: `lib/<x>/<x>.api.ts` (verb client) → a gateway route mirroring the host verb 1:1 → the host
verb (already capability-checked) → a `features/<x>/` view + hook. Every new surface — workspace,
channel-registry, members, inbox, outbox-status — is the **same four-file move**, against host verbs
that (except the session + channel registry) **already exist**. No new architecture; assembly.

**Identity is the keystone, so it's first.** Until "who am I / which workspace" comes from a real
session, isolation can't be demonstrated and every other route has nothing to scope by. The session:
the gateway gains a `login` route that issues a signed token (`lb_auth::mint`) for a principal +
workspace + caps; every other route reads the bearer token, `lb_auth::verify`s it, and uses that
principal (not the hardcoded demo one in `gateway/src/state.rs`). The UI keeps the token and sends it.
This single change makes the existing `ws` plumbing live and turns the **workspace-isolation test from
theatre into a real two-principal test across the gateway**.

**Channels need a registry because they're currently implicit.** Today a channel exists only by being
posted to (a bus subject). A UI needs to *list* channels, so add a thin **channel-registry record** per
`(ws, channel)` — written on first post and by an explicit `create` — and a `channel_list` verb. Posting
and history are unchanged; the registry is additive.

**Inbox replaces a fake with the real thing.** `WorkflowView` currently shows a simulated inbox from
`workflow.fake.ts`. The real `lb-inbox` verbs exist; this slice exposes them over `inbox_*` routes and
builds `features/inbox/` against them, so triage/approval items are the **real** durable items, and the
S6 approval gate becomes a real UI action.

**Rejected alternatives:**
- *Keep the demo principal, wire surfaces first.* Rejected — every surface is principal/workspace-scoped;
  without a real session you wire everything to a lie and rewrite it later. Identity first is cheaper.
- *Build a full auth provider now.* Rejected — over-scoped; a minimal verifiable token unblocks
  everything and the real IdP slots in behind the same `verify` seam later.
- *Give the outbox a CRUD UI.* Rejected — it's must-deliver infrastructure; users see *effects* and a
  *status*, never an editable queue.

## How it fits the core

- **Tenancy / isolation:** every route carries the session principal's workspace; the wall holds, and
  with **two real principals in two workspaces** the isolation test is finally meaningful end-to-end (a
  ws-B session cannot list/read ws-A channels, inbox, or members).
- **Capabilities:** each route runs the host verb's existing capability check against the **session
  principal's** caps (carried in the token); the deny path is real (not a fake's happy path). This is the
  first real exercise of **caller-grant verification at the gateway** (token-on-the-bus groundwork).
- **Placement:** unchanged — the gateway is a role; the Tauri shell runs the node in-process. Same node,
  same verbs, two transports.
- **MCP surface:** new gateway routes mirror host verbs 1:1: `login`, `workspace_list`/`create`,
  `channel_list`/`create`, `members_list`/`add`, `inbox_list`/`resolve`, `outbox_status`. The message +
  presence SSE stream already exists. The verbs are the same MCP tools the host/agent already call.
- **Data (SurrealDB):** workspaces (namespaces, exist), **channel-registry records (new)**, inbox items
  (exist), membership edges (exist, S4), outbox effects (exist). All state, workspace-scoped.
- **Bus (Zenoh):** presence liveliness (exists — now rendered) and the live message stream (exists). No
  new motion.
- **Sync / auth:** the token is the session; the gateway verifies it. Offline buffering/replay unchanged.
  Full token-on-the-bus (the hub re-verifying a routed caller) is noted as the next hardening, not this
  slice.
- **Secrets:** the token-signing key is held by the node, never the UI; the UI only holds the issued
  token.

## Example flow

1. **Alice logs in** (workspace `acme`) → the gateway issues a signed token; the UI stores it.
2. Alice sees the **workspace** `acme`, its **channel list** (`#general`, `#hvac-alerts`), the **member
   list**, and **who's online** — `Bob` shows present (rendered presence).
3. Alice **posts** in `#general`; **Bob's** session sees it live over SSE (two real principals, real
   messaging between people).
4. A **real inbox item** appears for Alice (e.g. a `needs:approval` from the S6 workflow) — from
   `lb-inbox`, not a fake. She **approves** it in `features/inbox/`; the resolution persists.
5. The approval drives the **outbox**; Alice sees the effect move **pending → delivered** in the
   read-only **outbox status** view.
6. A **ws-B user (Carol)** logs in and sees **none** of `acme`'s channels/inbox/members — the wall,
   demonstrated with real identities.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — over the **real route** (not a fake): a principal without
  `mcp:channel.post:call` / `inbox.resolve` / `members.add` is refused; the UI surfaces the `Denied`.
- **Workspace isolation — now end-to-end with two real sessions.** A ws-B token cannot `channel_list`,
  `inbox_list`, or `members_list` ws-A; cannot read ws-A history or presence. Across **gateway + store**.
  This is the test the demo principal made impossible; it gets teeth here.
- **Offline / sync** — messages and inbox items buffer offline and replay idempotently (existing path,
  re-verified through the real routes).

Plus this slice's cases:

- **Session** — `login` issues a token that `verify`s; an **expired or forged token is rejected** by the
  gateway; the workspace is taken from the token, not the request body.
- **Presence** — join/leave updates the rendered roster; a late-joining UI sees the full set
  (`history(true)`).
- **Inbox** — `inbox_list` returns the real items; `resolve`/approve persists and reflects in the view;
  the workflow fake is gone from the real path.
- **Outbox status** — reflects pending → delivered (→ dead-letter) for a real effect.
- **Channel registry** — `channel_create` then `channel_list` shows it; posting to a new channel
  registers it; history unaffected.
- **Vitest + Rust** — a view test per surface (mirror `ChannelView.test.tsx`) on the fake; a Rust route
  test per verb through the real node (mirror `commands_test` / `gateway_test`).

## Risks & hard problems

- **Identity scope creep.** The temptation to build OAuth/SSO now. Hold the line: a minimal **verifiable
  token** + a pluggable credential seam; the real IdP is a later slice behind the same `verify`. This is
  the #1 risk — it can swallow the whole slice.
- **The demo principal is load-bearing in many places.** `gateway/src/state.rs` fixes the workspace to a
  session principal and every route uses it; replacing it touches the gateway + every route. Mechanical
  but broad — sequence it first so nothing is built on the placeholder.
- **Channel registry vs implicit subjects.** Channels exist today only by being posted to; the registry
  must be **additive** — never break `post`/`history`, just make channels listable. Reconcile create-on-
  post with explicit create (both upsert the registry record).
- **Fake/route drift.** As surfaces go real, the **fakes must stay contract-identical** (they back the
  tests). A mismatched fake is a green test against a wrong shape. Keep verb names + payloads 1:1.
- **Presence ordering.** Join/leave races; rely on the liveliness `history(true)` snapshot + idempotent
  roster updates, not event order.
- **Token-on-the-bus is only half-done here.** The gateway verifies the caller; the in-process node
  trusts the gateway's verified principal. The hub re-verifying a **routed** caller's grant
  (cross-node) is explicitly deferred — don't imply this slice closes it.

## Open questions

**All resolved this slice** (see `sessions/frontend/collaboration-session.md`). Each lean was taken:

- **Login mechanism:** ✅ RESOLVED — a real signed-token endpoint (`POST /login` → `lb_auth::mint`,
  every route `verify`s) with a **dev credential store** (`session/credentials.rs`). The IdP plugs in
  behind the same `verify` seam later. (Lean taken.)
- **Channel model:** ✅ RESOLVED — a **registry record per `(ws, channel)`** (`channel_registry/`),
  upserted on first post (`register_on_post`, additive/best-effort) AND explicit `channel_create`.
  (Lean taken.)
- **Token transport to the node's caps check:** ✅ DEFERRED (as scoped) — the gateway verifies + sets
  the principal; the in-process node trusts it. Full token-on-the-bus for a **routed cross-node**
  caller is a separate hardening, noted not built.
- **Outbox status visibility:** ✅ RESOLVED — **workspace-scoped read, capability-gated**
  (`mcp:outbox.status:call`), read-only. (Lean taken.)
- **Teams vs members granularity:** ✅ RESOLVED — **minimal** (`list_members` + `add_team_member`),
  full team CRUD deferred. (Lean taken.)
- **Where the session lives in the UI:** ✅ RESOLVED — `lib/session/` (`session.store.ts` observable
  + `useSession`); `http.ts` / `channel.stream.ts` read the token. (Lean taken, shape confirmed.)

New decision recorded in the session doc: the SSE stream authenticates by a `?token=` query param
(`EventSource` cannot set an `Authorization` header); switching workspace is a re-login (the
workspace is the token's hard wall, §7).

## Related

- `scope/frontend/frontend-scope.md` — the parent; this resolves its open questions (presence rendering,
  dynamic workspace / demo principal, shadcn growth).
- `scope/bus/bus-scope.md` — presence (liveliness) + the message stream this renders.
- `scope/inbox-outbox/` — the inbox (now given a UI) and the outbox (now given a status view).
- `scope/tenancy/tenancy-scope.md` — the workspace wall the two-principal isolation test exercises.
- `scope/auth-caps/` — the token/principal/grant model the real session uses (`mint`/`verify`, caps).
- `scope/files/` + `scope/skills/` — the S4 membership backend (`add_member`/`authorize`/`visibility`)
  this surfaces as the teams/members UI.
- `scope/sync/sync-scope.md` — offline buffer + replay, unchanged, re-verified through the real routes.
- `scope/prefs/user-prefs-scope.md` — the real principal surfaced here is who finally **owns a
  preference** (language/tz/date/units); this UI is where the prefs settings surface and the resolved
  locale (catalog rendering, RTL) land. That scope's "How the UI handles this" names the client work.
- README **§6.10** (inbox/outbox), **§6.6** (identity/auth/caps), **§6.13** (frontend), **§7** (tenancy).
