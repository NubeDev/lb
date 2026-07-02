# datasources — DSN secret owner wall broke cross-admin CRUD (session)

- Date: 2026-07-02
- Scope: ../../scope/datasources/datasources-scope.md
- Stage: S8 (data plane) — datasources shipped; this is a correctness fix beneath it
- Status: done

## Goal

Make UI-driven datasource CRUD (`#/t/acme/datasources`) work reliably for any admin — the user
reported **Test** failing with `Failed: denied`, and needs add/edit/remove of connections to "100%
work" long-term. Find and fix the root cause, not just unblock the one source.

## What changed

The DSN secret for a source was **owned by whoever ran `datasource.add`** (the boot seed's
`ext:federation-bootstrap`, or a dev login's `user:ada`) but read/deleted under the fixed
`ext:federation` mediator. The secrets **owner wall** (gate 3: overwrite/delete are owner-only) then
denied every cross-admin update/remove, and `Denied`/`EndpointRefused`/owner-denial all collapsed to
the same opaque `"denied"`.

Fix — every DSN secret is owned by the single stable `ext:federation` principal, decoupled from the
caller:

- [federation/secret.rs](../../../rust/crates/host/src/federation/secret.rs) — one home for the
  mediator: `store_dsn` (write), `mediate_dsn` (read), `forget_dsn` (delete). Read uses a mediator
  built from the install's real granted set (requires federation installed → correct opaque deny when
  it isn't); write/delete use a host-constructed mediator with fixed caps (does NOT depend on the
  install record — managing the extension's own secret is not gated on the grant record existing,
  which also keeps the gateway CRUD-route test that never installs the sidecar green).
- [federation/add.rs](../../../rust/crates/host/src/federation/add.rs) — writes via `store_dsn`.
- [federation/remove.rs](../../../rust/crates/host/src/federation/remove.rs) — `forget_dsn`s the
  secret so a later re-add starts clean.
- [secrets/lib.rs](../../../rust/crates/secrets/src/lib.rs) — new `reclaim` (write-and-take-ownership;
  gates 1 workspace + 2 capability still enforced, only the owner-overwrite check skipped). `store_dsn`
  uses it, so a poisoned store self-heals on the next add/update.
- [federation/error.rs](../../../rust/crates/host/src/federation/error.rs) (+ the
  [query_worker.rs](../../../rust/crates/host/src/channel/query_worker.rs) match) — distinct
  `SecretUnavailable → BadInput("datasource has no configured connection…")`; `EndpointRefused` stays
  opaque by security design.

## Decisions & alternatives

- **Owned by `ext:federation`, not the caller** — chosen because it's the single principal that both
  reads (mediation) and now writes/deletes, so the owner wall is a no-op between admins. Rejected
  "own by caller + make the secret `Workspace`-visible" (the prior state): Workspace visibility fixed
  *reads* but not *overwrite/delete*, which are owner-only regardless of visibility — the actual trap.
- **`reclaim` (owner-reset) over a migration script** — chosen so existing dev/prod stores heal
  transparently on first edit; a one-shot migration would miss stores that boot later. Kept it narrow
  (host-mediated secrets only; user-owned secrets keep the wall so one member can't overwrite
  another's).
- **Write mediator independent of the install record** — chosen after the gateway CRUD-route test
  (which deliberately never installs the sidecar) started failing 403: coupling the *write* authority
  to `read_install` wrongly made "register a source" require the runtime already be installed. Read
  still requires it (no runtime → no usable source).
- **Distinct `SecretUnavailable`, but `EndpointRefused` stays opaque** — a missing DSN is an
  actionable config state, safe to name; a `net:*`-omitted endpoint must stay opaque (existence/policy
  leak), per the reference-extension deny discipline.

## Tests

Mandatory categories present: **capability-deny** (`reclaim_still_requires_the_write_cap`; the
gateway route deny tests) and **workspace-isolation** (`crud_is_workspace_isolated`).

```
cargo test -p lb-secrets
  → test result: ok. 14 passed  (incl. reclaim_takes_ownership_from_an_earlier_principal,
                                  reclaim_still_requires_the_write_cap)

cargo test -p lb-host --test datasource_crud_ownership_test
  → test result: ok. 4 passed   (two-admins CRUD w/o denial; heal of an ext:federation-bootstrap
                                  -owned secret; workspace isolation; DSN-never-in-list redaction)

cargo test -p lb-role-gateway --test datasources_routes_test
  → test result: ok. 5 passed   (the two I initially broke, fixed by the write-mediator split)
```

End-to-end vs a REAL spawned Postgres (rebuilt `node` on :8099, temp store) — full lifecycle, each
step previously `denied`/403:

```
seeded test 200 · update DSN 200 · test 200 · REMOVE 204 · re-add 200 · test 200 ·
NEW source add 200 · test 200
```

`cargo fmt` clean; no new clippy warnings in touched files.

## Debugging

[debugging/datasources/test-denied-secret-owner-wall-across-admins.md](../../debugging/datasources/test-denied-secret-owner-wall-across-admins.md)
— full symptom → root cause → fix → proof, with the regression tests above. Indexed in
`debugging/README.md`.

## Public / scope updates

- Promoted the durable invariant to
  [public/datasources/datasources.md](../../public/datasources/datasources.md): a new "Single-owner
  DSN (CRUD invariant)" wall + refreshed the test-summary line.
- Scope open questions: **n/a** — `scope/datasources/datasources-scope.md` has no Open Questions
  section, and the secret-ownership mechanism sits beneath the scope's intent (it specifies "mediated
  DSN + admin CRUD", which this makes correct). Nothing to resolve or contradict.

## Skill docs

Updated [skills/datasources/SKILL.md](../../skills/datasources/SKILL.md) — the Gotchas now note the
single-owner/self-healing CRUD invariant and the distinct "no configured connection" error (was:
"denials are opaque"). Grounded in the live :8099 run above (real gateway + real Postgres).

## Dead ends / surprises

- The initial hypothesis chain (federation not installed → endpoint not approved → grant intersection
  empty) was all **wrong** — the sidecar was installed, healthy, endpoint-matched, and `ext-loader::grant`
  correctly covers the wildcard→specific net case. The opaque `"denied"` conflating three causes is
  exactly what sent the investigation down those paths; the control (a fresh source reaching a real
  500 connect error) is what isolated it to the secret's ownership.
- A `cargo build -p node` without `--features postgres` silently overwrites the postgres sidecar in
  `target/debug`, turning `test` into a 500 "postgres source not built in" — rebuilt with the feature
  to restore the user's setup.

## Follow-ups

- **The user must restart `make dev`** — the running :8080 node still has the pre-fix binary. Existing
  sources self-heal on first edit after restart.
- Unrelated: the branch has in-progress ce-v3 work ([native/install.rs](../../../rust/crates/host/src/native/install.rs)
  calls `grant_ui_scope_to_admin`, not yet exported from `authz/grant_ui.rs`) that currently breaks
  `cargo build --workspace` — **not** part of this fix (verified my changes build in isolation).
- STATUS.md: not updated — datasources is already listed as shipped; this is a bug-fix beneath it, not
  a slice/stage change.
