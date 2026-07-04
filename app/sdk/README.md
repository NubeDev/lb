# `@nube/app-sdk` — contracts + client for app extensions

**Status: scaffold only.** Scope: `docs/scope/app/app-sdk-scope.md`.

What ships here (v1):

- `src/contract/` — the mount + widget contract types. `WidgetCtx` is byte-compatible
  with the shipped web v3 frames-in contract
  (`ui/src/features/dashboard/builder/federationWidget.ts`); this package is the
  **authored source** the web mirrors are checked against.
- `src/client/` — the shared `invoke(cmd, args)` seam + cmd→route verb map (extracted
  from `ui/src/lib/ipc/http.ts`, fetch-based, platform-free) and the SSE stream client
  with reconnect + `Last-Event-ID`.
- `src/defineExtension.ts` — the typed way a remote declares `{ Page?, Widget? }`.

Layout follows `docs/FILE-LAYOUT.md`: one contract per file, one verb-group per client
file, mirroring `ui/src/lib/*` names so tool ↔ dto ↔ client symmetry extends to the app.

Only `src/contract/` exists so far — the types the two example extensions and the shell
are designed against. The client extraction is its own implementation slice (see the
scope's testing plan: real `test_gateway`, no fakes).
