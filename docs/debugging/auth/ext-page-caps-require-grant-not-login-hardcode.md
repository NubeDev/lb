# An installed extension's page 403s — its tool caps were never granted, only ever hardcoded in the dev login

**Area:** auth (authz grants) · **Status:** resolved · **Date:** 2026-07-02

## Symptom

Opening the `control-engine` federated page (`/t/{ws}/ext/control-engine`) failed on load. Two
layers, one after the other as each was addressed:

1. **`out_of_scope: control-engine.appliance.list`** — thrown client-side by `makeBridge`. The
   page's effective bridge scope is the install's `[ui] scope` narrowed to the granted caps
   (`ui_decl::project` → `narrow`), and `appliance.list` had been dropped.
2. After widening the manifest/grant so the scope survived: **`403 denied`** from the host on the
   very same call.

## Root cause — extension caps had no path to a user except a login hardcode

The host gate (`caps::check` on `mcp:<ext>.<tool>:call`) checks the **caller's** token caps. The
only way those caps reached a dev user was `session/credentials.rs::member_caps()` — a hand-written
list where `proof-panel.*` and `thecrew`'s `assets.*` are literally enumerated. So every new
extension would have to edit the login to be usable. That is exactly the "gated caller as trusted
decider" anti-pattern the durable grant model (`authz-grants-scope.md`) exists to remove:

> Granting an extension's tool to a user/team is an **ordinary grant**; on login the session mints a
> token whose `caps = direct grants ∪ role caps ∪ team-inherited grants`.

The grant store (`grant_assign`/`resolve_caps`/roles) was fully shipped — but **the dev login
ignored it** (`dev_claims` returned only its hardcoded `member_caps()`), and **install granted
nothing to any subject**. So the intended data-driven path was never wired end to end.

A second, subtler trap: even granting caps to `role:workspace-admin` would NOT have resolved,
because `resolve_subject_caps` expanded a held `role:X` grant only via `role_caps(X)` (the role's
`role_define` **record**), and the built-in `workspace-admin` record is empty + immutable. Caps
`grant_assign`'d to the `Subject::Role("workspace-admin")` sat in the grant table unread.

## Fix — three generic changes, no per-extension code

1. **Install grants the UI surface to the admin role.** New `crates/host/src/authz/grant_ui.rs`:
   `grant_ui_scope_to_admin` grants each `[ui]`/`[[widget]]` scope tool (∩ what was actually
   `granted` at install) as `mcp:<tool>:call` to `role:workspace-admin`. Called from both
   `install_native` and the wasm `install`. Best-effort, idempotent, revocable from the admin
   console. Only the UI scope — the ext's internal store/net/callback caps stay with the sidecar.
2. **`resolve_subject_caps` folds a role subject's direct grants.** When a user holds `role:X`, it
   now expands BOTH `role_caps(X)` (the record) AND `resolve_subject_caps(Subject::Role(X))` (caps
   granted directly to the role) — recursively, dedup-bounded. This makes "grant a cap to a role" work
   without mutating an immutable built-in role record.
3. **Login merges the grant store.** `routes/login.rs` unions `resolve_caps(store, ws, user)` into
   the minted token's caps (best-effort — a store hiccup never fails login; the `dev_claims`
   wildcard base still mints a working session).

### The bug that made it still return 0 caps: a subject-name mismatch

After (1)–(3), an admin STILL resolved to 0 caps. Root cause: grants are stored under the **bare**
user name — the boot seed and the first-member bootstrap both do
`grant_assign(Subject::User(sub.strip_prefix("user:")), …)` → subject `user:ada`'s grant row is keyed
`user:ada` from `Subject::User("ada")`. But `routes/login.rs` called `resolve_caps(ws, req.user)` with
the **`user:`-prefixed** form, and `resolve_caps` re-wraps its arg as `Subject::User(arg)` →
`Subject::User("user:ada")` → key `user:user:ada` → matches **zero** rows. So even the user's own
`role:member`/`role:workspace-admin` grants didn't resolve, let alone the role's folded ext caps.
Fixed by stripping an optional `user:` prefix before `resolve_caps` (grants live under the bare name).
This was the last mile: with it, the dev token resolves **12** `mcp:control-engine.*:call` caps and
`/mcp/call control-engine.tree` returns the live engine's real tree.

### Also required for the callback path: `LB_GATEWAY_URL` in `make dev`

The registry verbs (`appliance.*`) reach the store via the sidecar's host callback, which needs
`LB_GATEWAY_URL` injected at spawn (`install_native` reads it from the node's env). The `make dev`
(and `cloud`) recipe set only `LB_GATEWAY_ADDR`, not `LB_GATEWAY_URL`, so every callback failed with
"no callback address". Fixed by adding `LB_GATEWAY_URL=$(GW_URL)` to both recipes.

## Proof (real gateway, e2e)

The dev token now carries all `mcp:control-engine.*:call` caps resolved from the grant store (not a
login edit). `/native/call control-engine.appliance.add` → `{"id":"local"}` **200**,
`appliance.list` → the seeded appliance **200** (both previously 403). authz tests
(`resolve_key_test`) + host native install/deny/isolation tests green.

## Lesson

An extension's user-facing caps belong in the **durable grant store**, granted to a role at install,
and the login must be a *projection* of that store — never a hand-edited allowlist. When a page 403s
with correct-looking manifest scope, check whether the caller's token actually resolves the cap
(`resolve_caps`), and whether anything ever granted it. Related: the callback-key boot-ordering bug
[extensions/sidecar-callback-401-key-minted-before-gateway.md](../extensions/sidecar-callback-401-key-minted-before-gateway.md).
