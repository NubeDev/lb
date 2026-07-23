# Ext-UI caller-identity scope — an extension PAGE learns whether the caller is an admin (and what caps they hold), without probing

Status: **in-progress** (contract shipped; consumers landing). The SDK change is tagged
[`ui-v0.12.0`](https://github.com/NubeDev/lb-ext-ui-sdk/pull/5) — `PageCtx`/`WidgetCtx` gain
`caps`/`isAdmin`, `useCaps()`/`useIsAdmin()` added, `role` deprecated as an authz signal. Host producer
([rubix-ai#26](https://github.com/NubeIO/rubix-ai/pull/26)) and first consumer
([ems#11](https://github.com/NubeIO/ems/pull/11)) PRs are open and verified live on the running node.
Promotes to `public/extensions/` once all three merge. Topic:
[`extensions`](extensions-scope.md) — the extension family. This is the **UI twin** of
[`native-caller-identity-scope.md`](native-caller-identity-scope.md) (SHIPPED): that scope carried the
caller's identity across the **native sidecar** call frame; this one carries it across the **federated
page mount** boundary. Touches the stable SDK contract
([`ext-sdk-scope.md`](ext-sdk-scope.md), [`ui-federation-scope.md`](ui-federation-scope.md)) and the
host mount ([`host-callback-scope.md`](host-callback-scope.md)). Consumes the caps model
([`../auth-caps/`](../auth-caps/)).

A federated extension page mounts its **own** React root, so host React context — including the
signed-in session and its capabilities — does **not** cross the boundary. The one seam that does is
`PageCtx`, and today `PageCtx` carries only `{ workspace }` plus header/theme hints. An extension page
therefore has **no trustworthy way to know whether the caller is an admin**, so it cannot decide which
admin-only affordances to render. The fix: the host stamps a **minimal, verified caller projection**
(`caps`, and the derived `isAdmin`) onto `PageCtx` at mount, exactly as the native scope stamped a
caller projection onto the native call frame. The verbs remain the real wall (capability-first); this
only lets the UI decide what to *show* — synchronously, correctly, and without a network probe.

## Goals

1. **The caller's capability signal reaches the page on mount.** `PageCtx` gains an additive,
   host-populated `caps: string[]` and a derived `isAdmin: boolean`. An extension page can gate an
   admin affordance from `ctx` directly — no round-trip, no flicker.
2. **`isAdmin` is derived the SAME way the host derives it for itself.** The host already computes
   `isAdmin(caps)` (rubix-ai `lib/session/admin-caps.ts`: any of the `ADMIN_SECTION_CAPS` present).
   The value stamped onto `PageCtx` is that same function's output — one definition of "admin", shared
   host↔ext, never a second guess.
3. **Additive + fail-closed.** `caps`/`isAdmin` are optional on `PageCtx` (an old host omits them). The
   SDK surfaces them with safe fallbacks: absent `caps` ⇒ `[]`, absent `isAdmin` ⇒ `false`. A page
   written against the new contract degrades to "hide admin affordances" on an old host — never to
   "show them to everyone."
4. **The role string is deprecated as an authz signal, in the contract and the docs.** lb mints every
   session `role: "member"` and carries real authority in the CAP set (see the native scope + the
   backend `caller.rs`: "caller.role cannot tell a real admin from a scoped member"). The SDK's session
   type must stop implying `role` is an authorization input; `caps`/`isAdmin` are the signal.

## Non-goals

- **No new grant, no widened reach.** This exposes what the caller ALREADY holds; it grants nothing.
  The bridge stays caps-checked; a page that shows a control it can't back still gets `out_of_scope`
  from the verb.
- **No full principal on the page.** Like the native scope's "minimal projection" decision, we thread
  the *capability signal*, not the token, not PII. `sub`/`ws` beyond the existing `workspace` are out
  of scope unless a concrete consumer needs them (open question below).
- **No per-entity reach on the page.** "Does this caller reach `site:123`?" stays a backend question
  (the `authz.*` verbs / the extension's own reach chokepoint). `isAdmin` is the coarse show/hide gate;
  row-level visibility is enforced server-side, unchanged.
- **No live re-supply.** `caps` is stamped at mount. A caps change mid-session re-mounts on the next
  navigation/login (matches how the header theme axes already behave — `update(ctx)` re-supply is the
  same deferred follow-up called out in `ExtHost`).

## Intent / approach

Three additive changes across three repos, released in dependency order (SDK → host → consumer), each
a no-op for anyone who hasn't adopted it:

1. **lb / `lb-ext-ui-sdk` (the contract).** Add to `PageCtx` (`src/page.ts`):
   `caps?: string[]` and `isAdmin?: boolean`. Add SDK accessors on the runtime
   (`src/runtime.ts`) — `useCaps(): string[]` (⇒ `ctx.caps ?? []`) and `useIsAdmin(): boolean`
   (⇒ `ctx.isAdmin ?? false`) — so consumers never touch the raw optional. Tag `ui-v0.12.0`.
2. **rubix-ai (the host — the producer).** `ExtHost` builds the mount ctx; today it passes
   `{ workspace, headerStyle, headerLine, sidebarToggle, onToggleSidebar }`
   (`ui/src/features/ext-host/ExtHost.tsx`). It has the session in scope (the router ctx already carries
   `caps`/`principal`). Add `caps: session.caps` and `isAdmin: isAdmin(session.caps)` — reusing the host's
   OWN `isAdmin` from `@/lib/session` (goal 2). Bump the SDK dep to `ui-v0.12.0`.
3. **ems-ext (the first consumer — proves the seam).** Replace the interim backend **probe**
   (`ui/src/hooks/useIsAdmin.ts`, which calls `ems.access.list` to infer admin) with the SDK's
   `useIsAdmin()` reading `ctx.isAdmin`. Delete the probe. The five gate sites already route through the
   local hook, so the swap is one file's body. Bump the SDK dep; re-publish.

**Alternative considered — keep the probe.** ems-ext currently ships a working probe (call an
admin-gated verb, treat 200 as admin). It's correct but: it's a network round-trip on every mount, it
flickers (fail-open until it resolves), every extension must re-invent it, and it couples the gate to a
specific verb's existence. The native scope already rejected "let the sidecar guess" for the identical
reason — the host, which HAS the identity, should hand it over. The probe stays only as the
fail-closed fallback story for an old host, not the design.

**Why not forward the full JWT/principal?** Minimal projection (native-scope precedent): the page needs
the capability signal to render, nothing more. A full principal on a federated page is a larger attack
surface for zero additional UI capability.

## How it fits the core

- **Capabilities:** the whole point — `caps`/`isAdmin` are a *projection* of the caller's existing
  grants, host-derived, never a new grant. Deny path unchanged: the bridge re-checks every call
  host-side; a mis-shown control fails at the verb. `isAdmin` is show/hide only.
- **Tenancy / isolation:** `workspace` is already the frozen field; `caps` are the caller's
  workspace-scoped caps as minted. No cross-workspace data crosses the mount.
- **Placement:** either — the ctx is built by whatever host mounts the page (cloud or edge); symmetric,
  no `if cloud`.
- **MCP surface:** **none new.** This is a mount-context contract, not a verb. Explicitly N/A for
  CRUD/get-list/watch/batch — the caps already ride the session the host verified at login; we thread a
  read-only projection of them across one boundary. (The interim ems-ext probe *reads* an existing verb;
  the shipped design removes that read.)
- **SDK/WIT impact:** **YES — flag loudly.** This changes the stable `PageCtx` in `lb-ext-ui-sdk`. It is
  additive (optional fields + new hooks), so it's a MINOR bump (`ui-v0.12.0`), not breaking; but every
  ext-UI consumer's SDK pin is the release-coordination surface (WORKFLOW-LB tag→bump). The widget mount
  ctx (`mountWidget`) should gain the same fields in the same tag for parity.
- **Data / Bus / Secrets / Sync:** N/A — no records, no subjects, no secrets, no new authority; a mount
  reads the already-verified session.

## Example flow

1. `ada@acme.com` (minted `role: "member"`, holding `mcp:ems.template.add:call`, `ems.access.grant`, …)
   opens the EMS extension in rubix-ai.
2. `ExtHost` mounts the EMS page with `ctx = { workspace: "acme", caps: [...ada's caps],
   isAdmin: isAdmin(caps) /* = true */, headerStyle, … }`.
3. EMS's `AppShell` calls the SDK's `useIsAdmin()` → `ctx.isAdmin` → `true`, **synchronously on first
   render**. The Studio + Access rail items and the New-site button render immediately — no probe, no
   flicker.
4. A scoped member (`bob`, no admin caps) opens the same page: `ctx.isAdmin` is `false`; the admin
   affordances never render. If a bug showed one, `ems.access.grant` still returns `out_of_scope`
   server-side — the wall held.

## Risks & hard problems

- **SDK release coordination (the real cost).** Three repos, in order, each pinned to the tag. A host
  on the new tag serving an ext built against the old SDK: the ext ignores the new fields (fine). An ext
  on the new SDK under an old host: `ctx.isAdmin` is `undefined` ⇒ SDK returns `false` ⇒ admin
  affordances hidden (fail-closed — an admin sees too little, never a member too much). Both directions
  are safe; that's the point of additive + fail-closed.
- **Two "isAdmin" definitions drifting.** Goal 2 is the guard: the host stamps the value from its own
  `ADMIN_SECTION_CAPS`; the SDK does NOT re-derive from `caps`, it reads the host's stamped `isAdmin`
  (with `caps` available for finer per-cap gates like `hasCap`). One authority, projected — not
  recomputed on the far side.
- **Staleness window.** Caps stamped at mount; a mid-session grant change isn't reflected until re-mount.
  Acceptable (matches the header-theme axes today); a live `update(ctx)` re-supply is the same deferred
  follow-up already noted for the theme fields — out of scope here, named as an open question.
- **Temptation to over-thread.** Keep the projection minimal; every field added to `PageCtx` is a
  permanent contract surface. `caps` + `isAdmin` answer the show/hide need; resist `sub`/`email`/roles.

## Testing plan

Mandatory categories from [`../testing/testing-scope.md`](../testing/testing-scope.md):

- **Capability gate (the core case).** With a real seeded admin session, the host builds a ctx with
  `isAdmin: true`; with a real seeded member session, `isAdmin: false`. Test against the REAL host mount
  + real login (no mock session) — rubix-ai e2e: sign in as the admin seed, mount the ext, assert the
  admin rail item is present; (when a member seed exists) sign in as member, assert it's absent AND the
  admin verb is denied server-side (the wall, independent of the UI).
- **Workspace isolation.** `caps` on the ctx are the caller's workspace-scoped caps; a two-workspace
  test confirms no cross-ws cap leaks into the projection.
- **Backward-compat / fail-closed.** Unit: SDK `useIsAdmin()`/`useCaps()` return `false`/`[]` when the
  ctx omits the fields (old-host path). Unit: an ext page built on the new SDK, mounted with a
  legacy `{ workspace }`-only ctx, hides admin affordances.
- **SDK contract.** `lb-ext-ui-sdk` unit tests: `PageCtx` accepts the new optional fields; the hooks
  read them; `mountWidget` parity.
- **ems-ext regression.** The five gate sites (AppShell, Sites, AccessPage, MeterPage, BuilderPage)
  render admin affordances for an admin ctx; the probe file is deleted and nothing imports it.

**Skill doc:** N/A — this exposes no new agent-/API-drivable surface (no MCP verb, no gateway route). It
changes a mount-context contract consumed by UI code only. (The `native-caller-identity` sibling was
likewise a contract change, not a drivable surface.)

## Open questions

1. **Field set:** ship `{ caps, isAdmin }` only, or also `sub` (some ext may want "created by me")? Lean
   `{ caps, isAdmin }` now (minimal projection); add `sub` when a concrete consumer needs it.
2. **`role` on the SDK session type:** delete it outright (breaking for any type-referencer) or keep it
   with a `@deprecated` "always 'member'; use `isAdmin`" note? Lean **deprecate-then-remove**: annotate
   in `ui-v0.12.0`, remove in the next MAJOR — no silent breakage.
3. **Widget parity in the same tag?** Recommend yes — `mountWidget`'s ctx gets `caps`/`isAdmin` too, so
   panel-options/ext widgets gate consistently. Confirm no widget consumer breaks on the additive field.
4. **Live re-supply of caps on grant change** — defer to the shared `update(ctx)` re-supply follow-up
   (tracked with the header-theme axes), or force it here? Lean defer; name the dependency.

## Related

- Sibling (the precedent this mirrors): [`native-caller-identity-scope.md`](native-caller-identity-scope.md).
- SDK contract: [`ext-sdk-scope.md`](ext-sdk-scope.md), [`ui-federation-scope.md`](ui-federation-scope.md),
  [`host-callback-scope.md`](host-callback-scope.md).
- Caps model: [`../auth-caps/`](../auth-caps/).
- Host admin derivation to reuse: rubix-ai `ui/src/lib/session/admin-caps.ts` (`isAdmin`, `ADMIN_SECTION_CAPS`).
- Interim consumer workaround being replaced: ems-ext `ui/src/hooks/useIsAdmin.ts` (the probe).
- Public stub (filled on ship): [`../../../doc-site/content/public/extensions/extensions.md`](../../../doc-site/content/public/extensions/extensions.md).
