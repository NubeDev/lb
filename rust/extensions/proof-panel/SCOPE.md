# proof-panel — scope: the "whole platform, one page" demo

**What this extension is for.** `proof-panel` is the Tier-1 WASM reference extension. Its *whole
point* is to prove the platform end to end **from inside one cap-gated federated page**, through the
host-mediated read-only/▷write bridge — not to be a real product surface. So as the core grows, this
page grows with it: every major seam (identity, store, ingest, durable workflow) gets one live control
here.

This file is the **co-located scope** (the ask). The canonical home is
`docs/scope/extensions/proof-panel-scope.md`; keep them in sync, or promote this once it stabilises.
Session log: `docs/sessions/extensions/proof-panel-session.md`. Public truth:
`docs/public/extensions/extensions.md`.

---

## Status (2026-06-27)

**DONE — federation rework (shipped, green).** The page now loads. We ripped out
`@originjs/vite-plugin-federation` (its `isHost`/shareScope/two-React chain ended in
"Invalid hook call") and replaced it with the **rubix-cube import-map + externalised-React** pattern:

- Host shell publishes its React singletons on `globalThis.__lb*` (`ui/src/features/ext-host/
  singletons.ts`), declares an import map in `ui/index.html`, ships shims in `ui/public/shims/*.mjs`,
  and `ext-host/federation.ts` is now a plain `import(url)` of the remote — no `__federation_method_*`.
- The remote (`ui/`) is a **Vite lib build**: `vite.config.ts` externalises
  `react`/`react-dom`/`react-dom/client`/`react/jsx-runtime`, emits a single `dist/remoteEntry.js`,
  and `src/remoteEntry.ts` injects the compiled Tailwind CSS `?inline` and re-exports the frozen
  `mount(el, ctx, bridge)`. Manifest `[ui] entry = "remoteEntry.js"`.
- **Green:** Playwright e2e (`ui/e2e/proof-panel.spec.ts`) loads the BUILT shell on :4173 against the
  REAL node on :8080, logs in `user:ada`/`acme`, opens the page, asserts real content rendered into
  `[data-ext-host="proof-panel"]` with NO "Could not load" / NO "Invalid hook call" / no console
  errors. Screenshot: `ui/e2e/__screenshots__/proof-panel-mounted.png`. Proof-panel UI unit tests
  green (6). (Still to run/confirm: shell `pnpm test:gateway`.)

**NEXT — this scope: the all-features demo.** The page currently proves only the **read** half
(`series.find`/`series.latest`). Build the full round-trip so the page *creates* the data it shows and
exercises the durable workflow layer.

---

## Goals

Restructure `Panel.tsx` into three live sections, each a real MCP verb over the bridge:

1. **Ingest → read round-trip (headline).** A "Write sample" button posts `ingest.write` (series
   `proof.demo`, auto-incrementing `seq`, a user value + `ts`). The existing `series.find`/
   `series.latest` reads it back live → proves **write → stage → drain (node commit worker) → read**
   end to end, in the browser, through the bridge. The page *creates* its own data.
2. **Outbox status (durable motion).** A card of `outbox.status` counts (`pending`/`delivered`/
   `dead_lettered`) + a Refresh button. Read-only, no args.
3. **Inbox triage (durable workflow).** `inbox.list { channel }` items, each with Approve/Reject
   buttons calling `inbox.resolve { item_id, decision }` — the first *write* that mutates workflow
   state from the page.

## Non-goals

- No new core verbs — only wire verbs that already exist (`ingest.write`, `outbox.status`,
  `inbox.list`, `inbox.resolve`).
- No federated **widgets** (Phase 2; dashboard scope owns that).
- fleet-monitor's matching rework is a **separate** follow-up — do NOT touch it until proof-panel's
  demo is green (the rework files for it were started but proof-panel comes first).

## How it fits the core

- **Capabilities (the gate).** Add to `extension.toml` `[capabilities] request` AND `[ui] scope`:
  `ingest.write`, `outbox.status`, `inbox.list`, `inbox.resolve`. The bridge filters locally
  (defense in depth) and the host re-checks each call server-side; the grant is intersected at install
  (`ui_scope_is_narrowed_to_the_grant`). The dev-login token already holds these (the shell's own
  Ingest/Inbox/Outbox views use them), so the live round-trip passes server-side.
- **Tenancy.** Every verb is workspace-scoped by the token's `ws` — the hard wall (§7). A `proof.demo`
  written in ws-A must be invisible to ws-B's page.
- **Data/bus.** `ingest.write` stages; the node's drain worker commits staging → `series`; the page
  reads back via `series.latest`. Confirm the running node drains (the shell Ingest view round-trips,
  so it does).
- **Bridge contract.** Frozen. All controls are `bridge.call(tool, args)`; `MountCtx`/`Bridge` in
  `ui/src/app/contract.ts` are unchanged.

## MCP surface (verbs this slice wires, end to end)

| Verb | Args | Returns | UI control |
|---|---|---|---|
| `ingest.write` | `{ samples:[{series,ts,seq,value}] }` | `{ accepted }` | Write sample |
| `series.find` / `series.latest` | (already wired) | series list / latest sample | auto after write |
| `outbox.status` | `{}` | `{ pending, delivered, dead_lettered }` | Outbox card + Refresh |
| `inbox.list` | `{ channel }` | `Item[]` | Inbox list + Refresh |
| `inbox.resolve` | `{ item_id, decision }` | `ok` | Approve / Reject |

## FILE-LAYOUT (one verb per file)

New hooks: `ui/src/data/useIngestWrite.ts`, `useOutboxStatus.ts`, `useInboxList.ts`,
`useInboxResolve.ts`. New section components under `ui/src/pages/` (e.g. `IngestSection.tsx`,
`OutboxSection.tsx`, `InboxSection.tsx`); `Panel.tsx` becomes a thin composition. ≤400 lines/file, no
`utils`/`helpers`.

## Testing plan (real infra, seeded via the real write path — CLAUDE §9)

- **Deny-test per new verb** (gateway suite + the co-located bridge-scope test): an out-of-scope
  bridge call rejects locally (`/out_of_scope/`) and the real host 403s an ungranted principal.
- **Workspace isolation:** `ingest.write` `proof.demo` in ws-A; a fresh ws-B page's `series.find`
  returns none of it.
- **Live round-trip (gateway vitest):** write a sample through the bridge → `series.latest` returns
  the value just written (after drain).
- **Playwright e2e (extend the existing spec):** click Write sample → assert the new value renders in
  the latest line; click Refresh outbox → assert counts render; assert still NO "Invalid hook call" /
  console errors. Capture an updated screenshot.

## Open questions — RESOLVED (2026-06-27, see session doc)

1. **Inbox producer.** RESOLVED → **(a)** honest empty state when the node emits no items (no
   fabricated workflow state). Approve/Reject is exercised by seeding a real inbox item in the host +
   gateway tests, not by faking one in the page.
2. **`seq` source.** RESOLVED → auto from `series.latest`'s last seq + 1, fall back to 1 — one-click.
3. **Build order.** RESOLVED → shipped ingest + outbox first (green live round-trip), then inbox.

## Status — SHIPPED (Session 2, 2026-06-27)

The all-features demo is built and green. Backend: `call_tool` dispatches the new workflow verbs +
`ingest.write` drains synchronously for instant read-back. Manifest carries the four verbs in both
`[capabilities] request` and `[ui] scope`. Frontend: one hook per verb + section components +
thin `Panel.tsx`. Tests (real infra, seeded via the real write path): 9 host (5 new), 8 proof-panel
unit, 9 real-gateway (5 new), full shell gateway suite 65, Playwright e2e (click Write sample → value
renders; Refresh outbox → counts render; no hook/console errors). **Finding:** the persistent SurrealKV
store throws `Invalid revision` on the 2nd ingest drain (pre-existing engine bug, reproduced on the
untouched `POST /ingest`); the live demo runs on the in-memory engine. See the session doc + the
debugging entry.

## How to run / verify (servers already up in dev)

```
# node on :8080 already running (make cloud / make dev). Rebuild + redeploy this ext's UI + shell:
cd rust/extensions/proof-panel/ui && ./node_modules/.bin/vite build           # → dist/remoteEntry.js
cd /home/user/code/rust/lb && make publish-ext EXT=proof-panel                 # deploy wasm + UI bundle
cd ui && VITE_GATEWAY_URL=http://127.0.0.1:8080 ./node_modules/.bin/vite build # build shell
cd ui && VITE_GATEWAY_URL=http://127.0.0.1:8080 ./node_modules/.bin/vite preview --host 127.0.0.1 --port 4173 &
# tests:
cd rust/extensions/proof-panel/ui && ./node_modules/.bin/vitest run            # proof-panel unit
cd ui && VITE_GATEWAY_URL=http://127.0.0.1:8080 LB_SHELL_URL=http://127.0.0.1:4173 ./node_modules/.bin/playwright test --project=chromium
cd ui && ./node_modules/.bin/vitest run --config vitest.gateway.config.ts      # shell real-gateway
```
