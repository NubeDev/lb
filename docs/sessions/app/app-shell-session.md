# app — shell slice: the RN host + @nube/app-sdk client (session)

- Date: 2026-07-04
- Scope: [app-shell-scope.md](../../scope/app/app-shell-scope.md) (+ the client slice of
  [app-sdk-scope.md](../../scope/app/app-sdk-scope.md))
- Stage: post-S10 app slice 1 of 3 (shell → extensions → sdk extraction)
- Status: **done** — shell slice shipped; next up: app-extensions
- Public: [public/app/app.md](../../public/app/app.md) · Setup session:
  [app-scope-setup-session.md](app-scope-setup-session.md)

## Goal

Build the first implementation slice: a React Native shell (iOS + Android, one codebase)
that logs into the real gateway, holds one session **per workspace** (switcher), talks to
the node ONLY through a shared `invoke(cmd, args)` client that maps verb→route 1:1 with
`ui/src/lib/ipc/http.ts`, renders Channels end to end (list → history → live SSE → post),
and lists cap-gated `ext.list` nav entries. **No** MF remote loading, no example
extensions, no full sdk extraction — later slices.

## Version pins (scope open question, resolved)

| Piece | Pin | Why |
|---|---|---|
| react-native | **0.86.0** | current stable; template generated with `@react-native-community/cli@20.1.0` |
| react | **19.2.3** | what RN 0.86 requires |
| @callstack/repack | **5.2.5** | current Re.Pack 5 (Rspack + MF2 built in); `@rspack/core ^1.5.8` |
| react-native-keychain | ^10 | token storage (Keychain/Keystore — never AsyncStorage) |
| @react-navigation/native(+native-stack) | ^7 | scope's assumed default; Expo not adopted (Re.Pack replaces Metro) |
| react-native-fetch-api + web-streams-polyfill@3 + text-encoding | — | streaming fetch for SSE (below) |

## What was built

**`app/sdk/` — the shared client (only what the shell needs now):**
- `src/client/` — `config.ts` (injected baseUrl/token/fetch seam), `errors.ts` (**typed
  `InvokeError`** carrying the HTTP status: `isDenied` = 403 cap deny, `isUnauthenticated`
  = 401), `request.ts` (bearer on every call; 401-with-token fires `onAuthError` → that
  workspace's session drops; 403 never logs out), `invoke.ts` (verb→route 1:1 with the web
  `http.ts`: `login`, `workspace_list/create`, `channel_list/create/post/history`,
  `ext_list`, `mcp_call`), `create.ts` (composes the whole `GatewayClient`).
- `src/sse/` — `parse.ts` (SSE frame parser) + `stream.ts` (streaming-fetch transport,
  auto-reconnect) + `channel.stream.ts` (mirror of the web `channel.stream.ts`).
  **Resume design:** the gateway emits no SSE `id:` fields, so resume is NOT
  Last-Event-ID replay — it is reconnect + a durable **`channel.history` catch-up read**
  fired on every `onOpen` (channels are durable-inbox-backed; exactly the scope's
  transport rationale). Proven by the kill+resume test.
- `src/session/` — `session.types.ts` (mirrors `LoginReply`), `session.storage.ts`
  (**`SessionStorage` seam**: keychain on device, `memorySessionStorage()` in tests — a
  storage adapter, not a fake backend), `session.store.ts` (observable
  **one-session-per-workspace** store + active pointer; `useSyncExternalStore`-ready).
- `src/ext/` — `ext.types.ts` (mirrors `ExtRow`), `nav.ts` (`extNavEntries` — the
  cap-gated nav derivation, segment-wise wildcard match on `mcp:<tool>:call`).
- `src/channel/channel.types.ts` — `Item`/`ChannelRecord` mirrors.

**`app/shell/` — the RN host:**
- Native template (android/ ios/) generated with RN 0.86.0; Re.Pack wired via
  `react-native.config.js` + `rspack.config.mjs` — the **MF2 host** sharing `react`,
  `react-native`, `@nube/app-sdk` as eager singletons (remotes come next slice, additive).
- `src/polyfills.ts` — streaming fetch (RN's stock fetch buffers whole bodies; SSE would
  never emit). Loads before anything imports the sdk.
- `src/lib/` — `node-url.store.ts` (manual node URL; QR later), `client.ts` (the ONE
  `GatewayClient`, keychain-backed, rebuilt on URL change).
- `src/features/session/` — `keychain.storage.ts`, `useSession.ts`, `LoginScreen.tsx`.
- `src/features/workspaces/` — `useWorkspaces.ts` + `WorkspaceSwitcher.tsx` (stored-token
  re-activate, else re-mint by re-login — see decisions).
- `src/features/channels/` — `useChannels.ts`, `useChannel.ts` (id-keyed merge of history
  catch-up + live frames; deny renders "not permitted", never a silent empty room),
  `ChannelsScreen/ChannelScreen/MessageComposer`.
- `src/features/ext-host/` — `useExtensionNav.ts` + `ExtNavList.tsx` (list only; entries
  say "opens with the app-extensions slice").
- `src/App.tsx` — session-gated native-stack nav.

## Decisions (and rejected alternatives)

1. **Workspace switch = re-login, stored-token fast path.** There is no server re-mint
   route yet (global-identity direction); the scope pre-authorized this fallback. The
   client keeps one session per workspace, so switching back re-activates the stored
   token without a round trip. When a re-mint route lands, only `switchWorkspace` changes.
2. **"Old-token rejected" reinterpreted honestly.** Tokens are stateless — a pre-switch
   token stays valid *for its own workspace* until expiry. The session tests prove the
   real guarantees instead: the old token stays **confined** to its workspace, a dropped
   session has no token → typed 401, a tampered token → 401 **and** the client drops that
   session. Revocation-on-switch would need the admin `authz_revoke_tokens` lever — not a
   shell concern.
3. **`app/shell` is NOT a pnpm-workspace member** (`app/shell/.npmrc`,
   `ignore-workspace=true`; install with `pnpm install --ignore-workspace` if the .npmrc
   is ever bypassed). The workspace root pins `@types/react` to 18 (web shell is React
   18) — RN 0.86 requires React 19 + its types; one lockfile cannot hold both. `app/sdk`
   IS a member (its tests run against the repo toolchain); the shell links it via
   `file:../sdk`, so web and app still share one client source. Revisit when the web
   shell moves to React 19. Rejected: scoped overrides (fragile) and downgrading RN.
4. **SSE over streaming fetch, not an EventSource lib.** One parser/transport in the sdk
   works in Node tests (undici streams natively) and on device via the
   `react-native-fetch-api` polyfill (`textStreaming: true`). Rejected:
   `react-native-sse` (a second, RN-only code path the tests couldn't exercise —
   the drift the shared sdk exists to prevent).
5. **The sdk compiles without the DOM lib** — `stream.ts` uses structural types for
   `ReadableStream`/`TextDecoder` from `globalThis`, so the RN tsconfig (no DOM) can
   typecheck the sdk sources directly.

## Testing (rule 9 — real spawned gateway, no fakes, seeded via real write paths)

`app/sdk/vitest.gateway.config.ts` + `tests/real-gateway.ts` build and spawn the same
`test_gateway` bin `ui/ pnpm test:gateway` uses. Explicit-cap tokens come from the
harness's `/_seed/session`; the extension row from `/_seed/extension`; the second user is
admitted through the **real** `POST /admin/members` route. 12 tests / 5 suites:

- `session.gateway.test.ts` — login → workspace list → switch (re-minted token, both
  sessions stored, switch-back reuses) · old token confined to its workspace · dropped
  session + tampered token → typed 401, session cleared.
- `channels.gateway.test.ts` — list → post → history → **live SSE** (second client's post
  arrives) · **kill+resume**: message lands while disconnected, reconnect's history
  catch-up recovers it, live feed works after.
- `caps-deny.gateway.test.ts` — **mandatory deny per verb**: post/create denied without
  `bus:chan/{cid}:pub` (reads still work) · list/history denied without `bus:chan/*:sub`
  · `mcp_call` denied without the tool cap. All surface `InvokeError.isDenied`.
- `isolation.gateway.test.ts` — **mandatory workspace isolation**: ws-B can't see ws-A's
  channel in a list, and reading it by name lands in B's own (empty) namespace; A still
  reads its row.
- `ext-nav.gateway.test.ts` — real seeded install → `ext_list` → `extNavEntries` shows it
  under the dev cap set and hides it without the tool cap · installs are ws-scoped.

```
$ pnpm --dir app/sdk test:gateway
 ✓ tests/channels.gateway.test.ts (2 tests) 476ms
 ✓ tests/session.gateway.test.ts (3 tests) 225ms
 ✓ tests/caps-deny.gateway.test.ts (4 tests) 87ms
 ✓ tests/ext-nav.gateway.test.ts (2 tests) 101ms
 ✓ tests/isolation.gateway.test.ts (1 test) 119ms
 Test Files  5 passed (5)
      Tests  12 passed (12)
```

Typechecks: `pnpm --dir app/sdk typecheck` and `app/shell$ pnpm typecheck` (tsc, React 19
types) both exit 0.

**Deferred, explicitly (not a silent gap):** RN component tests
(react-native-testing-library). RN 0.86's Flow-typed source doesn't transform under
Vitest without a Babel pipeline; the shell's components are thin over hooks that are thin
over the sdk seam the 12 gateway tests exercise. The next slice (app-extensions) sets up
the RN jest/babel harness and adds component coverage there. Also not run here: a device
build (no Android SDK/Xcode in this environment) — the native template is stock RN 0.86 +
the Re.Pack wiring above.

## Debugging

No product defects. Two findings worth recording (session-level, no `docs/debugging/`
entry — nothing in the node broke):
- A second user's login into a bootstrapped workspace is **refused by design**
  (global-identity decision #4, "not a member"); tests must admit them via the real
  `POST /admin/members` first. The channels tests hit this and were corrected — the
  refusal is the feature working.
- Adding `app/shell` to the pnpm workspace pulls the root `@types/react@18` override into
  an RN/React-19 project (typecheck breaks). Resolution = decision #3 above.

## Follow-ups carried to the next slices

- app-extensions: MF2 remote load + mount (`ExtNavList` already labels entries), the
  `[app]` manifest block, RN jest component harness, the two reference exts.
- app-sdk: extract the web `http.ts` verb map into the sdk as the authored source
  (this slice deliberately duplicated only the 9 shell verbs); CI drift check.
- Replace re-login switch with a real re-mint route when global identity lands one.
- QR node-URL hand-off from the web shell.
