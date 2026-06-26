# Extensions scope — the GitHub webhook-receiver role crate

> The live HTTP ingress for the coding workflow's inbound edge. Resolves the explicit follow-up the
> [`github-bridge`](github-bridge-scope.md) slice left open: that slice shipped
> `lb_host::ingest_via_bridge` (a typed host helper a test/UI drove); this crate is the real
> `POST /webhook` that drives it from an actual GitHub delivery.

- Stage: S7 — platform maturity (STAGES.md). A follow-up; the S7 exit gate is already MET.
- Topic: `extensions` (the role layer; beside [`lb-role-registry-host`](../registry/registry-scope.md)).

## Goals

- A real HTTP edge (`lb-role-github-webhook`) that accepts a GitHub webhook `POST`, verifies its
  `X-Hub-Signature-256` HMAC, and drives `ingest_via_bridge` — closing the live ingress path.
- Prove the signature gate is **transport authenticity**, layered *before* — never instead of —
  the host's two capability gates and the workspace wall.
- Mediate the webhook secret: hold it behind state, read it only in the constant-time HMAC check,
  and never log it or leak it in an error body.

## Non-goals

- **Multi-tenant routing by repo.** One receiver serves one `(ws, principal, secret)`. A front door
  that routes a delivery to a workspace by repository is a follow-up (needs a repo→ws directory).
- **A `lb-secrets`-backed secret.** `lb-secrets` is still an S0 placeholder; the secret is passed in
  as config bytes for now (mediated, never logged). It moves behind `lb-secrets` at that crate's stage.
- **Re-implementing normalization or the orchestrator.** The bridge normalizes; the host writes the
  inbox and drives triage→approval→job. The receiver only does HTTP↔host translation.
- **TLS / a verified login session.** TLS terminates at the deployment's ingress; a real
  login→token→principal lands with the gateway's auth follow-up. The receiver uses a fixed principal.

## Intent / approach

The receiver **is a node** (symmetric nodes, §3.1) that also exposes one inbound route. It adds no
authority. Two layers guard it:

1. **Transport authenticity** — `HMAC-SHA256(secret, raw-body)` against `X-Hub-Signature-256`,
   compared in **constant time**, over the **exact raw bytes** GitHub signed (re-serializing parsed
   JSON would change bytes and never match). A failure is an opaque `401` — no oracle, no secret leak.
2. **Capability + workspace** — a verified delivery calls `ingest_via_bridge` under a fixed
   principal/workspace, so the SAME two host gates (`mcp:github-bridge.normalize:call`, then
   `mcp:workflow.ingest_issue:call`) and the workspace wall guard it as they guard every other caller.

`axum` + `hmac` live in the role crate, never in core `lb-host` (roles depend on host, never the
reverse — the same rule that keeps `reqwest` in `lb-role-registry-host`).

## How it fits the core

- **Capabilities.** No new capability. The signature gate is *authenticity*, not authority; authority
  stays the existing two `mcp:*:call` gates inside `ingest_via_bridge`. Deny-test: an authentic
  delivery whose principal lacks the grants is `403` (distinct from the `401` forgery case).
- **Tenancy/isolation.** The receiver writes the fixed `ws` it was built with. Isolation-test: a
  receiver fronting ws-A writes ws-A's inbox and never ws-B's, even sharing one node + the node-global
  bridge instance (the wall is principal+ws + the store, per the github-bridge finding).
- **Data / bus.** No new records or subjects — it reuses the inbox `triage` upsert `ingest_via_bridge`
  already performs. Idempotency on the normalized issue id makes GitHub's re-delivery one item.
- **Sync/authority.** n/a — the receiver is a stateless edge; the durable truth is the inbox item.

## MCP surface

None. The receiver is an HTTP edge, not an MCP tool — it *calls* the existing `github-bridge.normalize`
and `workflow.ingest_issue` tools through `ingest_via_bridge`. (A future `webhook.*` admin surface to
register a repo→ws mapping would be its own slice.)

## Testing plan

`role/github-webhook/tests/` (split `webhook_test.rs` security + `webhook_ingest_test.rs` behavior +
shared `common/mod.rs`, to stay under 400 lines), plus unit tests of the HMAC verifier in `verify.rs`:

- **bad-signature (mandatory):** a forged / tampered-body / absent header is `401` and ingests nothing.
- **capability-deny (mandatory):** an authentic delivery with no grants is `403`, ingests nothing.
- **workspace-isolation (mandatory):** a ws-A receiver writes ws-A's inbox, never ws-B's.
- **idempotent re-delivery (mandatory):** the same issue delivered twice → one inbox item.
- **happy + real-socket:** a signed delivery `200`s and lands one canonical triage item, over both
  `tower::oneshot` and a real bound port.
- **malformed payload:** an authentic but un-normalizable body is `422` (distinct from `401`/`403`).
- **verifier units:** correct / tampered / wrong-secret / missing / malformed-header.

## Risks & hard problems

- **Timing oracle.** A short-circuiting compare on the MAC leaks how many bytes matched. Mitigated by a
  constant-time XOR-accumulate over the raw 32-byte MACs.
- **Signing the wrong bytes.** Verifying over re-serialized JSON never matches GitHub's MAC. Mitigated
  by hashing the raw body before any parse (the route takes `Bytes`, not a typed `Json`).
- **Secret leakage.** A detailed error ("expected X") is an oracle. Mitigated by one opaque
  `SignatureError` → a bare `401`, and a crate-private secret never logged.

## Open questions

- ~~**Multi-tenant front door.**~~ **RESOLVED (S7):** `tenant_router` (`POST /webhook/{tenant}`) +
  a `TenantRegistry` (slug → `{ws, principal, secret}`) front many workspaces from one process, each
  with its own secret. Routing is by URL slug (chosen before the HMAC check, so authenticity-before-
  parse holds); a delivery signed with one tenant's secret can't cross into another's workspace, and
  an unknown tenant is an opaque `401` (no enumeration oracle). See
  `../../sessions/extensions/github-webhook-multitenant-session.md`. *Still open here:* a **dynamic**
  tenant directory (the registry is built at boot — onboarding without a restart, a durable repo→ws
  directory, is the next step).
- **`lb-secrets` backing.** Move the (now per-tenant) secret behind `lb-secrets` when that crate lands
  — the registry would hold a secret *handle*, not the bytes. (Open.)
- ~~**Auto-start on approval.**~~ **RESOLVED (S7):** the triage→approval→job chain now auto-closes via
  `react_to_approvals` (the resolution reactor). See `../../sessions/coding-workflow/close-the-loop-session.md`.

## Status

**Shipped.** See [`sessions/extensions/github-webhook-session.md`](../../sessions/extensions/github-webhook-session.md)
and `public/extensions/extensions.md`. The mandatory categories and the verifier units are green.
