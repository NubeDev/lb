# Registry — real HTTP `Source` + `registry-host` server (session)

- Date: 2026-06-26
- Scope: ../../scope/registry/registry-scope.md
- Stage: S7 — platform maturity (STAGES.md). An S7 **follow-up** (the S7 exit gate was already MET by
  the signed-registry + native-tier slices). This slice removes the registry's last mocked external:
  the in-memory `MapSource` test stub becomes a **real HTTP transport** — a `registry-host` server +
  an `HttpSource` client — with the verify-before-cache guarantee unchanged.
- Status: done

## Goal

Replace the only stubbed registry external with the real one: stand up the `lb-role-registry-host`
crate as an **HTTP server** that serves signed `Artifact`s by `(ext_id, version)`, and add an
**`HttpSource`** that implements the host's `Source` fetch seam over HTTP. The pull path is otherwise
untouched — the fetched bytes are still **untrusted** and `pull` still verifies-before-cache, so the
mandatory signing/offline/isolation guarantees hold over the wire exactly as they did in-memory.

## What changed

The registry's `Source` trait + `pull` verb already anticipated this (`source.rs`: "a real HTTP
`registry-host` client (S7 follow-up) … supplies the impl"). Nothing in the host service changes —
the seam was built for it. New code is all in the role crate, the transport unit:

- **`lb-role-registry-host`** (was a placeholder `lib.rs`) — the server + the matching client, one
  responsibility per file, mirroring the `lb-role-gateway` axum split (`router`/`serve` separate from
  state so tests drive routes with `tower::oneshot`):
  - `catalog.rs` — `ArtifactStore`: the server's in-memory `(ext_id, version) → Artifact` origin
    (the cloud catalog's stand-in; a real DB/object-store backing is a further follow-up). Can be
    flipped `offline` to model an unreachable origin.
  - `routes.rs` — `GET /artifacts/{ext_id}/{version}` → the signed `Artifact` as JSON (`404` if the
    origin lacks it / is offline — the two are indistinguishable to the client, matching `Source`'s
    contract). The server signs nothing and verifies nothing; it is a dumb origin.
  - `server.rs` — `router`/`serve` (the axum `Service`, bound-port serving split out).
  - `client.rs` — **`HttpSource`**: implements `lb_host::Source` by `GET`ting the artifact over HTTP
    and deserializing it. The bytes are UNTRUSTED — `HttpSource` does no verification; that is
    `pull`'s job, host-side, unchanged. A transport error / `404` maps to `NotAvailable` (offline).

- Workspace: `lb-role-registry-host` gains real deps (`axum`, `reqwest`, `tokio`, `lb-host`,
  `lb-registry`, serde). `reqwest` becomes a workspace dependency (the first outbound HTTP *client*
  in the tree — previously only a gateway dev-dep).

## Decisions & alternatives

- **`HttpSource` lives in the role crate, not in core `lb-host`.** The host owns the `Source` *trait*
  (the seam); a concrete HTTP impl needs `reqwest` (a TLS/HTTP client stack). Putting that in core
  `lb-host` weighs down every node — edge nodes that only ever pull from a cache, or front a UI, do
  not need an HTTP client compiled in. So the client lives beside its server in
  `lb-role-registry-host`, exactly as `lb-role-ai-gateway` owns the `ModelAccess` *impl* while host
  owns the trait. **Roles depend on host, never the reverse** (host `Cargo.toml` comment) — honored:
  the role crate depends on `lb_host::Source`, host stays oblivious to HTTP. Rejected adding `reqwest`
  to `lb-host` behind a feature flag: a feature gate on a core crate to keep an HTTP client optional
  is the symptom that the client doesn't belong in core at all.
- **The server is a dumb origin — it neither signs nor verifies.** Signing is the publisher's job
  (offline, with the publisher key); verification is the *client's* job (`verify_artifact`, against
  the workspace allow-list). A server that verified would be claiming an authority it doesn't have
  (it doesn't know which workspace trusts which key) and would tempt a future reader to trust the
  transport. Keeping it dumb makes the security property obvious: **the artifact is signed bytes; the
  wire is untrusted; trust is re-established client-side on arrival.** A tamper *in transit* is caught
  by exactly the same `verify_artifact` that catches a tamper at rest — proven by a test.
- **`404` = `NotAvailable`, same as the in-memory stub.** The `Source` contract already says "lacks
  it OR is unreachable … indistinguishable to the caller" (`source.rs`). HTTP makes that literal: a
  missing version and an unreachable server both surface as `NotAvailable`, and the offline-cached
  path never calls `fetch` at all — so a node with the artifact cached installs with the server down.
- **Catalog backing stays in-memory for now.** The slice's point is the *transport*, not durable
  cloud storage. A SurrealDB/object-store backing for the server, a publish endpoint (the outbox
  `Target` write, not a `Source` read — see the scope), and TLS/auth on the server are follow-ups.

## Tests

`rust/role/registry-host/tests/http_source_test.rs` — the HTTP transport end to end, booting a Node
(→ Zenoh peer) so multi-thread flavor + a unique workspace id per test. Mandatory categories applied
**at the new transport surface**:

- **happy / round-trip:** a signed artifact served by the `registry-host` server is pulled over HTTP
  by `HttpSource`, verified, cached, and installed through the real `hello` wasm — the install path,
  now over a real socket instead of the map.
- **offline (offline/sync category):** once cached, the same `(ext_id, version)` installs again with
  the **server shut down** — `HttpSource::fetch` is never called (the cached path short-circuits).
- **signing/verification over the wire:** an artifact **tampered in transit** (the server hands back
  bytes whose digest no longer matches the signature) is rejected with `Unverified` — the
  verify-before-cache guarantee holds over HTTP; nothing is cached.
- **workspace-isolation:** ws-B pulling over the same server never sees ws-A's cached artifact /
  catalog entry (the cache is workspace-namespaced; the server is shared, the trust + cache are not).
- **capability-deny:** `install_from_registry`/`pull` over the HTTP source is still refused without
  the grant — the gate is host-side and transport-independent (asserted via the same
  `authorize_registry` chokepoint).

Green output:

```
$ cargo test -p lb-role-registry-host --test http_source_test
running 5 tests
test install_over_http_is_denied_without_the_grant ... ok
test an_artifact_tampered_in_transit_is_rejected ... ok
test ws_b_cannot_see_ws_a_artifact_pulled_over_the_same_server ... ok
test installs_a_signed_artifact_pulled_over_http ... ok
test cached_artifact_installs_with_the_server_offline ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.61s

$ cargo test -p lb-host --test registry_test --test registry_offline_test \
    --test registry_isolation_test --test registry_rollback_test   # regression — unchanged, still green
registry_isolation_test:  ok. 2 passed
registry_offline_test:    ok. 3 passed
registry_rollback_test:   ok. 2 passed
registry_test:            ok. 4 passed

$ cargo fmt --check        # clean
$ bash rust/scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (304 checked)
$ cargo clippy -p lb-role-registry-host --all-targets   # no warnings in the new crate's files
```

So **5 new + 11 registry regression** green; fmt + file-size + clippy clean. The mandatory categories
(deny, workspace-isolation, offline, signing/verification) are all exercised over the real HTTP wire.

## Debugging

None — the slice composed the existing `Source` seam, which was built for exactly this. No behavior in
the host service changed, so no regression surfaced; the new transport passed on the first run.

## Public / scope updates

- Promote to `public/registry/registry.md` + `public/SCOPE.md`: the registry now has a real HTTP
  transport (server + client); the in-memory source is no longer the only origin.
- Resolve the scope open question "a real HTTP `Source`/`registry-host` server (the in-memory source
  is the only stub)" — shipped. The remaining registry follow-ups (durable publisher-key allow-list,
  key rotation, cache GC, public catalog union, `registry.update`, publish-as-outbox-effect, durable
  server backing + TLS/auth) stay open.

## Dead ends / surprises

- **The seam paid off — zero host changes.** The whole slice is a new role crate + a test; the host
  `registry` service didn't change a line. That's the `Source` abstraction working as designed: the
  in-memory stub and the real HTTP client are interchangeable behind one trait, so "swap the mock for
  the real thing" was literally a constructor swap (`MapSource::new` → `HttpSource::new`) in the test.
- **`reqwest` with `rustls-tls` (not native-tls)** to avoid a system OpenSSL build dependency — the
  same posture as the rest of the cloud stack (Zenoh already pulls in rustls).
- The isolation test asserts ws-B is rejected at *verification* (foreign trust set) rather than at the
  cache: the cache is workspace-namespaced, so ws-B never sees ws-A's cached bytes and is forced down
  the fetch+verify path — which is the stronger property (isolation holds even though the *server* is
  shared).

## Follow-ups

- Durable backing for the `registry-host` catalog (SurrealDB/object-store), a **publish** endpoint
  (an outbox `Target` write, not a `Source` read), and TLS/auth on the server.
- The other registry follow-ups remain in `scope/registry/`.
- STATUS.md: add the slice row + mark the registry HTTP transport shipped.
