# Extensions — the GitHub webhook-receiver role crate (session)

- Date: 2026-06-26
- Scope: ../../scope/extensions/github-webhook-scope.md
- Stage: S7 — platform maturity (STAGES.md). A follow-up resolving the explicit open item the
  `github-bridge` slice left (its scope: *"A webhook-receiver role crate that drives `ingest_via_bridge`
  on a real HTTP POST (with `X-Hub-Signature` verification) is the follow-up"*). The S7 exit gate was
  already MET.
- Status: done

## Goal

Ship `lb-role-github-webhook`: the **live HTTP ingress** for the coding workflow's inbound edge. The
`github-bridge` slice shipped `lb_host::ingest_via_bridge` (a typed host helper a test/UI drove); this
slice is the real `POST /webhook` that drives it from an actual GitHub delivery — verifying the
`X-Hub-Signature-256` HMAC first, then handing the raw body to the host edge.

## What changed

New role crate `rust/role/github-webhook/`, one responsibility per file (beside `lb-role-registry-host`;
roles depend on host, never the reverse):

- `src/verify.rs` — the HMAC verifier: `HMAC-SHA256(secret, raw-body)` vs `X-Hub-Signature-256`,
  **constant-time** comparison over the raw 32-byte MACs, opaque `SignatureError` (Missing / Malformed
  / Mismatch). The one piece with real security weight; unit-tested in-file.
- `src/state.rs` — `WebhookState` (the shared node + ingest principal + `ws` + the secret). The secret
  is `Arc<Vec<u8>>`, crate-private (`secret()` only the verifier calls), never logged.
- `src/routes.rs` — `post_webhook`: verify on the **raw `Bytes`** (before any parse), then
  `ingest_via_bridge`. Status mapping: `401` bad/absent signature · `403` authentic-but-denied · `422`
  malformed/non-UTF-8 · `200` ingested. Each leaks nothing.
- `src/server.rs` — `router`/`serve` split (mirrors registry-host/gateway), so tests drive the route
  with `tower::oneshot` *and* over a real bound port.
- `src/lib.rs` — the module wiring + the doc explaining the two-layer boundary.

Workspace: added `role/github-webhook` to `members` + the `lb-role-github-webhook` path dep, and
`hmac = "0.12"` to `[workspace.dependencies]` (pairs with the existing `sha2`; lives only in this role
crate). No core crate, no SDK/WIT, no capability grammar touched — flagged as a non-event by design
(the github-bridge slice was shaped to avoid an ABI change; the receiver as a role crate needs none).

## Decisions & alternatives

- **Verify the RAW body, before parsing.** The route takes `Bytes`, not a typed `Json<T>`. GitHub's MAC
  is over the exact bytes it sent; re-serializing parsed JSON changes key order/whitespace and never
  matches. Rejected: an extractor that parses first (would have silently broken every signature).
- **Constant-time compare.** A short-circuiting `==` on the MAC is a timing oracle that leaks how many
  leading bytes a forgery got right. Hand-rolled XOR-accumulate over the 32 raw bytes (no new dep; the
  codebase already hand-rolls hex in `lb_registry::digest_hex`). Rejected: pulling in `subtle` for one
  comparison.
- **`X-Hub-Signature-256` only, not the legacy SHA-1 `X-Hub-Signature`.** SHA-1 is broken for this; we
  reject deliveries that only carry the old header (treated as "no SHA-256 signature" → `401`).
- **Authenticity layered before authority, distinct statuses.** A forgery is `401` (not GitHub); an
  authentic-but-ungranted delivery is `403` (is GitHub, but unauthorized). Keeping them distinct makes
  the two-layer model legible and the deny-test meaningful — the signature gate is *not* the cap gate.
- **Secret as config bytes, not `lb-secrets`.** `lb-secrets` is still an S0 placeholder; the secret is
  mediated (crate-private, never logged) and moves behind that crate at its stage. Recorded as an open
  question, not silently skipped.
- **One receiver = one `(ws, principal, secret)`.** A repo→workspace routing front door is a follow-up
  (needs a directory); today the fixed `ws` IS the wall, which the isolation test proves.

## Tests

`role/github-webhook/tests/webhook_test.rs` (security boundary) + `webhook_ingest_test.rs` (ingest
behavior) + shared `common/mod.rs`, split to keep each file under the 400-line FILE-LAYOUT limit (the
same split the github-bridge tests use). Verifier units live in `src/verify.rs`. All HTTP tests run the
**real** `github_bridge_ext.wasm` installed through the registry + the real store/bus; the registry
`Source` + publisher keys are the only externals (testing §3). Mandatory categories, all present:

- **bad-signature:** forged sig / tampered body / absent header → `401`, and nothing reaches the inbox.
- **capability-deny:** an authentic delivery whose principal has no grants → `403`, ingests nothing.
- **workspace-isolation:** a ws-A receiver writes ws-A's inbox, leaves ws-B's untouched (shared node +
  node-global stateless bridge instance; the wall is principal+ws + the store).
- **idempotent re-delivery:** the same issue delivered twice (different `ts` → different bytes + sig)
  upserts ONE inbox item.
- **happy + real-socket:** a signed delivery `200`s and lands the canonical `acme/api#2451` triage item,
  over both `tower::oneshot` and a real bound port.
- **malformed payload:** an authentic but un-normalizable body → `422`.

Green output:

```
$ (cd rust/extensions/github-bridge && cargo build --target wasm32-wasip2 --release)
   Finished `release` profile [optimized] target(s) — github_bridge_ext.wasm

$ cargo test -p lb-role-github-webhook
running 5 tests (unittests src/lib.rs)
test verify::tests::a_correct_signature_verifies ... ok
test verify::tests::a_tampered_body_does_not_verify ... ok
test verify::tests::the_wrong_secret_does_not_verify ... ok
test verify::tests::a_missing_header_is_missing_not_a_pass ... ok
test verify::tests::malformed_headers_are_rejected ... ok
test result: ok. 5 passed; 0 failed

running 4 tests (tests/webhook_ingest_test.rs)
test a_signed_delivery_ingests_one_triage_item ... ok
test re_delivery_of_the_same_issue_is_idempotent ... ok
test a_malformed_payload_is_422 ... ok
test a_signed_delivery_round_trips_over_a_real_socket ... ok
test result: ok. 4 passed; 0 failed

running 3 tests (tests/webhook_test.rs)
test a_bad_or_missing_signature_is_401_and_ingests_nothing ... ok
test an_authentic_delivery_without_the_grants_is_403 ... ok
test a_ws_a_receiver_never_writes_ws_b ... ok
test result: ok. 3 passed; 0 failed

$ cargo build --workspace                 # green (node links the new crate)
$ cargo fmt -p lb-role-github-webhook --check    # clean
$ cargo clippy -p lb-role-github-webhook --tests # no warnings in the crate's files
$ git add -A && cd rust && bash scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (316 checked)
```

So **12 new** (5 verifier units + 4 ingest + 3 security) green; fmt + clippy + file-size clean; the
whole workspace still builds.

## Debugging

None — nothing broke. (The only friction was the test file landing at 433 lines; resolved by the
standard split into `_test` + `_ingest_test` + `common/mod.rs`, not a bug.)

## Public / scope updates

- Promote to `public/extensions/extensions.md` + `public/SCOPE.md`: the live webhook ingress ships as
  `lb-role-github-webhook` (HMAC-verify → `ingest_via_bridge`), completing the inbound edge.
- Resolve the github-bridge scope's webhook-receiver follow-up (back-linked from there). The new
  `github-webhook-scope.md` carries the remaining opens forward: multi-tenant routing, `lb-secrets`
  backing, and auto-start-on-approval (the latter cross-linked to the outbox follow-up).

## Dead ends / surprises

- **The whole slice needed no core change.** Because `ingest_via_bridge` already existed and carried
  both capability gates + idempotency, the receiver is pure HTTP↔host translation + one HMAC check. The
  honest slice is small — which is exactly what the github-bridge slice's seam was designed to enable.
- **No HTTP-client dev-dep for the socket test.** Rather than pull `reqwest` into the test deps for one
  round-trip, the real-socket test writes a raw `POST` over a `TcpStream` and parses the status line —
  enough to prove the server is wired, with zero new dependency.

## Follow-ups

- A **multi-tenant front door** (route a delivery to a workspace by repository; per-repo secrets) —
  needs a repo→ws directory.
- Move the secret behind **`lb-secrets`** when that crate lands.
- A **resolution reactor** that auto-starts the coding job on approval (the receiver lands
  `needs:triage`; the triage→approval→job chain still needs the outbox reactor) — cross-linked to the
  outbox follow-up.
- STATUS.md: add the slice row + note the github-bridge webhook follow-up resolved.
