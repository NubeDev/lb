# Session — a native sidecar learns WHO called it, and can reach ABOUT a subject

- **Scope:** [`scope/extensions/native-caller-identity-scope.md`](../../scope/extensions/native-caller-identity-scope.md)
- **Siblings:** [`native-callback-transport-scope`](../../scope/extensions/native-callback-transport-scope.md)
  (child→host transport, the direction this reuses) ·
  [`auth-caps/entity-scoped-grants-scope`](../../scope/auth-caps/entity-scoped-grants-scope.md)
  (`check_scoped` / `scope_filter`, extended here).
- **Stage:** native tier follow-up. Brings the native (Tier-2) tier to parity with the wasm guest,
  which already delegates in-process via `call_with_ctx`.
- **Status:** shipped. Backend green — 5 delegated-reach tests + 1 real-spawned-sidecar frame test,
  plus 21 SDK unit tests (incl. two backward-compat), all pass. No regressions in the existing
  native / entity-scoped suites.
- **Date:** 2026-07-12.

## The two gaps, restated

A native sidecar could not enforce **per-caller row visibility** — the childcare product's sacred
guardian-isolation invariant — for two compounding reasons, both verified end to end downstream:

- **GAP A — the native call frame carried no caller.** `CallParams { tool, input }` was all the host
  sent the child on a routed `call`. `SidecarDispatch::call_tool` passed only `(qualified_tool,
  input_json)` through; its `ctx` argument was *ignored* (comment: "a sidecar has its own
  `LB_EXT_TOKEN` identity"). So the child could not know **who** invoked the verb — every dispatch
  defaulted to a synthetic admin, which bypasses any row filter. `grep` confirmed no
  `LB_EXT_PRINCIPAL_JSON` stamp anywhere in lb.
- **GAP B — the reach verbs only answered about the caller's own token.** `authz.check_scoped` /
  `authz.scope_filter` resolved `principal.sub()` and deliberately accepted no `subject`. A sidecar
  holds the *extension's* token, not the guardian's, and cannot mint a user token — so even knowing
  the caller, it could not ask "does `user:ana` reach `child:leo`?" over the callback.

Both are generic platform gaps (rule 10 — nothing here names cc-app / care).

## Decisions (with the alternative rejected)

1. **Subject-reach half → option (a): parameterize the existing verbs.** `authz.check_scoped` /
   `authz.scope_filter` gain an optional `subject`. Present ⇒ require the caller to hold the
   delegation cap, then resolve *that* subject's reach; **absent ⇒ byte-for-byte today's behaviour**,
   so every existing call site is untouched (proven: the entity-scoped suite is still green, and a
   dedicated `absent_subject_…` test). *Rejected — option (b), a sibling `check_scoped_for` verb:*
   duplicates two hot read verbs for no gain; (a) mirrors `grants.assign`'s existing `subject`.

2. **Projection fidelity → minimal.** The frame carries `{sub, ws, role, delegated}` — the least a
   per-caller row filter needs — not the full cap set. Least authority, smallest control-line frame
   (the scope's frame-size risk). The *subject's* caps are resolved server-side behind the delegation
   cap, so the caller's caps never need to ride the wire.

3. **Delegation cap → `mcp:authz.delegate_reach:call`, a grant-only marker.** It dispatches to no
   verb; its sole meaning is "may name a `subject`". This reuses the existing `message.render_recipient`
   pattern (a marker cap checked with `authorize_tool` / `holds_cap`, listed in the system catalog),
   so it needs **no** new `Surface`/`Action` in the grammar and is admin-revocable like any grant.

4. **Where the projection type lives.** `lb-supervisor` (native wire) and `lb-runtime` (the wasm
   `CallContext` vehicle) are siblings with no cross-dep, and neither should pull the other (supervisor
   is light; runtime pulls wasmtime). So the projection is defined **once per boundary** — a plain
   serde `Caller` in each — and the **host** (the one crate that knows both worlds) maps between them.
   Two tiny identical structs + one one-line mapper each is cheaper than a new inter-crate edge.

## What shipped

**GAP A — caller in the frame (additive, no `PROTOCOL_MAJOR` bump).**

- **Wire (SDK `lb-ext-sdk`, `sdk-v0.4.0`):** `CallParams` gains `caller: Option<Caller>`
  (`#[serde(default, skip_serializing_if = "Option::is_none")]`). `Caller = {sub, ws, role,
  delegated}`. Additive-by-absence: an old host omits it, an old child ignores it. `Tools` gains a
  `call_with_caller` default-method that forwards to `call`, so an identity-unaware extension needs
  no change; `serve` dispatches through it. Two backward-compat unit tests
  (`old_frame_without_caller_deserializes_to_none`, `absent_caller_is_omitted_on_the_wire`).
- **Host wire (`lb-supervisor`):** the same additive `caller` field on `lb_supervisor::CallParams`
  (the host's copy of the wire) + a mirror `Caller` struct. `Sidecar::call_with_caller(tool, input,
  caller)` stamps it; `Sidecar::call` delegates with `None` (the old frame, unchanged).
- **Threading:** `lb_runtime::CallContext` gains `caller: Option<Caller>` (the wasm guest ignores it;
  the native adapter serializes it). `build_call_context` (host) projects the already-authorized
  `&Principal` into it. `SidecarDispatch::call_tool` reads `ctx.caller`, maps `runtime::Caller →
  supervisor::Caller`, and passes it to `call_once_or_restart → call_with_caller` (re-stamped on the
  crash-retry, since a restarted child is a fresh process). The direct `native.call` verb
  (`native::tool::call_sidecar`) projects its `&Principal` through the shared `native::caller::project`
  so both entry points stamp identically.
- **Read is a read, not a re-mint:** the `mcp:<tool>:call` gate fired first (workspace-first) before
  the sidecar ever saw the call. The projection carries no signature — it is inert identity, never a
  bearer token. A cross-node routed call carries no `ctx`, so the frame is the old shape (single-node
  this slice, matching the scope's non-goal).

**GAP B — reach ABOUT a subject (`authz::scoped`).**

- `authz_check_scoped` / `authz_scope_filter` read an optional `subject`. A new `resolve_subject`
  helper: no `subject` → the caller's own bare sub (unchanged); `subject` present → require
  `holds_cap(principal, ws, "mcp:authz.delegate_reach:call")`, else **`ToolError::Denied`** (the
  opaque 403). **Fail closed** — a caller lacking the delegation cap never falls back to its own reach.
  The underlying `check_scoped_with` / `scope_filter_with` already took a `user: &str`, so resolving a
  different subject is one argument, no resolver change.
- The delegation marker cap is listed in `system/catalog.rs` (group `authz`) so the console sees it.

## Testing (real infra, no mocks — CLAUDE §9 / testing §0)

- **Frame carries the caller** — `native_caller_identity_test::frame_carries_the_authorized_caller_to_the_child`:
  a real node + real axum gateway on a real TCP port + a **real OS-spawned `echo-sidecar`**. The
  sidecar gained a `whoami` tool that reflects `params.caller`; a routed `POST /native/call` from
  `user:ana` asserts the child received the exact `{sub, ws, role:"member", delegated:false}`.
- **Backward compatible** — SDK `old_frame_without_caller_deserializes_to_none` (a no-`caller` frame →
  `None`, no panic) + `absent_caller_is_omitted_on_the_wire` (byte-identical to the pre-`caller` wire).
- **Delegated reach — allow** — `delegated_check_scoped_resolves_the_subject_not_the_caller` /
  `delegated_scope_filter_returns_the_subjects_rows`: a caller with the delegation cap gets the
  **subject's** reach (ana reaches leo) even though the caller holds no reach of its own.
- **Delegated reach — deny (the sacred one)** — `subject_without_delegation_cap_is_denied_never_falls_back`:
  a `subject` without the delegation cap is a 403 on both verbs — never a silent fallback.
- **Absent subject unchanged** — `absent_subject_resolves_the_callers_own_reach_without_the_delegation_cap`.
- **Cross-workspace isolation** — `delegated_subject_never_crosses_the_workspace_wall`: a ws-B caller
  naming a ws-A subject sees none of ws-A's grants (resolution reads only the caller's namespace).

All grants seeded through the **real** `POST /admin/grants` route; reach exercised through the **real**
`POST /mcp/call` bridge.

## Release story (what a downstream embedder bumps)

1. **SDK first — `sdk-v0.4.0`** (`lb-ext-sdk`): the additive `caller` on the native wire + the
   `call_with_caller` default-method. An out-of-tree native extension bumps `lb-ext-native` to this
   tag to read the caller and override `call_with_caller`. Additive: an extension on `sdk-v0.3.0`
   keeps working unchanged (the field is ignored, the default method forwards to `call`).
2. **Host next — the node release** (`node-v0.4.0` when cut): carries the `lb_supervisor` wire field,
   the `CallContext` threading, and the `subject` reach verbs + delegation cap. lb's own consumption
   of the SDK (`lb-sidecar-client` / `lb-sdk`) is **not** forced to bump — the transport and WIT world
   are untouched.
3. **Downstream (cc-app):** bump to `node-v0.4.0` (+ `sdk-v0.4.0` for its `care` sidecar), install its
   `care` extension **requesting `mcp:authz.delegate_reach:call`** (an admin approves exactly it), and
   flip its era-2 chokepoint on: read the caller from the frame, call `authz.check_scoped {…, subject:
   caller.sub}`. Its rule-7 assertions (stranger `child.get` → deny, stranger `child.list` → empty) go
   green with **no call-site change**, and the guardian-UI gate drops.

> Note for the maintainer: the SDK tag `sdk-v0.4.0` and the lb branch are **prepared locally, not
> pushed**. Push the SDK tag before cutting the node tag so the git-tag pin resolves on a fresh build.

## Files touched

- SDK (`lb-ext-sdk`, branch `native-caller-identity`, tag `sdk-v0.4.0`): `crates/lb-ext-native/src/{wire,serve,lib}.rs`, `Cargo.toml`.
- Wire: `rust/crates/supervisor/src/{rpc,sidecar,lib}.rs`.
- Threading: `rust/crates/runtime/src/{bridge,lib}.rs`; `rust/crates/host/src/tool_call.rs`;
  `rust/crates/host/src/native/{call,caller,tool,mod}.rs` (new `caller.rs`).
- Reach: `rust/crates/host/src/authz/scoped.rs`; `rust/crates/host/src/system/catalog.rs`.
- Fixtures / tests: `rust/extensions/echo-sidecar/{src/main.rs,extension.toml}` (the `whoami` probe);
  `rust/role/gateway/tests/{native_caller_identity_test,delegated_reach_test}.rs`.
