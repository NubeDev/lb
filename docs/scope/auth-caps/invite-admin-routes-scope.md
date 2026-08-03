# Auth-caps scope — invite admin routes (the browser half of a shipped verb family)

Status: scope (the ask). Issue [#130](https://github.com/NubeDev/lb/issues/130). Promotes to
`doc-site/content/public/auth-caps/` once shipped.

> Read with: `invites-scope.md` (the owning scope — the record, the token, the accept surface, all
> **built**), `email-login-scope.md` (**shipped** — the global email+password an invite redeems
> into), `global-identity-scope.md` (**shipped** — identity + `membership.add`),
> `admin-crud-scope.md` (the admin-route conventions this follows).

`invites-scope.md` is implemented on the host side. `rust/crates/host/src/invites/` has
`create` / `list` / `revoke` / `resend`, each gated `mcp:invite.create:call`, dispatched through
`tool.rs` and reachable via the `"invite."` prefix in `tool_call.rs:69`. The redeem half is
reachable too: `POST /public/invite/accept` and `GET /public/invite/verify` are wired in
`role/gateway/src/server.rs:114-135`, both unauthenticated and IP-rate-limited.

**The consequence, verified against `node-v0.15.0`:** `grep '/admin/invites' role/gateway/src/server.rs`
returns nothing. Every *authenticated* invite verb is MCP-only. A browser holding a session bearer
— the admin console, the exact caller the scope was written for — has no route to mint an invite,
list pending ones, revoke, or resend. The redeem side of the door is hung and the mint side is not,
so the feature is unreachable from the product that needs it. Downstream, rubix-ai's setup wizard
ships an in-product apology for a flow whose backend is otherwise complete
(`NewWorkspaceWizard.tsx:169-176`).

This scope closes that: four thin gateway routes over the shipped host verbs, following the
`/admin/*` conventions already in `server.rs`. No new host logic, no new capability, no schema
change.

## Goals

- **`POST /admin/invites`** — mint. Body `{ email, role?, team?, payload?, locale?, expires_ts? }`,
  the argument set `invite_create` already takes. Returns `{ token }` — the plaintext, **once**;
  only its hash is stored.
- **`GET /admin/invites`** — the pending roster for the token's workspace. Returns invite records
  (email, role, team, status, expiry, audit) and **never** a token or token hash usable to redeem.
- **`POST /admin/invites/{token_hash}/revoke`** and **`/resend`** — over `invite_revoke` /
  `invite_resend`. Resend returns a fresh `{ token }`; the prior one is dead.
- **Every route re-checks `mcp:invite.create:call` server-side**, workspace and principal taken
  from the bearer, never the body or path. Identical to the `identity.*` and `membership.*` blocks
  at `server.rs:242-266`.
- **Route handlers stay thin.** One file, `role/gateway/src/routes/invites.rs`, that deserialises,
  calls the host verb, and maps the error. All authorization stays in `invites/*.rs` where it
  already lives — a second gate in the route would be a place for the two to drift apart.

## Non-goals

- **Any change to the invite record, token, or accept flow.** Those shipped in `invites-scope.md`
  and are load-bearing for the pre-auth routes. This scope is additive transport only.
- **Delivering the invite by email.** The route returns a link for the caller to send. Actual
  delivery is an outbox target (`../inbox-outbox/outbox-scope.md`) and lands separately — a
  copyable link is fully useful without it.
- **A password-reset token.** Still deferred, per `login-hardening-scope.md:43` and
  `email-login-scope.md:60-62`. An admin rotating via `identity.set_password` remains the recovery
  path; an emailed reset link is its own scope when the outbox target exists.
- **A new capability.** `mcp:invite.create:call` already covers create/list/revoke/resend
  (`builtin_roles.rs:723`). Splitting it now would be a caps-grammar change for no caller.

## Intent / approach

1. **`routes/invites.rs` — one file, four handlers**, mirroring `routes/identity.rs` in shape:
   typed body structs, `gw.now()` for the logical clock, host call, error map.
2. **Wire in `server.rs`** next to the existing invite comment block, so the pre-auth and admin
   halves of the same feature read together:
   `.route("/admin/invites", get(list_invites).post(create_invite))` and
   `.route("/admin/invites/{token_hash}/revoke", post(revoke_invite))` (likewise `/resend`).
3. **Token-hash in the path, not the token.** The host verbs key on `token_hash`, and it is the
   value `list` safely returns. *(Rejected: a numeric invite id — it would need a new index and a
   host signature change, for a value the caller already holds.)*
4. **Error mapping follows `identity.rs:146-150`:** denial → `403`, bad input → `400`, unknown or
   already-redeemed token → `404`, store failure → `500`. A revoked-vs-missing distinction is
   deliberately *not* exposed — same token-oracle reasoning as the pre-auth routes.
5. **No rate limit on these routes.** They sit behind the bearer and an admin cap; the IP limiter
   at `server.rs:122` exists because `accept`/`verify` are unauthenticated. Adding it here would
   throttle a legitimate admin bulk-onboarding a site.

Sequencing: entirely self-contained. Ships in one PR, then a `node-v*` tag that rubix-ai bumps to.

## How it fits the core

- **Tenancy / isolation.** Workspace comes from the bearer. `invite_list` is already
  workspace-scoped; the route passes `ws` from the principal and has no path to name another.
- **Capabilities.** `mcp:invite.create:call`, re-checked in the host verb. The deny is `403` with
  no body detail. Unchanged grammar, no new cap, no wildcard widening — note
  `builtin_roles.rs:43-50` on why broad `mcp:*.<verb>:call` patterns are forbidden here.
- **Placement.** Either — a gateway route on any node running the gateway role. No `if cloud`.
- **MCP surface.** Unchanged. These routes *consume* existing tools; they expose no new verb, so
  MCP remains the contract and HTTP remains a transport over it. Shape: **create + list + two
  targeted actions**; no live-feed (an invite roster is state, not motion) and no batch (an admin
  mints per-person; a bulk import is a job, and there is no caller for one).
- **Data (SurrealDB).** No new table or field. `rust/crates/authz/src/invite.rs:214-239` unchanged.
- **Bus / sync / secrets.** No bus subject. The plaintext token is the only secret: returned once
  by `create`/`resend`, never logged, never in `list`.

## Example flow

**Ada invites Bob into `acme`.**

1. `POST /admin/invites` with bearer, body `{ email: "bob@acme.com", role: "member" }`.
2. The route resolves principal + `ws=acme` from the token, calls `invite_create`, which checks
   `mcp:invite.create:call` and writes the record with a hashed token.
3. **`200`** `{ "token": "inv_…" }`. Ada's console renders the accept link once.
4. Bob opens it. `GET /public/invite/verify` previews it in the invite's locale;
   `POST /public/invite/accept` redeems into identity + membership + grants.
5. `GET /admin/invites` now shows the record as `accepted`, with redeem audit. No token field.

**The deny path.** A `member`-role principal calls `POST /admin/invites`. `authorize_tool` fails
in `invite_create` → **`403`**, no record written, no token generated.

**The stale-token path.** Ada calls `/revoke` on a hash already redeemed → **`404`**. The response
does not distinguish redeemed from never-existed.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability-deny tests** — one per route: a principal lacking `mcp:invite.create:call` gets
  `403` and no side effect. This is the category that must not be skipped for an admin surface.
- **Workspace-isolation** — an invite minted in `acme` is absent from `GET /admin/invites` under a
  `beta` bearer, and `/revoke` on its hash from `beta` is `404`, not `403` (no cross-workspace
  existence oracle).
- **No mocks (CLAUDE §9)** — real `mem://` store, real gateway, real principals through the lib
  API. No fake invite service.
- **Secret hygiene** — assert `GET /admin/invites` contains no redeemable token for any status, and
  that `create`/`resend` tokens appear in no log line.
- **Round trip** — mint via `POST /admin/invites`, redeem via `POST /public/invite/accept`, assert
  identity + membership + the granted role all exist and the caps are live. This is the case that
  proves the two halves of the door line up.
- **Resend invalidates** — the prior token fails `accept` with `404` after a resend.
- **Regression entry** — `debugging/auth-caps/invite-admin-route-drift.md` if the route set and the
  host verb set ever diverge again.

## Risks & hard problems

- **This gap is the real risk, and it is a class not an instance.** A verb family shipped with MCP
  dispatch and no gateway route is invisible to every test that exercises either side alone. Worth
  a coverage check for other host verbs with no `/admin/*` route — `invite.*` is unlikely to be the
  only one.
- **`token_hash` in a URL path.** It is not redeemable, but it is an identifier for a
  pending-invite record and will land in access logs and browser history. Acceptable — it is the
  same class as a resource id — but it must never be confused with the token in review.
- **Resend semantics are a footgun.** It returns a *new* token and kills the old, so an admin who
  resends after already sending the first link has silently broken it. The route is correct; the
  console must say so.

## Open questions

- **Should `list` support `status` filtering and paging?** A long-lived workspace accumulates
  accepted/expired records indefinitely. Recommend shipping `?status=pending` from day one — it is
  the only view a console actually renders, and retrofitting a filter after callers depend on the
  unfiltered shape is worse.
- **Should expired invites be reaped?** Recommend **no** in this scope: expiry is already a status,
  and the audit trail of who invited whom is worth keeping. Revisit if the table grows unbounded.
- **`POST` or `DELETE` for revoke?** Recommend `POST …/revoke`, matching the shipped
  `/admin/grants/revoke` and `/admin/authz/revoke-tokens` — revoke is a state transition with
  audit, not a row deletion.

## Related

- **The owning scope:** `invites-scope.md` — the record, token, accept surface and caps this scope
  merely exposes over HTTP. **Built**; this is its missing transport.
- Siblings: `email-login-scope.md` (**shipped** — what an invite redeems into),
  `global-identity-scope.md` (**shipped**), `login-hardening-scope.md` (**shipped** — the
  credential seam), `admin-crud-scope.md` (route conventions).
- **Downstream consumer:** `NubeIO/rubix-ai` → `docs/scope/frontend/credential-admin-scope.md` —
  the People-tab credential + invite UI, blocked on these routes for its invite half.
- Source: `rust/crates/host/src/invites/` (`create.rs`, `list.rs`, `revoke.rs`, `token.rs`,
  `tool.rs`), `rust/crates/authz/src/invite.rs:214-239`,
  `role/gateway/src/server.rs:114-135` (the pre-auth half), `role/gateway/src/routes/identity.rs`
  (the shape to mirror), `rust/crates/host/src/authz/builtin_roles.rs:723` (the cap).
