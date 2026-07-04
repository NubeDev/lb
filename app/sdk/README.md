# `@nube/app-sdk` — contracts + client for app extensions

**Status: contracts + the shell-slice client SHIPPED.** Scope: `docs/scope/app/app-sdk-scope.md`.

What ships here (v1):

- `src/contract/` — the mount + widget contract types. `WidgetCtx` is byte-compatible
  with the shipped web v3 frames-in contract
  (`ui/src/features/dashboard/builder/federationWidget.ts`); this package is the
  **authored source** the web mirrors are checked against.
- `src/client/` — the shared `invoke(cmd, args)` seam + cmd→route verb map (1:1 with
  `ui/src/lib/ipc/http.ts`, fetch-based, platform-free; the shell's 9 verbs so far) and
  `src/sse/` — the stream client with reconnect + `channel.history` catch-up (the
  gateway emits no SSE ids, so durable history IS the replay). `src/session/` holds the
  token-per-workspace store over a `SessionStorage` seam (keychain on device).
- `src/defineExtension.ts` — the typed way a remote declares `{ Page?, Widget? }`.

Layout follows `docs/FILE-LAYOUT.md`: one contract per file, one verb-group per client
file, mirroring `ui/src/lib/*` names so tool ↔ dto ↔ client symmetry extends to the app.

`src/contract/` + the shell-slice client are real (tested: `pnpm test:gateway` spawns
the real `test_gateway` — 12/12, incl. deny-per-verb + ws-isolation). Still to come:
`defineExtension.ts` (app-extensions slice) and the FULL web verb-map extraction with
the CI drift check (app-sdk slice).
