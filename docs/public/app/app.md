# App — the React Native mobile app (shipped truth)

Shipped so far: the **shell slice** (session:
[app-shell-session.md](../../sessions/app/app-shell-session.md)). The extension mount
(`app-extensions`) and the full sdk extraction (`app-sdk`) are still scope — see
`docs/scope/app/README.md`.

## What exists

**`app/shell/`** — the RN host (one codebase, iOS + Android). Pins: **RN 0.86.0, React
19.2.3, Re.Pack 5.2.5** (Rspack + Module Federation 2; the shell is the MF2 host sharing
`react`/`react-native`/`@nube/app-sdk` as eager singletons — remotes land next slice).
Installs **standalone** (`app/shell/.npmrc`): the repo workspace pins `@types/react@18`
for the React-18 web shell, RN 0.86 needs React 19. Surfaces: login (node URL + dev
credential), workspace switcher, Channels (list/create → room with live SSE + composer),
Extensions (cap-gated `ext.list` nav entries, listed only).

**`app/sdk/` (`@nube/app-sdk`)** — the shared, platform-free gateway client both future
app extensions and the shell use (a pnpm-workspace member; the shell links it via
`file:../sdk`):

- `createGatewayClient({ baseUrl, storage })` → `{ invoke, session, login,
  switchWorkspace, streamChannel }`.
- **`invoke(cmd, args)`** maps verb→route **1:1 with `ui/src/lib/ipc/http.ts`** (this
  slice: `login`, `workspace_list/create`, `channel_list/create/post/history`,
  `ext_list`, `mcp_call`). The app adds **no** gateway verbs and **no** caps.
- **Typed errors:** `InvokeError.isDenied` (403 cap deny → render "not permitted") vs
  `.isUnauthenticated` (401 → that workspace's session drops, back to login).
- **Session = token per workspace** (`SessionStore`), persisted through the
  `SessionStorage` seam — platform keychain on device
  (`app/shell/src/features/session/keychain.storage.ts`), `memorySessionStorage()` in
  tests. The active workspace is always the signed token's; switch re-activates a stored
  token or re-mints by re-login (no server re-mint route yet).
- **SSE** (`openChannelStream`/`openSse`): streaming-fetch transport with
  auto-reconnect. The gateway emits no SSE ids, so resume = reconnect + a
  `channel.history` catch-up read on every `onOpen` (durable-inbox-backed — nothing is
  lost across kills; proven by test). On device this needs the shell's
  `src/polyfills.ts` (streaming fetch) loaded first.
- `extNavEntries(rows, caps)` — the cap-gated nav derivation over `ext.list` (a
  convenience gate; the host re-checks every call).

## Transport (decided, shipped)

REST + SSE through the gateway only — the phone is the fourth thin client (browser,
Tauri, CLI, mobile). zenoh-ts stays rejected (an unmediated second server surface); the
full rationale is in [app-shell-scope.md](../../scope/app/app-shell-scope.md).

## How it's tested (rule 9)

`app/sdk$ pnpm test:gateway` spawns the **real** `test_gateway` node (the same bin as
`ui/ pnpm test:gateway`) — 12 tests: session/switch, channels REST+SSE incl. kill+resume
catch-up, **capability-deny per verb**, **workspace isolation**, ext.list nav gating.
Green output in the session doc.
