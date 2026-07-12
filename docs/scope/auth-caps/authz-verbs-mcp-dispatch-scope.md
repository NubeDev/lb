# Auth-caps scope — route `grants.*` / `roles.*` / `teams.*` through the MCP dispatcher

Status: **shipped** 2026-07-12 (branch `authz-verbs-mcp-dispatch`). Implemented per Intent /
approach §1–4; tests in `rust/crates/host/tests/authz_mcp_dispatch_test.rs` (5, green) + existing
authz suites (16, green); promoted to `doc-site/content/public/auth-caps/auth-caps.md`. Session:
`docs/sessions/auth-caps/authz-verbs-mcp-dispatch-session.md`.

> Read with: `entity-scoped-grants-scope.md` (this closes the "**Deriveable by extensions**"
> goal it stated but could not yet deliver over the native tier), `authz-grants-scope.md`
> (the durable grant/role/team store whose verbs `call_authz_tool` already implements),
> `../mcp/mcp-scope.md` §"The contract" (the `<ext>.<tool>` dispatch pipeline),
> `../host-tools/host-tools-scope.md` (host-native verbs reached through the one MCP
> contract — the exact seam this extends), `../extensions/native-callback-transport-scope.md`
> (the `SidecarClient` → `POST /mcp/call` path a native sidecar calls the host back on),
> `../testing/testing-scope.md` §2.

The host MCP dispatcher (`lb_host::call_tool` in `rust/crates/host/src/tool_call.rs`) routes
host-native verb families to their handlers by name prefix — `series.` → ingest, `authz.` →
`call_authz_tool`, `invite.` → invites, and so on. `call_authz_tool` **already implements**
the full authz admin surface: `grants.assign`, `grants.revoke`, `grants.list`,
`grants.list_scoped`, `roles.define`, `roles.list`, `roles.delete`, `teams.create`,
`teams.list`. But the dispatcher has **no arm** for the `grants.` / `roles.` / `teams.`
prefixes — only `authz.`. So a call to `grants.assign` over `POST /mcp/call` never reaches
the handler that exists three lines away: it falls through to the generic
extension-registry path, finds no registered extension named `grants`, and returns `Denied`.
Consequence: `grants.*`/`roles.*`/`teams.*` are reachable **only** through their dedicated
gateway REST routes (`POST /admin/grants`, …) that the admin console uses — **not** through
the generic `/mcp/call` bridge that a **native (Tier-2) extension** reaches the host on.
This scope makes those verbs reachable over the callback exactly like every other host-native
verb — a prefix-table entry plus dispatch arm, four cap-gate aliases, and catalog rows (see
Intent / approach; it is *not* the single `else if` the discovery record sketched).

## Goals

- **A native extension can mint/revoke a scoped grant over the host callback.**
  `SidecarClient::call_tool("grants.assign", …)` (and `.revoke` / `.list` / `.list_scoped`)
  dispatches to `call_authz_tool` and succeeds when the caller holds the existing admin cap
  (`mcp:grants.assign:call`, …) — closing the `entity-scoped-grants-scope.md` promise that a
  guardianship edge linked/unlinked "can create/remove scoped grants through the normal
  granted `grants.*` verbs."
- **`roles.*` and `teams.*` come along for free**, since `call_authz_tool` already handles
  them and they share the same admin-cap gate. One arm, three prefixes.
- **Purely additive, zero behavior change to existing callers.** No new verb, no new
  capability, no grammar change, no WIT/SDK bump. The REST routes are untouched; this only
  *adds* a second, MCP-native way to reach the same already-gated handler.
- **Same wall, same deny.** The verbs stay gated by their existing admin caps, checked
  workspace-first at the MCP `authorize` phase exactly as `authz.*` is today.

## Non-goals

- **No new authz semantics.** The grant/role/team model, the anti-widen rule, the
  built-in-role immutability, the scope selector — all unchanged (`authz-grants-scope.md`,
  `entity-scoped-grants-scope.md` own those). This is *routing only*.
- **No change to the REST admin routes.** `POST /admin/grants` et al. keep working as-is for
  the console; this doesn't deprecate or move them.
- **No new capability for the callback path.** A native extension calling `grants.assign`
  is gated by the *same* `mcp:grants.assign:call` cap a console admin needs — the callback is
  not a privilege escalation, it is a second transport to the same gate.
- **No cross-node routing work.** These are host-native verbs over the embedded store; they
  dispatch locally like the other `call_authz_tool` verbs. Zenoh routing is out of scope
  (and N/A — a grant write is workspace-local).

## Intent / approach

Four small touches in `rust/crates/host/src/`, all following existing patterns. **Not** the
"one `else if` arm" the discovery record sketched — that arm alone is dead code, because the
whole host-native dispatch block sits behind `is_host_native(qualified_tool)`, which consults
the `HOST_NATIVE_PREFIXES` const; a prefix not in that list falls to the extension-registry
path before any arm is reached.

1. **`tool_call.rs` — `HOST_NATIVE_PREFIXES`:** add `"grants."`, `"roles."`, `"teams."` beside
   the existing `"authz."` entry. This is the gate that actually decides host-native vs
   extension routing.
2. **`tool_call.rs` — the dispatch arm:** extend the existing `authz.` arm's condition (or add
   a sibling arm) so the three prefixes delegate to `crate::call_authz_tool` exactly as
   `authz.` does today.
3. **`tool_call.rs` — `gate_tool_for` aliases (load-bearing).** The outer MCP gate runs
   `mcp:<tool>:call` with the tool's *own name* by default, but four of the nine verbs are
   inner-gated (and role-bundled) under a *different* cap — the established
   ride-an-existing-grant pattern (`outbox.enqueue_held` → `outbox.enqueue`, etc.):
   - `grants.revoke` → `grants.assign` (assign/revoke share the cap — `authz/grants.rs`)
   - `grants.list_scoped` → `grants.list`
   - `teams.create` → `teams.manage` (no `mcp:teams.create:call` exists in any role bundle)
   - `roles.delete` → `roles.manage`

   Without these aliases the outer gate demands caps (`mcp:teams.create:call`,
   `mcp:grants.revoke:call`, …) that **no role mints — workspace-admin included** — so
   revoke/create/delete would be dead on arrival while assign/list worked, and the example
   flow's unlink step would fail in production with a deny no grant can fix. `gate_tool_for`
   is also the `tools.catalog` visibility gate, so the aliases keep the palette honest too.
4. **`system/catalog.rs` — descriptor rows** for the three new families. The
   `host_catalog_covers_dispatch_prefixes` test derives from `HOST_NATIVE_PREFIXES` and
   hard-fails on any dispatched prefix with no catalog entry (this mirror-drift trap is
   exactly why the test exists). Nine rows, following the existing `authz.*` block at
   `system/catalog.rs` §authz.

**Why this and not "expose a new `authz.grant_*` verb":** the verbs already exist, are already
capability-gated, and are already the names the REST routes and the console use. Coining
`authz.grant_assign` aliases would fork the vocabulary (two names for one effect), churn the
grammar and every caller, and still leave the admin console on the old names. Routing the
real names is the smaller, truer change — the dispatcher's job is to *route*, and it simply
has a gap in its prefix table.

**Why the read half already works (and proves the shape):** `authz.check_scoped` and
`authz.scope_filter` are also implemented inside `call_authz_tool`, but they sit under the
`authz.` prefix, so the existing arm already routes them. A native extension can therefore
*read* the scoped-grant surface over the callback today — it just can't *write* it. This arm
makes write symmetric with read.

## How it fits the core

- **Tenancy / isolation:** unchanged. `call_authz_tool` resolves against `&node.store` scoped
  to the caller's `ws` (from the verified token, never the body — the `/mcp/call` route takes
  `ws` from the session per README §7). A grant can only ever be written under the caller's
  own workspace; the arm adds no new workspace surface.
- **Capabilities:** the MCP `authorize` phase runs `mcp:<gate_tool_for(tool)>:call`
  workspace-first before dispatch, so `grants.assign` over the callback demands
  `mcp:grants.assign:call` — the same cap the verb's inner gate enforces. The four aliased
  verbs (approach §3) gate under the same cap their inner gate and the admin role bundle
  already use (`teams.create` under `mcp:teams.manage:call`, …) — the alias *narrows nothing
  and widens nothing*; it makes the outer gate agree with the inner one. Every verb still
  re-runs its inner `authorize_tool` + anti-widen inside `call_authz_tool` (defense in depth,
  same as `insight.*`). **The deny path is unchanged in shape:** an ungranted caller gets the
  opaque MCP `Denied` (no existence signal) — previously from the registry fall-through, now
  from `authorize_tool`; indistinguishable to the caller.
- **Placement:** either — these are embedded-store verbs; every node that runs the host MCP
  surface can serve them. No cloud/local split (symmetric-nodes: no `if cloud`).
- **MCP surface** (API shape §6.1): **no new verbs.** Existing writes
  (`grants.assign`/`grants.revoke`, `roles.define`/`roles.delete`, `teams.create`) and reads
  (`grants.list`/`grants.list_scoped`, `roles.list`, `teams.list`) simply become reachable on
  a second transport. No live-feed, no batch, no job — each call is a single bounded store
  write/read. CRUD is already fully specified by `authz-grants-scope.md`; this scope adds *no*
  API shape, it removes a routing gap.
- **Data (SurrealDB):** the existing `grant` / role / team records under the workspace
  namespace — untouched. No new table, no migration.
- **Bus (Zenoh):** N/A. A grant write is a workspace-local store mutation; it does not ride
  the bus. (Downstream cache-freshness of resolved caps is `builtin-role-freshness-scope.md`'s
  concern and is unchanged by *how* the write arrived.)
- **Sync / authority:** node-local authoritative write to the embedded store, same as the REST
  path. Offline behavior unchanged.
- **Secrets:** N/A.
- **SDK/WIT impact:** **none** — this is host-internal dispatch routing. The `SidecarClient`
  callback ABI (`native-callback-transport-scope.md`) already carries `call_tool(name, args)`;
  no new export, no protocol-major bump. **Flagged explicitly because the plugin boundary is
  load-bearing:** verify no WIT change is needed (it isn't — the transport is generic).

## Example flow

The childcare embedder (`cc-app`) deriving a guardian's row-level reach when an admin links a
guardianship edge:

1. Admin calls `care.guardianship.link(guardian, child)` on the care sidecar (Tier-2 native
   extension), holding `mcp:grants.assign:call` and the reach cap in its token.
2. The verb's transactional body writes the `guardianship` edge, then derives reach:
   `SidecarClient::call_tool("grants.assign", { subject: "user:<guardian>", cap:
   "mcp:care.reach.child:call", scope: { table: "child", ids: ["<child>"] } })`.
3. The callback POSTs `/mcp/call` on the gateway. The gateway authenticates the session token,
   then calls `lb_host::call_tool(node, principal, ws, "grants.assign", input)`.
4. `authorize` passes (`mcp:grants.assign:call` present, workspace matches). **The new arm**
   matches the `grants.` prefix and delegates to `call_authz_tool`, which writes the scoped
   grant row (anti-widen: the caller holds the reach cap, so it may grant it).
5. Later the guardian's `assert_reach` reads it back via `authz.scope_filter` (already live) —
   read and write now travel the identical callback, and era-2 reach is fully live end-to-end.
6. On `care.guardianship.unlink`, the symmetric `grants.revoke` call removes the row; the
   guardian's reach to that child is physically gone.

Before this arm, step 4 fell through to `Denied`, so step 2 had to fall back to an in-process
seed and the write half of era-2 could not go live (tracked in the embedder at
`cc-app/docs/debugging/authz/grants-verbs-not-on-mcp-callback-surface.md`).

## Testing plan

Mandatory categories from `../testing/testing-scope.md` that apply:

- **Capability deny-test (§2):** a caller **without** `mcp:grants.assign:call` calling
  `grants.assign` over `/mcp/call` gets the opaque MCP `Denied` — no existence signal, no
  write. Same for `roles.define` / `teams.create` against their caps. This is the load-bearing
  test: the new arm must not open a hole.
- **Positive dispatch test:** a caller **with** the cap calling `grants.assign` over
  `/mcp/call` writes the grant (assert the row is readable via `grants.list_scoped` on the
  same transport) — the regression that this whole scope exists to fix.
- **Anti-widen still fires over the callback:** a caller holding `mcp:grants.assign:call` but
  **not** the target cap `X` calling `grants.assign(cap: X)` gets `BadInput("cannot grant a
  cap you lack")` — proving the handler's guard runs regardless of transport.
- **Workspace-isolation (§2):** a `grants.assign` call authenticated in workspace A cannot
  write a grant that resolves in workspace B (the `ws` comes from the token, not the body);
  a ws-B principal sees none of ws-A's grants.
- **Read/write symmetry:** the same `SidecarClient` that reads via `authz.scope_filter` writes
  via `grants.assign` and reads its own write back — one integration test over a real booted
  gateway (no mocks; real `Node`, real gateway, real store — testing-scope §0).
- **Aliased-verb gate agreement:** a workspace-admin token (which carries
  `mcp:teams.manage:call` / `mcp:roles.manage:call` / `mcp:grants.assign:call` but **no**
  `teams.create` / `roles.delete` / `grants.revoke` / `grants.list_scoped` caps by name) can
  call all nine verbs over `/mcp/call` — this is the regression test for approach §3; without
  the aliases exactly these four deny while the other five pass, which a naive
  assign-only test would miss.
- **Catalog coverage:** `host_catalog_covers_dispatch_prefixes` green (it fails automatically
  if §4 is skipped), and `tools.catalog` advertises `grants.assign` to a holder and hides it
  from a non-holder (the gate/visibility shared-alias rule).
- **REST route unregressed:** the existing `POST /admin/grants` admin-console path still
  assigns/revokes exactly as before (the arm is additive, not a move).

No new mandatory category is introduced; there is no hot-reload or offline surface here.

## Risks & hard problems

- **Deny-symmetry is the whole risk.** The one thing that must stay true: routing `grants.*`
  through MCP must not weaken the gate. It doesn't — `authorize` runs `mcp:grants.assign:call`
  before dispatch and `call_authz_tool` re-checks anti-widen — but the deny-test above is
  non-negotiable, because a native extension reaching a mint verb is precisely the surface an
  attacker would probe. A green deny-test is the exit gate.
- **Prefix collision:** confirm no *registered extension* can legitimately be named `grants`,
  `roles`, or `teams` (they'd now be shadowed by the host arm). These are reserved host verb
  namespaces already (the REST routes and `call_authz_tool` own them), so shadowing is correct,
  but state it so a future extension author isn't surprised — mirror how `authz.`/`invite.`/
  `series.` are already reserved.
- **Underestimation trap:** the discovery record called it a one-line change; it is four touch
  points, and the dangerous half is the `gate_tool_for` aliases — get one wrong (say,
  aliasing `grants.revoke` to a cap members hold) and you've widened a mint verb. Each alias
  must point at the exact cap the verb's *inner* gate already checks, nothing broader. And as
  before: ship any of it without the deny-test and you've shipped a potential escalation with
  no proof it isn't one.
- **Undo-journal asymmetry (accept, note):** MCP-dispatched calls at depth 0 are auto-captured
  into the undo journal (`call_tool_at_depth`); the REST admin routes are not. A grant write
  arriving over the callback therefore gains an undo entry its REST twin lacks. This is the
  platform-standard behavior for every bridged verb (not special to authz) and `undo` itself
  re-authorizes — but "zero behavior change" is precise only for existing callers; the new
  transport carries the bridge's standard semantics (undo capture, telemetry dispatch record,
  schema pre-validation).

## Open questions

- **Should `roles.*` / `teams.*` ship in the same arm, or only `grants.*`?** Recommendation:
  all three — `call_authz_tool` already handles them, the cap gate is identical, and splitting
  them just leaves the same gap for the next consumer to rediscover. Confirm no reason to
  withhold `roles`/`teams` from the native tier specifically.
- **Any other `call_authz_tool` verb that should be reachable but isn't prefix-covered?**
  Audit the handler's match arms against the dispatcher's prefixes once, so this fix is
  exhaustive rather than incremental (e.g. confirm `authz.resolve` / `authz.revoke-tokens`
  are already reached via the `authz.` arm — they are).

## Related

- `entity-scoped-grants-scope.md` — the scope this unblocks (its "Deriveable by extensions"
  goal); back-link this from there under its native-tier note.
- `authz-grants-scope.md` — the grant/role/team verbs `call_authz_tool` implements.
- `../mcp/mcp-scope.md` §"The contract", `../host-tools/host-tools-scope.md` — the dispatch seam.
- `../extensions/native-callback-transport-scope.md` — the `SidecarClient` callback transport.
- Downstream consumer / discovery record: `cc-app` (the childcare embedder) filed this gap at
  `docs/debugging/authz/grants-verbs-not-on-mcp-callback-surface.md` with the additive patch
  it validated against `node-v0.3.1`.

## Skill doc

N/A — this exposes no *new* agent-/API-drivable surface. The drivable verbs
(`grants.*`/`roles.*`/`teams.*`) already exist and are documented by
`authz-grants-scope.md`; this scope only adds a transport to reach them. If those verbs lack
a `skills/` entry today, that is `authz-grants`'s gap to close, not this one's.
