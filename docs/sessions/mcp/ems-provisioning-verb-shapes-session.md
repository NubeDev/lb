# Session — confirm ems provisioning verb shapes (issue #48)

Date: 2026-07-14. Branch: `confirm-ems-provisioning-verb-shapes`. Answers
[NubeDev/lb#48](https://github.com/NubeDev/lb/issues/48). No lb-core code change — a
**confirmation** task: pin the wire shapes of the six host verbs ems's `CallbackProvisioner`
assumes, so ems (out of tree, in `rubix-ai-extensions`) can drop its guesses.

## What I did

Traced each verb from its dispatch arm to its reply serialization, then to a passing test that
asserts the reply shape — so nothing here is itself an assumption (rule 9). Wrote the authoritative
contract to `docs/scope/mcp/ems-provisioning-verb-shapes-scope.md`.

Sources read:
- `crates/host/src/rules/mod.rs:225,245` — `rules.save`→`{ id }`, `rules.delete { id }`→`{ ok: true }`.
  **There is no `rules.create`.**
- `crates/host/src/ingest/tool.rs:54` + `crates/ingest/src/read.rs` + `crates/ingest/src/sample.rs`
  — `series.latest`→`{ sample: Sample | null }`; value/ts live in `sample.payload` / `sample.ts`.
- `crates/host/src/authz/scoped.rs` — `authz.check_scoped`→`{ allowed }`,
  `authz.scope_filter`→`{ filter: "all" | { ids } }`.
- `crates/host/src/authz/tool.rs:28,41` + `crates/authz/src/scope.rs` — `grants.assign`/`revoke`
  →`{ ok: true }`, `Scope::Ids` on the wire as `{ kind: "ids", table, ids }`.

## Outcome — 4 of 6 ems assumptions correct, 2 wrong

- ✅ `authz.check_scoped`, `authz.scope_filter`, `grants.assign`, `grants.revoke` — exact match.
- ❌ `rules.create { name, body }→{ rule_id }` — the verb is `rules.save`, cap `mcp:rules.save:call`
  (not `mcp:rules.create:call`), reply `{ id }`. `rules.delete` arg is `id`, not `rule_id`.
- ❌ `series.latest`→`{ value, ts }` — actually `{ sample }`; read `sample.payload` / `sample.ts`.

All six are callable by a native sidecar over `/mcp/call` under the standard `authorize_tool` gate,
provided the sidecar holds the matching cap — with the `rules.*` cap-name correction above.

## Tests

No new code, so no new lb-core test. The contracts I documented are each anchored to an existing
green test (cited in the scope doc's table): `authz_scoped_test.rs:83,99,137,185,306`,
`ingest_test.rs:91` (`latest["sample"]["seq"]`), `rules_test.rs:126` (`rules.save { id, body }`).
Ran the affected suites to confirm still green:

```
cargo test -p lb-host --test authz_scoped_test --test ingest_test --test rules_test
```

(see terminal output in the session — all green). The **new** regression this issue really wants
lives in the ems repo (a CallbackProvisioner test against a real spawned node, per rule 9); that is
tracked there once ems applies the corrections. Filed the correction list back on the issue and
closed it.

## Follow-up

None for lb-core. ems edits `provisioning/callback.rs` (one file) per the scope doc.
