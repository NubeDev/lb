# Preview login fails "Failed to fetch" / "not a member" against the make dev node

- Area: app
- Status: resolved
- First seen: 2026-07-04
- Resolved: 2026-07-04
- Session: ../../sessions/app/app-preview-stale-session-session.md
- Regression test: rust/role/gateway/tests/identity_routes_test.rs::login_canonicalizes_a_bare_handle_to_the_user_principal

## Symptom
In the RN-web browser preview the login screen showed **"Failed to fetch"** on Sign in, or the
gateway returned **403 "not a member of any workspace"** for the prefilled `ada` / `acme`. The
preview looked broken even though the node was healthy.

## Reproduce
1. `make dev` (root) — node + gateway on **8080**, persistent store, seeded `LB_SEED_USER=user:ada`.
2. Open the RN preview pointed at that node: `…5310/?node=http://127.0.0.1:8080`, prefill `ada`/`acme`.
3. Sign in → 403 "not a member". (With no node on the prefilled port at all → "Failed to fetch".)

## Investigation
- "Failed to fetch" was the shallow half: the preview defaulted `?node=` to **8087** (the app's own
  `test_gateway`), but the user runs `make dev` whose node is on **8080** — nothing was listening on
  8087, so `fetch` rejected. Pointing at 8080 replaced that with a real reply: **403**.
- `curl -d '{"user":"ada",...}'` to 8080 → 403; `curl -d '{"user":"user:ada",...}'` → **200** with a
  token whose `sub` is `user:ada` and which lists the real channels. So the node was fine; the
  *handle form* was the difference.
- The node log showed `boot seed: user:ada is a workspace-admin member of acme` and live telemetry
  `actor=user:ada` — the member is `user:ada`, not `ada`.

## Root cause
The identity model keys on the **`user:<name>` principal** everywhere (the token `sub`, the
`membership` row, `created_by`, the seed `LB_SEED_USER=user:ada`), but the dev-login route
(`role/gateway/src/routes/login.rs`) used the request's `user` string **verbatim**. So a bare `ada`
was treated as a principal literally named `ada` — a different identity from the seeded `user:ada`.
`membership_login_resolve` then found `acme` already had members but not `ada` → `NotAMember` (403).
It only "worked" against an empty in-memory `test_gateway` because there `acme` had no members, so
the stranger `ada` bootstrapped itself as the first member (decision #3) — masking the bug until the
preview met a *populated* store.

## Fix
Canonicalize the login handle at the gateway edge, before every downstream use
(`role/gateway/src/routes/login.rs`):

```rust
let principal = if req.user.starts_with("user:") { req.user.clone() }
                else { format!("user:{}", req.user) };
```

`user_login_check`, `membership_login_resolve`, `dev_claims` (the token `sub`), the grant resolution
(which re-strips the prefix — grants are stored bare), and the `LoginReply.principal` all use
`principal`. So `ada` and `user:ada` resolve to the **same** identity on any node; an empty node
still bootstraps it. This is an edge normalization — core membership/`Subject` handling is unchanged
and nothing branches on an extension.

Two follow-ons that the fix exposed / needed:
- **Preview default port + prefill** (`app/shell/src/lib/dev-defaults.ts`,
  `app/shell/web/index.web.tsx`): default `nodeUrl` moved 8087 → **8080** (the `make dev` node the
  user actually runs); `?node=` still overrides for `make -C app dev`. The stale "8087 dodges the
  403" comment is corrected — the 403 is fixed at the source.
- **`addMember` harness** (`app/sdk/tests/harness.ts`): it passed a bare `"bob"` to
  `POST /admin/members`, which wrote a roster row keyed `bob` AND skipped the member-role grant
  (`membership_add`'s `bare_user()` only grants for a `user:`-prefixed sub). Bare `bob` "worked"
  only because the old `login("bob")` matched the bare row; once login canonicalizes to `user:bob`
  the mismatch surfaced as 403 in `channels.gateway.test.ts`. Fixed the harness to canonicalize to
  `user:bob` — the form `membership_add` documents it wants, which also lands the role grant.

## Verification
- `curl` bare `ada` → 200, `sub: user:ada`, channels listed (against both a rebuilt fixture node and
  the user's live `make dev` node).
- Playwright e2e (`…5310/?node=http://127.0.0.1:8080`, bare `ada` prefill): logs in and shows the real
  channels (`#123`, `#abc`, `#general`) — no "Failed to fetch", no "not a member".
- `cargo test -p lb-role-gateway` (incl. the new `login_canonicalizes_a_bare_handle_to_the_user_principal`)
  and `cargo test -p lb-host --test identity_membership_test --test authz_test` green;
  `app/sdk$ pnpm test:gateway` 17/17.

## Prevention
The login edge now owns handle→principal canonicalization, so no caller can accidentally mint or
match on a non-canonical identity. Lesson: a convenience input (a bare handle) must be normalized to
the canonical key at the single edge that admits it — leaving it raw made an in-memory node (which
bootstraps any stranger) hide a bug that a persistent, seeded node exposed. Cross-ref the twin
[[stale-preview-session-shows-empty]] (the same preview, the *session-liveness* half of the trap).
