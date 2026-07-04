# App scope — the React Native shell (iOS/Android host)

Status: scope (the ask). Promotes to `public/app/` once shipped.

Build the mobile counterpart of the web shell: a React Native app (iOS + Android) that
logs a person in, lets them switch between **many workspaces**, talks to the node
**only through the gateway**, discovers installed extensions via `ext.list`, and mounts
each extension's app bundle as a federated remote. The shell owns identity, transport,
navigation, and theming; **everything else is an extension** — exactly the web shell's
posture, on a phone.

## Goals

- One RN host app (`app/shell/`) that builds for iOS and Android from one codebase.
- Login → a person's workspaces → a workspace switcher; the active workspace comes
  from the signed session token, never from client state (the hard wall, README §3.6).
- All platform access through the existing gateway verb surface: MCP calls
  (`POST /mcp/call`), channels, agent invoke, series, prefs — the same routes the web
  shell uses, behind the same one-seam client (`invoke(cmd, args)`).
- Live data via the gateway's SSE streams (channel messages, agent RunEvents, series).
- Extension discovery (`ext.list`) → cap-gated nav entries → federated mount, mirroring
  `ui/src/features/ext-host/`.
- Offline-tolerant reads (cache-and-revalidate), honest failures on writes.

## Non-goals

- No Zenoh session on the phone (see transport decision below). Zenoh stays node↔node.
- No native-module surface for extensions (that's the JS-only rule in
  `app-extensions-scope.md`).
- No offline-first sync/authority work — the app is a client of one node; `sync/` owns
  multi-node authority.
- No tablet/desktop layout work in v1; phone-first.
- Push notifications — real, but a later slice (needs APNs/FCM plumbing plus an
  outbox-driven notify target; scope it separately when it's next).

## Intent / approach

The shell is **the fourth thin client** of the gateway (browser, Tauri, `lb` CLI, now
mobile), holding no authority of its own — it is only ever as authorized as the token it
presents. It reuses the web shell's proven seams rather than inventing parallel ones:

- **Client seam:** one `invoke(cmd, args)` dispatcher (the `ui/src/lib/ipc/invoke.ts`
  pattern) with an HTTP implementation mapping verb→route 1:1 with `ui/src/lib/ipc/http.ts`.
  Feature code never sees the transport. The verb map itself becomes a shared,
  platform-free module in `@nube/app-sdk` so web and app cannot drift (see
  `app-sdk-scope.md`).
- **Session:** login (`POST /login`) issues the signed token; the token carries
  principal + workspace + caps. Multi-workspace = the global-identity model
  (`auth-caps/global-identity-scope.md`): login resolves to the person's workspaces,
  switching workspaces re-mints a workspace-scoped token. Tokens live in the platform
  keychain (Keychain/Keystore), never AsyncStorage.
- **Build system:** **Re.Pack 5 (Rspack) + Module Federation 2** — the one production
  path for module federation on React Native, and the same MF2 mental model as the web.
  The shell shares `react`, `react-native`, and `@nube/app-sdk` as singletons; remotes
  are pure-JS containers fetched from the gateway.

### Transport decision: REST + SSE via the gateway. zenoh-ts rejected.

The question was REST/SSE vs WebSocket vs zenoh-ts (`eclipse-zenoh/zenoh-ts`). The
answer is **REST + SSE through the gateway**, for the long term, because of what
zenoh-ts actually is and what the platform's rules require:

- **zenoh-ts is not a Zenoh peer.** It speaks WebSocket to the `zenoh-plugin-remote-api`
  on a `zenohd`/node. Adopting it means exposing a *second* server surface (the remote-api
  plugin) directly to the internet — one that sits **beside** the gateway, not behind it.
  Every message would need the capability + workspace checks re-implemented at that
  surface, or it becomes a hole in the wall. Rules 5–7 (capability-first, workspace is
  the hard wall, MCP is the contract) say there is exactly one mediated front door; the
  browser already honors that (Zenoh is never exposed to the browser — motion reaches it
  as gateway SSE), and the phone should too.
- **State vs motion still holds server-side.** Zenoh keeps moving messages between
  nodes; the gateway is where motion is re-authorized and fanned out to clients. The
  client transport choice doesn't touch rule 3.
- **Mobile realities favor stateless HTTP + resumable SSE.** Backgrounding kills
  sockets; NAT/carrier middleboxes drop idle connections. Request/response over HTTP
  needs no session repair; SSE reconnects with `Last-Event-ID` replay (channels are
  durable-inbox-backed, so history catch-up is a `channel.history` read, already built).
  A persistent Zenoh/WS session buys nothing here and costs battery + reconnection
  state machines.
- **Symmetry with the web shell is compounding value:** same routes, same verb map,
  same SSE endpoints, same tests (`test_gateway`), same debugging story.

**Rejected alternative, kept as a future option:** a single multiplexed WebSocket
carrying the *same gateway verbs* (not raw Zenoh) — worth revisiting only if measured
battery/socket-count pain appears with many concurrent SSE streams. Because everything
sits behind the `invoke`/stream seam, that would be an optimization patch, not an
architecture change. Direct zenoh-ts stays rejected regardless: the blocker is
mediation, not framing.

## How it fits the core

- **Tenancy / isolation:** the gateway derives principal + workspace from the token on
  every request; the app never sends workspace ids as authority. Workspace switch =
  new token. The mandatory isolation test drives two real workspaces through the app's
  client seam and proves neither sees the other's channels/series.
- **Capabilities:** the app adds **no new capabilities**. Every call is an existing
  gateway route already gated (`mcp:<tool>:call`, `bus:chan/...`, member caps). The
  deny path surfaces honestly in the UI (a denied verb renders as "not permitted",
  never a silent empty state).
- **Placement:** the app connects to whichever node the user points it at (cloud hub
  normally; a LAN edge node works identically — symmetric nodes, rule 1).
- **MCP surface:** consumed, not extended. Get/list via the typed routes + `mcp_call`;
  live feeds via the existing SSE routes (`/channels/{cid}` stream, agent RunEvents,
  `/series/{s}/stream`). No new verbs → no new API-shape decisions.
- **Data (SurrealDB):** none touched directly; the app holds only an ephemeral
  client-side cache (react-query style) that is never authoritative.
- **Bus (Zenoh):** not reachable from the app; motion arrives as gateway SSE.
- **Sync / authority:** the node is authoritative; the app's cache is best-effort and
  revalidated. Offline = read cache + queue nothing (writes fail honestly in v1).
- **Secrets:** only the session token, in Keychain/Keystore. No provider keys on
  device, ever — model access stays behind the node's ai-gateway.
- **AI agent access:** `POST /agent/invoke` (the `agent.invoke` seam) plus the
  in-channel agent (`channel.post` with `kind:"agent"`, RunEvents over SSE) — the same
  two paths the web shell uses. Nothing agent-specific in the shell beyond UI.

## Example flow

1. User installs the app, enters the node URL (or scans a QR from the web shell), logs in.
2. `POST /login` → token; the shell lists the person's workspaces, user picks one; a
   workspace-scoped token is minted and stored in the keychain.
3. Shell calls `ext.list`; rows with an `[app]` block become nav entries (cap-stripped,
   exactly like `useExtensionPages.ts` on web).
4. User opens Channels (a shell surface): `channel.list` → `channel.history` → SSE
   stream; posts a message (`channel.post`); asks the agent in-channel
   (`kind:"agent"`) and watches the RunEvent stream render live.
5. User opens the `proof-panel` extension entry: the shell fetches the signed app
   bundle from the gateway, mounts its `Page` component with `{ ctx, bridge }`; the
   page calls `proof-panel.proof.derive` through the scoped bridge.
6. User switches workspace from the drawer: token re-mint, nav + caches reset; the
   other workspace's channels are provably not reachable with the old token.

## Testing plan

Per `scope/testing/testing-scope.md` — real infra, rule 9, no fakes:

- **Gateway integration (the backbone):** the app's client layer runs against the
  **real spawned node** (`test_gateway` bin + a `vitest.gateway` config, exactly the
  `ui/ pnpm test:gateway` pattern). Seed real records; no `*.fake.ts`.
- **Mandatory capability-deny:** a token without `mcp:channel.post:call` (and without
  `bus:chan/{cid}:pub`) gets a deny through the app client; assert the typed error
  surfaces, not a crash.
- **Mandatory workspace-isolation:** two workspaces, two tokens; channels/series/exts
  of one are invisible to the other through the app seam.
- **Session:** login → workspace list → switch → old-token calls rejected.
- **SSE:** post to a channel from a second client; the app's stream sees it; kill and
  resume the stream; history catch-up closes the gap.
- **Component tests** (react-native-testing-library) for shell surfaces, driven
  through the same real-gateway client — jsdom-level rendering, real data.

## Risks & hard problems

- **RN + Rspack toolchain drift.** Re.Pack tracks RN releases; pin versions and treat
  upgrades as their own slices. This is the piece most likely to eat unplanned time.
- **SSE on mobile networks** — backgrounding, radio sleep, proxy buffering. The
  reconnect + durable-history catch-up must be designed in from day one (it's why REST
  + SSE won; don't squander it).
- **Two shells, one drift risk.** The verb map and widget contract must live in shared
  packages (`@nube/app-sdk`, later `@nube/panel-kit`), or web and app will diverge
  silently. Treat any copy-paste of `http.ts` as a finding.
- **Store review**: remote JS bundles are within Apple/Google policy (interpreted code
  that doesn't change app purpose — the CodePush precedent) but the *framing* matters;
  the shell must degrade gracefully when a remote fails to load or verify.

## Open questions

Resolved by the shell slice (`sessions/app/app-shell-session.md`, 2026-07-04):

- ~~Version pins~~ → **RN 0.86.0, React 19.2.3, Re.Pack 5.2.5** (+ react-native-keychain,
  react-navigation 7 — the assumed default held; Expo not adopted **in the shell
  slice**. Adopting Expo's native modules (bare install) without losing Re.Pack/MF is
  scoped separately in `app-expo-scope.md`.
- ~~Workspace switch: re-mint vs re-login~~ → **re-login IS the re-mint** (no server
  re-mint route yet) with a stored-token fast path per workspace; only
  `switchWorkspace` changes when a real re-mint route lands.
- RN SSE client → **streaming fetch** (one sdk transport for tests + device via the
  fetch-streams polyfill); resume = reconnect + `channel.history` catch-up (the gateway
  emits no SSE ids).

Still open:

- Node URL discovery UX: manual entry shipped; add QR from the web shell (cheap).
- `app/shell` sits OUTSIDE the pnpm workspace (root `@types/react@18` override vs RN's
  React 19) — rejoin when the web shell moves to React 19.
- RN component-test harness (jest + RN babel preset) — deferred to the app-extensions
  slice; the client seam is covered by the 12 real-gateway tests meanwhile.

## Skill doc

N/A — the shell exposes no new agent-/API-drivable surface; it consumes existing verbs.
If a later slice adds one (e.g. push-notification registration verbs), that slice names
its skill.

## Related

- `app-extensions-scope.md`, `app-sdk-scope.md` (this topic)
- `../extensions/ui-federation-scope.md` — the web counterpart of the mount path
- `../auth-caps/global-identity-scope.md` — multi-workspace login/switcher
- `../channels/channels-scope.md`, `../channels/channels-agent-scope.md` — the surfaces v1 ships
- `../cli/operator-cli-scope.md` — the sibling "thin client of the gateway" doctrine
- README §3 (rules), §6.5 (dispatch chokepoint), §7 (identity)
