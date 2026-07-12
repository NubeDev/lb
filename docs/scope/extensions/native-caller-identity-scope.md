# Native caller-identity scope — a native sidecar learns WHO called it, and can enforce per-caller row scope

Status: **SHIPPED** (2026-07-12) — see
[`sessions/extensions/native-caller-identity-session.md`](../../sessions/extensions/native-caller-identity-session.md).
Both gaps closed generically + additively: GAP A ships `sdk-v0.4.0` (the additive `caller` on the
native wire); GAP B ships in the host (`subject` on `authz.check_scoped`/`scope_filter`, gated by
`mcp:authz.delegate_reach:call`). Decisions: subject-reach **(a)** (parameterize the verbs, absent
`subject` unchanged), projection **minimal** (`{sub, ws, role, delegated}`). Promotes to
`public/extensions/`. Topic:
[`extensions`](native-tier-scope.md) — the native-tier family. Sibling of
[`native-callback-transport-scope.md`](native-callback-transport-scope.md) (the child→host
transport) and [`native-callback-sdk-export-scope.md`](native-callback-sdk-export-scope.md)
(publishing that transport through the SDK). Consumes
[`../auth-caps/entity-scoped-grants-scope.md`](../auth-caps/entity-scoped-grants-scope.md).

> **Why this scope exists — the downstream forcing function.** `cc-app` (the childcare
> product embedding lb) put its entire domain behind ONE native (Tier-2) sidecar, `care`. Its
> defining, non-negotiable invariant (`cc-app` CLAUDE.md rule 7) is **guardian isolation**: a
> guardian may only ever read records for children they hold a live guardianship edge to. That
> invariant is enforced by a row-level chokepoint *inside* the sidecar. Standing the sidecar up
> on a real node proved the chokepoint **cannot** enforce it, for two reasons that are both
> lb-side gaps, not cc-app bugs:
>
> 1. **The native call frame carries no caller.** `CallParams { tool, input }`
>    (`lb-ext-native::wire`) is all the host sends the child on a routed `call`. The sidecar has
>    no way to know *who* invoked the verb, so every dispatch defaults to a synthetic admin — and
>    an admin bypasses the row filter. Result: a stranger guardian reads another family's child.
>    (Verified end-to-end: `cc-app` `tests/live_node.rs`.)
> 2. **`authz.check_scoped` / `scope_filter` only answer about the *caller's own* token.** Even
>    if the sidecar knew the caller is `user:ana`, the reach verbs
>    ([`entity-scoped-grants-scope`](../auth-caps/entity-scoped-grants-scope.md)) resolve the
>    caller principal's grants — and the sidecar's callback token is the *extension's* identity,
>    not the guardian's. A sidecar cannot mint a guardian token, so it cannot ask "does `user:ana`
>    reach `child:leo`?" over the callback.
>
> Both are generic platform gaps (rule 10: no product may be special-cased). A native extension
> that enforces per-caller row visibility is a general need — the ros/mqtt/control-engine sidecars
> hit the same wall the moment they want per-user scope. This scope closes it generically; `cc-app`
> then enforces rule 7 with **no call-site change** (its chokepoint already has the two-era shape).

## Goals

1. **The caller's identity reaches the sidecar on every `call`.** The host stamps the routed
   caller's principal into the native call frame; the child receives a verifiable projection of it
   (sub, ws, role, caps) and hands it to its verb layer. A sidecar's per-call authorization decision
   can finally be *about the caller*, not about the sidecar's own service identity.
2. **A granted extension can ask reach questions ABOUT the caller.** Either the reach verbs
   (`authz.check_scoped` / `scope_filter`) gain an **optional `subject`** the caller may name IFF it
   holds a new delegation cap, OR a sibling **delegated-reach** verb answers "does `<subject>` reach
   `(table,id)` under `cap`?" for a caller that holds the delegation grant. The extension resolves
   the CALLER's row scope through the wall, keyed on the caller the frame now carries.
3. **The wall stays the wall (rule 7 / no-widening).** The caller projection is a *read* of the
   already-authorized principal — the host's `mcp:<tool>:call` gate still fired first, workspace-
   first, before the sidecar ever saw the call. The delegation cap is an ordinary grant, revocable
   in the admin console; a sidecar without it can only ever learn its own reach (today's behaviour).
4. **Tier-agnostic + additive.** The wasm guest already runs in-process under the caller's delegated
   authority (`call_with_ctx`); this brings the *native* tier to parity. No `WORLD_MAJOR` bump beyond
   what an additive frame field requires; an old sidecar that ignores the new field is unaffected.

## Non-goals

- **No new domain surface.** This carries identity and answers reach; it adds no CRUD/list/watch.
- **No sidecar impersonation.** The frame projection is NOT a token the child can replay as the
  caller — it carries no signature the gateway would accept for a *new* call. The child uses it only
  to (a) attribute its own row-filter decision and (b) name a `subject` on a reach verb it is
  separately granted to delegate. A sidecar can never *act as* the caller against a third tool.
- **No cross-workspace reach.** The caller's ws is the frame's ws is the token's ws (structural, as
  today). A `subject` argument is always resolved within the caller's workspace.
- **No streaming identity.** Request/response `call` only (matches native-callback-transport's
  request/response non-goal). A per-subscriber identity on a native `watch` is a separate scope.
- **No change to the wasm guest path.** `call_with_ctx` already delegates in-process; this scope is
  the native-tier dual and leaves the guest bridge untouched.

## Intent / approach

Two additive pieces, each behind an existing seam:

1. **Caller in the call frame (the SDK + host halves).**
   - **Wire (SDK, `lb-ext-native`):** add an optional `caller` field to `CallParams` — a
     JSON projection of the routed principal (`sub`, `ws`, `role`, `caps`, and the `constraint` if
     the caller is itself delegated). Additive + versioned by absence: an old host omits it, an old
     child ignores it, so the frame stays backward compatible (the same additive-by-absence rule the
     manifest's `input_schema`/`emits_external` use).
   - **Host (`native/call.rs` → `SidecarDispatch`):** the dispatch adapter already holds the routed
     call; thread the authorized `&Principal` into `call_tool` and serialize its projection into the
     frame. The projection is a *read* of the principal the host already authorized — no new trust,
     no re-mint. The `serve_call`/`dispatch` trait grows one parameter (or a `CallCtx`), mirroring
     how the wasm path threads `CallContext` today.
   - **Child (`serve_stdio`):** parse `caller` and hand it to `Tools::call` (a new `call_with_caller`
     default-method, or a `CallCtx` argument) so the verb layer sees the real principal per dispatch
     instead of a synthetic fallback.

2. **Reach ABOUT a subject (the authz half).** Pick the smaller of:
   - **(a) `subject`-parameterized reach:** `authz.check_scoped { cap, table, id, subject? }` and
     `authz.scope_filter { cap, table, subject? }`. When `subject` is present, the host requires the
     CALLER to hold a new delegation cap (`mcp:authz.delegate_reach:call`, or a scoped
     `authz:reach:<table>:delegate`), then resolves `subject`'s scoped grants instead of the
     caller's. Absent `subject` → today's exact behaviour (caller's own reach), so every existing
     call site is unchanged.
   - **(b) a sibling verb** `authz.check_scoped_for { subject, cap, table, id }` gated by the same
     delegation cap, leaving `check_scoped`/`scope_filter` byte-for-byte unchanged.

   (a) is fewer verbs and mirrors `grants.assign`'s existing `subject` argument; (b) keeps the hot
   read verbs untouched. The implementing session picks one — both satisfy the goal.

The care chokepoint then, in era 2: read the caller from the frame; if the caller is a guardian, call
the reach verb with `subject = caller.sub`; the extension's own delegation grant (an ordinary install
grant) authorizes the delegated read. No cc-app call site changes — the chokepoint's `assert_reach` /
`reachable_children` signatures are identical; only their era-2 body learns to pass the frame caller.

## How it fits the core

- **Rule 7 / caps wall:** unchanged and still first. The host runs `mcp:<tool>:call` (workspace-
  first) BEFORE the sidecar sees the call; the caller projection is downstream of that gate, and the
  delegation cap is a normal second grant. Deny stays one opaque 403.
- **Tenancy:** the frame ws == the token ws == the request ws (structural isolation). A `subject`
  resolves only within that workspace.
- **Symmetry with the wasm tier:** the guest already gets the caller's authority via
  `call_with_ctx` + the in-process `host.call-tool` bridge under a delegation constraint. This scope
  is the out-of-process dual — the native tier reaches parity, no `if native` branch on the call path.
- **Rule 10:** entirely generic. Nothing here names care/cc-app; the frame field and the delegation
  cap serve any native extension that scopes rows per caller.

## Example flow

Ana (guardian, edge to Leo only) opens the family app → `POST /mcp/call care.child.get {id:"leo"}`
with her session token. Host: `mcp:care.child.get:call` ✓ (workspace-first) → routes to the `care`
sidecar, stamping `caller = {sub:"user:ana", ws:"acme", role:member, caps:[…]}` into the frame. The
sidecar's chokepoint reads the caller, calls `authz.check_scoped {cap:REACH_CAP, table:"child",
id:"leo", subject:"user:ana"}` over its callback (authorized by the extension's
`authz.delegate_reach` grant), gets `allowed:true`, returns Leo. Mallory (no edge) does the same for
`leo` → `check_scoped subject:"user:mallory"` → `allowed:false` → the sidecar denies (403 on
`get`, empty on `list`). Today, with no caller in the frame, BOTH resolve as the synthetic admin and
BOTH see Leo — the leak this scope closes.

## Testing plan

Real infra, no mocks (rule 4), mirroring `native-callback-transport`'s live-gateway harness:

- **Frame carries the caller:** a native test sidecar echoes back the `caller` it received; a routed
  `call` from a known principal asserts the projection matches (sub/ws/role/caps).
- **Backward compatible:** an old-shape frame (no `caller`) still dispatches; the child's fallback is
  exercised and does not panic.
- **Delegated reach — allow:** caller holds the delegation cap; `check_scoped subject:X` returns X's
  scoped grant, NOT the caller's.
- **Delegated reach — deny (the sacred one):** caller LACKS the delegation cap → `subject:X` is a
  403 (never a silent fallback to the caller's own reach — fail closed).
- **Cross-workspace:** a `subject` resolved from a ws-B caller sees none of ws-A's grants.
- **The downstream proof:** `cc-app`'s `tests/live_node.rs` rule-7 asserts (stranger `child.get` →
  deny, stranger `child.list` → empty) go green once cc-app bumps to the tag that ships this — the
  regression that is red today.

## Risks & hard problems

- **The projection must never be a replayable token.** It carries identity for *reading* a decision,
  not bearer authority. If a child could replay it as the caller against a third tool, that is an
  impersonation/widening bug — the projection must be inert (no accepted signature), and the ONLY
  privileged thing a child does with it is name a `subject` on a verb it is *separately* granted to
  delegate. Guard: the delegation cap is required for `subject`; the projection alone grants nothing.
- **Delegation-cap sprawl.** Over-granting `authz.delegate_reach` to an extension re-widens reach.
  It should be install-approved per extension (like any grant) and auditable; a care install requests
  exactly it, an admin approves exactly it.
- **Frame-size / churn.** The caps list can be large; consider projecting only what the reach path
  needs (sub/ws/role + a delegation marker) rather than the full cap set, if size matters on the
  control line.

## Open questions

- ✅ **(a) vs (b)** for the subject reach — **(a) shipped**: optional `subject` on
  `authz.check_scoped`/`scope_filter`, gated by `mcp:authz.delegate_reach:call`. Absent `subject` is
  byte-for-byte today's behaviour (proven by the still-green entity-scoped suite + a dedicated test).
- ✅ **Caller projection fidelity** — **minimal shipped**: `{sub, ws, role, delegated}`. The subject's
  caps are resolved server-side behind the delegation cap, so the caller's caps never ride the wire.
  **Follow-up (`sdk-v0.4.1` / `node-v0.4.1`): added `admin: bool`.** The minimal projection was a
  half-measure: `role` is *cosmetic* in lb (the gateway mints every session as `member`; admin power
  rides caps, never the role enum — `lb-role-gateway::session::credentials`), so a sidecar that
  bypasses its row filter for admins had NO usable signal — `role` said `member` for admins and
  guardians alike, and the projection deliberately omits caps. `admin` closes that: the host derives
  it once from the caller's caps (`lb_host::caps_hold_admin`, the admin-only cap delta) and hands the
  child one boolean. Additive-by-absence; a caller is only ever treated as LESS privileged if a stale
  host omits it. Surfaced downstream by `cc-app` `tests/live_node.rs` (admin reads denied because the
  chokepoint read the cosmetic role).
- ⬜ **Does the wasm guest want the same explicit `subject` verb** for symmetry? Left as-is — the
  guest's in-process `call_with_ctx` delegation is sufficient; the `subject` arg is available to a
  guest via `host.call-tool` too, gated by the same delegation cap, if a future guest wants it.

## Related

[`native-tier-scope.md`](native-tier-scope.md) ·
[`native-callback-transport-scope.md`](native-callback-transport-scope.md) ·
[`native-callback-sdk-export-scope.md`](native-callback-sdk-export-scope.md) ·
[`../auth-caps/entity-scoped-grants-scope.md`](../auth-caps/entity-scoped-grants-scope.md) ·
`../mcp/` (the one MCP contract) · downstream: `cc-app` `docs/scope/care/care-authz-scope.md`
(rule 7, the chokepoint that consumes this) + `cc-app` `docs/debugging/authz/`.
