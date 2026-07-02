# Datasource Test/CRUD fails with opaque `denied` — DSN secret owned by the wrong principal

**Area:** datasources · **Status:** resolved · **Date:** 2026-07-02

## Symptom

From the Datasources UI (`#/t/acme/datasources`), pressing **Test** on a source
failed with `Failed: denied`. Telemetry showed the tool ran and was refused:

```
tool=datasource.list ... outcome=allow   msg=datasource.list allowed
tool=datasource.test ... outcome=deny    msg=datasource.test denied
```

`datasource.list` was **allowed** but `datasource.test` was **denied** — so it was not
a missing `mcp:datasource.test:call` cap (that chain is granted by dev-login, and list
proves the caller is authorized). Re-adding or removing the source from the UI *also*
returned `denied` (HTTP 403). The gateway was up, the `federation` sidecar was installed,
running, healthy, and `LB_FEDERATION_ENDPOINTS=127.0.0.1:5433` matched the source's
endpoint exactly — every "obvious" gate was green.

## Root cause

The DSN secret for a federated source was **owned by whichever admin ran
`datasource.add`**, but read/mediated under a **different, fixed** principal.

- `datasource.add` wrote the DSN via `lb_secrets::set_with(caller, …)` → the secret's
  `owner` was the *caller's* subject (the boot seed's `ext:federation-bootstrap`, or a
  dev login's `user:ada`).
- `secret::mediate_dsn` reads the DSN as `ext:federation` (the install-grant principal).

The secrets crate has a **gate-3 owner wall** (`crates/secrets/src/lib.rs`):
`set`/`delete` are **owner-only**, and `get` on a `Private` secret is owner-only. So:

1. A source seeded at boot (owner `ext:federation-bootstrap`) could not be **overwritten
   or removed** by a UI admin (`user:ada`) — the owner-only mutation wall returned
   `SecretsError::Denied`.
2. Every one of these collapsed to the **same opaque `denied` string**: `FederationError::Denied`,
   `EndpointRefused`, and a secret-owner denial all mapped to `ToolError::Denied` → 403
   `"denied"` at `role/gateway/src/routes/datasources.rs`. Nothing distinguished "you're not
   allowed" from "this source's secret is unreadable/uneditable" — which is why the true cause
   (an owner mismatch) was invisible for the whole investigation.

Confirmed by a control: a **freshly added** source (`add` writes a brand-new secret, no
owner to collide with) tested straight through to a real PostgreSQL connect error (HTTP
**500**, not 403) — proving the caps/endpoint/sidecar path was fine and the failure was
purely the secret's ownership.

## Fix

Every federation DSN secret is now owned by the **single stable `ext:federation`
principal**, decoupled from whoever runs the verb, so CRUD by any admin never collides:

- `federation::secret` is the one home for the mediator principal: `store_dsn` (write),
  `mediate_dsn` (read), `forget_dsn` (delete) all run as `ext:federation`.
- `datasource.add` writes through `store_dsn`; `datasource.remove` now `forget_dsn`s the
  secret so a later re-add starts clean (leaving it behind was the re-add collision).
- A new **`lb_secrets::reclaim`** verb writes-and-takes-ownership, bypassing only the
  gate-3 *owner-overwrite* check (gates 1 workspace + 2 capability still apply). `store_dsn`
  uses it, so a store poisoned by an earlier bootstrap owner **self-heals** on the next
  `add`/update. Owner-reset is narrow and deliberate — user-owned secrets keep the wall.
- The `ext:federation` mediator only holds `secret:federation/*:get` from its install
  grant; the host adds `:write` for the CRUD path exactly (the caller already passed the
  `datasource.*` cap gate; the value never crosses back to the caller).
- Debuggability: a distinct `FederationError::SecretUnavailable → ToolError::BadInput`
  ("datasource has no configured connection") so a missing DSN is no longer confused with
  a capability deny. (`EndpointRefused` stays opaque by security design.)

## Proof

Against the fixed `node` binary + a real spawned Postgres, the full lifecycle is green
(each step was previously `denied`/403):

```
seeded test → 200 · update DSN → 200 · test → 200 · REMOVE → 204 ·
re-add → 200 · test → 200 · NEW source add → 200 · test → 200
```

## Regression tests

- `crates/secrets/src/lib.rs` — `reclaim_takes_ownership_from_an_earlier_principal`
  (heals owner drift; owner-gated overwrite/delete then pass) and
  `reclaim_still_requires_the_write_cap` (gate 2 still enforced).
- `crates/host/tests/datasource_crud_ownership_test.rs` — CRUD by **two different
  admins** on one source without denial; heal of a secret owned by
  `ext:federation-bootstrap`; workspace isolation; DSN-never-in-`list` redaction. Real
  embedded SurrealDB, no Postgres/sidecar needed (add/remove + mediation never touch the
  child).

## Healing an already-poisoned dev store

Fresh boots are correct automatically. An existing dev store self-heals on the next
`add`/update of the source (via `reclaim`). To force-clean a source: remove it and re-add
it in the UI, or wipe `.lazybones/data/dev-store` and let the boot seed re-run.
