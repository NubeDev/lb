# Slice 7 — `BridgeTransport` + the federated wiresheet page

Status: scope slice (S7). Depends on: S1+S2 (the seam + vendored package), S3–S5
(verbs), S6 (watch — can start against the fallback plumbing). Parent:
`control-engine-scope.md`.

The LB-authored UI half: a `BridgeTransport` implementing the wiresheet's
`EngineTransport` over `bridge.call('ce.*')` + `bridge.watch('ce.watch')`, and the
extension's federated `[ui]` page that mounts the vendored `CeEditor` with it, plus an
appliance picker. The browser never touches CE; every canvas action is a caps-gated
MCP call.

## Where the code lives (and doesn't)

- `rust/extensions/control-engine/ui/` — the federated remote (the `proof-panel`
  precedent): `Page.tsx` (appliance picker + editor mount), `bridge-transport.ts`,
  `frames.ts` (ce.watch JSON → `DecodedFrame`/`TopologyMsg`/`SchemaMessage`),
  one responsibility per file.
- `packages/ce-wiresheet` — **untouched** (the S2 rule). The transport is injected:
  `<CeEditor transport={new BridgeTransport(bridge, appliance)} />`. If the seam turns
  out to be missing something, the fix goes upstream (S1 branch), then re-vendor.

## `BridgeTransport` design

- **request half** — a table from the wiresheet's REST paths to `ce.*` tools; the
  transport refuses paths with no mapping (loud error listing the path — that's the
  signal a follow-up verb is needed, never a silent 404):
  `GET /nodes*` → `ce.tree` · `GET /schema` → `ce.schema` · `POST /nodes` →
  `ce.add-node` · `PATCH /nodes/uid/*` → `ce.patch` · `PATCH /overrides/*` →
  `ce.set-override`/`ce.clear-override` · `POST /bulknodes` (edge form) →
  `ce.add-edge` · `DELETE /nodes/uid/*` → `ce.remove-node` · `POST /call/*` →
  `ce.call-action`.
  Alternatively — decided at implementation with S1 — the seam's request half may be
  *typed* (per-operation methods) rather than path-shaped, which deletes this table;
  prefer typed if the S1 refactor makes it natural.
- **stream half** — `openStream` = `bridge.watch('ce.watch', { appliance })`;
  `frames.ts` decodes the S6 JSON kinds into the handler callbacks. `setSubscriptions`
  maps to the watch args (scope by visible uids) or, v1-simple, subscribes the whole
  appliance and filters client-side — decide by frame volume on the real engine
  (measure, then choose; record the call).
- **v1 gaps, explicit:** presence bar → hidden (no presence over the bridge);
  per-actor undo → engine-shared stack (no `actor` forwarded);
  drag-position persistence → parked until `ce.set-layout` (S5 deferred list) — the
  page shows positions from CE but drags snap back on reload. Each gap surfaces in
  the UI as absent-not-broken.

## The page

- Manifest `[ui]` federated remote (ui-federation-scope): route `/control-engine`,
  nav entry via the shell's extension nav contract.
- Appliance picker: `ce.appliance.list` → dropdown; empty state links the
  `ce.appliance.add` flow (a small form calling the tool — admin-capped, so the form
  simply errors DENIED for non-admins).
- The page's install scope requests exactly the `ce.*` read+write tools it drives;
  `bridge.call` narrowing (install scope ∩ caller grant) is what makes a read-only
  user's canvas read-only.

## Testing / exit gate (real gateway, rule 9 — no `*.fake.ts`)

- `pnpm test:gateway` (vitest against a real spawned node with the extension +
  `ce_fake`-backed sidecar):
  - `ce.tree` renders components on the canvas;
  - a canvas edit round-trips (`ce.patch` observed in the store via `ce.tree` re-read);
  - a COV frame injected into the fake arrives and updates a rendered value
    (through real SSE);
  - read-only grant: canvas renders, an edit attempt surfaces the DENIED error;
  - ws-B user: appliance absent, page shows the empty state.
- Unit (vitest, no network): `frames.ts` decode vectors; the request-map table
  covers every path `rest.ts` can emit (derive the list from S1's typed surface —
  a compile-time exhaustiveness if typed, a fixture list if path-shaped).
- **Exit gate:** the gateway suite green + a manual run: cloud UI editing a
  `ce-studio` engine on the same box through the full bridge (screenshot in the
  session doc).
