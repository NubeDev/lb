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
- **Validated restore** (`client.restore()` → `probeSession`): on boot the shell doesn't just
  rehydrate the stored token — it probes the node once (`GET /workspaces`) and **drops a session
  the node no longer honours** (401 = token dead; network error = node unreachable, dropped under
  `onUnreachable:"drop"`, kept under `"keep"`; 403/2xx = live, kept). So a token that outlived its
  node (e.g. after an in-memory preview restart re-keys the gateway) falls to the login screen
  instead of rendering a stale empty channel list. Fix lives in the SDK, so native gets it too.
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
`ui/ pnpm test:gateway`) — 17 tests: session/switch, channels REST+SSE incl. kill+resume
catch-up, **capability-deny per verb**, **workspace isolation**, ext.list nav gating, a
**client-singleton regression** (a login-established session stays observable on the one
client — the shell's "stuck on the login screen" fix), and a **restore-liveness regression**
(a rehydrated-but-invalid session is dropped to login, not shown empty — 401 dead, unreachable
drop/keep, live kept). Green output in the session doc.

## Running the browser preview (dev)

`make -C app dev` runs the react-native-web preview (the real `App.tsx` in a browser, no
emulator) on `127.0.0.1:5310` against its **own** throwaway in-memory `test_gateway` on
`8087` — deliberately off `8080`, where the root `make dev` node's persistent store may
already have `acme` claimed by another user (that would 403 the ada/acme prefill with "not
a member", global-identity decision #4). Open `http://127.0.0.1:5310/?node=http://127.0.0.1:8087`;
the prefilled `ada`/`acme` sign straight through to Channels. `make -C app gateway` refuses to
kill a non-`test_gateway` holding `8087` (won't clobber another terminal's node — pass
`GW_PORT=<free>` instead). The preview node is **in-memory**, so restarting it wipes channels and
re-keys tokens; the shell's validated restore now catches the dead token and shows the login screen
(prefilled — one click back in) rather than an empty channel list. See
[app-preview-login-session.md](../../sessions/app/app-preview-login-session.md) and
[app-preview-stale-session-session.md](../../sessions/app/app-preview-stale-session-session.md).
