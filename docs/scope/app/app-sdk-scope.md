# App scope — `@nube/app-sdk` and the shared panel/widget SDK

Status: scope (the ask). Promotes to `public/app/` once shipped.

One contract package (`app/sdk/` → `@nube/app-sdk`) that every app extension builds
against — the mount/bridge/ctx types, the gateway verb map, and the stream client —
and, long term, the **shared panel/widget SDK**: one headless layer that the web
dashboard and the mobile app both render from, so a widget authored once works in a
dashboard cell and on a phone.

## Goals

- `@nube/app-sdk` v1 ships four things, each platform-free unless stated:
  1. **Contract types** — `MountCtx`, `WidgetCtx` (the v3 frames-in shape),
     `Bridge`/`WidgetBridge`, `AppRemote`. Byte-compatible with
     `ui/src/features/dashboard/builder/federationWidget.ts`.
  2. **Verb map + client** — the `invoke(cmd, args)` seam and the cmd→route table,
     extracted from `ui/src/lib/ipc/http.ts` into shared code (fetch-based; works in
     RN and the browser), so web and app cannot drift.
  3. **Stream client** — SSE with reconnect + `Last-Event-ID`, RN-compatible
     (`react-native-sse` or a thin fetch-stream reader; decide at implementation).
  4. **`defineExtension` helper** — the typed way a remote declares
     `{ Page?, Widget? }`, giving compile-time contract errors instead of mount-time.
- The web `ui/` gradually consumes 2–3 from the same package (delete-the-copy, not
  fork-the-copy).
- Long term: promote `ui/src/lib/panel-kit/` to `packages/@nube/panel-kit` (already
  the stated Next-up in STATUS.md) and make it the **one** panel-authoring/
  source-binding brain; web `views/*` and RN widget renderers are two view layers
  over it.

## Non-goals

- No RN component library in v1 (no `@nube/app-ui`); extensions use RN primitives +
  shell theme tokens. A shared component kit is a later scope once two extensions
  demonstrate the repeated need.
- No renderer unification — web keeps DOM/ECharts renderers, RN gets native chart
  renderers; **only the headless layer is shared**. Trying to share pixels (e.g.
  react-native-web or WebView charts everywhere) is explicitly rejected: it flattens
  both platforms to the worse of each.
- panel-kit promotion is *aligned with*, not *blocked on*, this scope — v1 app-sdk
  ships without it.

## Intent / approach

Three layers, drawn once:

```
@nube/panel-kit   (later)  ← headless: cell spec ⇄ editor state, sources[], SQL builder, genui
@nube/app-sdk     (this)   ← contracts + verb map + invoke/stream clients (platform-free core)
shell bindings             ← web: ext-host/federationWidget (DOM mount)   app: MF2 component mount
```

The load-bearing decision is **where the contract source of truth lives**. Today the
widget contract exists in three mirrors kept in sync by hand (host `federationWidget.ts`,
each extension's `contract.ts`, the devkit template); the app adds a fourth consumer.
This scope makes `@nube/app-sdk` (a repo-root-style workspace package under `app/sdk`,
published to the pnpm workspace) the **authored source**: the web host and devkit
template import (or CI-diff against) it. The alternative — a fifth hand-synced copy —
is rejected as the drift risk it obviously is.

`WidgetCtx` stays the shipped v3 frames-in contract *unchanged*: a data widget
receives shell-resolved `ctx.data` frames and never fetches. That contract is exactly
what makes cross-platform widgets feasible — the platform-specific part is rendering
frames, not acquiring them. The app shell reuses the same source-resolution logic
(`sources[]` → frames) once panel-kit is a package; until then the app resolves via
the same verbs the web `useVizFrames` calls.

## How it fits the core

- **Tenancy / Capabilities:** the sdk carries no authority — it is types + a client
  that forwards the shell's token and surfaces denies as typed errors. Nothing new to
  gate.
- **Placement / Data / Bus / Sync / Secrets:** N/A — a client-side library; no store
  tables, no bus subjects, no secret material (it must never persist tokens itself;
  the shells own storage).
- **MCP surface:** none added. The verb map is a *projection* of existing routes; a
  route addition lands in one shared table instead of two `http.ts` files.
- **Symmetric nodes / one datastore / state-vs-motion:** N/A (client library).
- **One responsibility per file:** the package follows FILE-LAYOUT — one contract per
  file (`contract/mount.ts`, `contract/widget.ts`), one verb-group per client file
  (`client/channel.api.ts`, `client/agent.api.ts`), mirroring `ui/src/lib/*` naming
  so the cross-stack symmetry (tool ↔ dto ↔ client) extends to the app.

## Example flow

1. An extension author runs the devkit app template; the scaffold imports
   `defineExtension`, `WidgetCtx`, `Bridge` from `@nube/app-sdk`.
2. They author `Widget` rendering `ctx.data` frames as a native chart; the same
   widget id's web tile renders the same frames via ECharts — one binding model, two
   renderers.
3. A gateway route is added for some new verb: one edit to the shared verb map; web
   and app both pick it up on the next dependency bump.
4. Later, panel-kit becomes `@nube/panel-kit`; the app gains a phone panel viewer by
   wiring RN views onto `usePanelEditor`/`cellToEditorState` — zero new authoring
   logic.

## Testing plan

Per `scope/testing/testing-scope.md`:

- **Contract lock:** a CI check that `federationWidget.ts` + the devkit template types
  are assignable to/from the sdk's (until they import it directly); breaking either
  direction fails the build.
- **Client against the real gateway:** the shared invoke/stream clients run the
  existing `test_gateway` suites (the web `pnpm test:gateway` proves the extraction
  changed nothing; an RN-runtime smoke proves the same client works off-DOM).
- **Mandatory capability-deny / workspace-isolation:** inherited by construction from
  the shell + extension suites, but the sdk adds one direct case each: a deny
  round-trips as the typed error shape; a cross-workspace read through the shared
  client returns nothing.
- **No fakes:** the sdk ships no mock transport. The one seam (`invoke`) is the same
  seam the web already uses to swap HTTP/Tauri — real gateway in tests, per rule 9.

## Risks & hard problems

- **Extraction gravity.** Pulling the verb map out of `ui/src/lib/ipc/http.ts`
  touches every feature's `*.api.ts` import path. Do it mechanically, one commit per
  verb group, with the gateway suite green at each step — or the extraction stalls
  half-done and becomes the drift it was meant to kill.
- **RN/browser fetch-stream differences.** SSE in RN is not free; if the stream
  client forks per platform, keep the fork behind one interface in one named file —
  not sprinkled `Platform.OS` checks.
- **panel-kit's `@/lib/dashboard` type graph** is the known blocker for promotion
  (STATUS.md). This scope must not smuggle dashboard types into app-sdk to work
  around it — that inverts the dependency and blocks panel-kit forever.

## Open questions

- Package layout: does `app/sdk` join the existing repo-root `packages/*` pnpm
  workspace, or does `app/` get its own workspace? (Lean: one root workspace — the
  web must import the sdk, so a second workspace only adds linking pain.)
- SSE client choice in RN (`react-native-sse` vs hand-rolled fetch reader) — decide
  with a spike in the implementing session; record in the session doc.
- Does the sdk own error types (typed deny/transport errors) or adopt the web's
  existing shapes? (Lean: sdk owns them; web adopts on extraction.)

## Skill doc

N/A — no agent-/API-drivable surface; it is a client library. The devkit skill doc
gains an app-template section under `app-extensions-scope.md`'s remit.

## Related

- `app-shell-scope.md`, `app-extensions-scope.md` (this topic)
- `ui/src/lib/panel-kit/` + STATUS.md "Next up" (the promotion this aligns with)
- `../frontend/dashboard/ext-widget-source-binding-scope.md` (the v3 frames-in contract)
- `../frontend/dashboard/widget-kit-scope.md` (the web-side widget reuse push — same
  destination, other shore)
- `packages/source-picker`, `packages/genui` (prior art for headless-first packages)
- `docs/FILE-LAYOUT.md` (the layout the package must follow)
