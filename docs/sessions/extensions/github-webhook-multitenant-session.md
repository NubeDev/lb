# github-webhook — the multi-tenant front door (session)

- Date: 2026-06-27
- Scope: ../../scope/extensions/github-webhook-scope.md (the top open question)
- Stage: S7 — platform maturity (STAGES.md). A webhook follow-up: one receiver process now fronts
  **many workspaces**, each with its own secret. The S7 exit gate was already MET.
- Status: done

## Goal

The webhook ingress shipped single-tenant: one receiver served one fixed `(ws, principal, secret)`.
Before one deployed node can serve more than one workspace, it needs a **front door that routes a
delivery to the right workspace** — and authenticates each tenant with its **own** secret, so one
tenant's secret can never be used to write into another's workspace.

## The design decision (route by URL slug, not by the body)

The per-tenant secret must be chosen **before** the HMAC check (authenticity-before-parse, §3.5).
But the repo identity lives *inside* the signed body — a chicken-and-egg. Two options:

- **Route by a path segment** `POST /webhook/{tenant}` — GitHub lets each repo point its Payload URL
  at a distinct path, so the tenant is known from the URL with zero trust in the unverified body.
- *Rejected:* parse the repo out of the body to pick the secret — that reads attacker-controlled,
  unverified bytes before authenticating them, inverting the security order.

Chose the path segment. The slug is **opaque** (e.g. `acme-api` → the `acme` workspace) so it need
not leak the tenant↔workspace mapping.

## What changed (all in `lb-role-github-webhook`, a role crate — `axum`/`hmac` stay out of core)

- `tenant.rs` (new): `WebhookTenant { principal, ws, secret }` (one tenant's binding) +
  `TenantRegistry { node, slug → WebhookTenant }` (the routing table + the shared node). The secret
  is crate-private, read only by the verifier — mediated exactly as in `WebhookState`.
- `route_tenant.rs` (new): `post_tenant_webhook` — resolve the tenant from the `{tenant}` path param,
  verify the raw body against **that tenant's** secret, then `ingest_via_bridge` into that tenant's
  `(ws, principal)`. Same status mapping as the single-tenant route, plus: an **unknown tenant folds
  into the `401`** (not a `404`) — no enumeration oracle.
- `server.rs`: `tenant_router` / `serve_tenants` beside the existing `router` / `serve`.
- `routes.rs`: `SIGNATURE_HEADER` made `pub(crate)` so both routes share the one header constant.
- `lib.rs`: export `TenantRegistry`, `WebhookTenant`, `tenant_router`, `serve_tenants`.

The single-tenant `/webhook` route + `WebhookState` are untouched (the one-repo deployment still
works) — the multi-tenant front door is layered *beside* it, not a rewrite.

## How it fits the core (the platform checklist)

- **Workspace is the hard wall — at the front door.** The secret is per-tenant. A delivery signed
  with tenant A's secret but POSTed to tenant B's slug fails B's HMAC → `401`, and never reaches B's
  workspace. The isolation test proves nothing lands in B's inbox.
- **Authenticity before authority, before parse.** Tenant resolved from the URL → HMAC over the raw
  bytes with that tenant's secret → only then the body is read and `ingest_via_bridge`'s cap+ws gates
  re-check the resolved principal. A `401` (forgery or unknown tenant) is indistinguishable to a
  prober; a `403` (authentic but ungranted) is distinct.
- **Symmetric nodes.** One process, many tenants, by config (the registry) — no `if cloud`, no
  per-tenant code branch. Each tenant is a row in a map.
- **No core/WIT/cap-grammar change.** A routing table + a second route handler; `ingest_via_bridge`
  and the two capability gates are reused verbatim.

## Tests (all green — pasted below)

`tests/webhook_tenant_test.rs` (4), driving `tenant_router` via `tower::oneshot`, one node fronting
two real tenants (the `github-bridge` installed in each):

- **per-tenant routing** — each delivery lands one item in *its* workspace's triage inbox, only there;
- **cross-tenant secret rejection** (the workspace-isolation headline) — A's secret on B's slug → `401`,
  nothing in B;
- **unknown tenant → `401`** (not `404`) — no enumeration oracle;
- **capability-deny** — an authentic delivery to a tenant whose principal lacks the ingest caps → `403`.

```
$ cargo test -p lb-role-github-webhook
   unittests src/lib.rs ............... ok. 5 passed
   tests/webhook_ingest_test.rs ....... ok. 4 passed
   tests/webhook_tenant_test.rs ....... ok. 4 passed   (NEW)
   tests/webhook_test.rs .............. ok. 3 passed

$ cargo fmt --all --check        # clean
$ bash scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (328 checked)
```

Net: **~210 Rust + 26 Vitest + 2 shell** tests green (+4 Rust this slice).

## Open questions still open

- **`lb-secrets` backing.** The per-tenant secret is still an in-process byte string; move it behind
  `lb-secrets` when that crate lands (the registry then holds a secret *handle*, not the bytes).
- **A dynamic tenant directory.** The registry is built once at boot. Onboarding a tenant without a
  restart (a durable repo→ws directory the front door reads) is the next step — same shape as the
  registry-host catalog's durable-backing follow-up.

## Cross-links

- Scope: ../../scope/extensions/github-webhook-scope.md (multi-tenant front-door question resolved;
  `lb-secrets` + dynamic directory still open).
- Public: ../../public/extensions/extensions.md (the ingress is now multi-tenant).
- Prior: ./github-webhook-session.md (the single-tenant ingress this extends),
  ../coding-workflow/close-the-loop-session.md (the reactor the ingest now feeds end to end).
