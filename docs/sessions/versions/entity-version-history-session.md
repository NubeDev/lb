# Versions — generic entity version history + restore (session)

- Date: 2026-07-28
- Scope: ../../scope/versions/entity-version-history-scope.md (tracked as **NubeDev/lb#112**)
- Downstream: `NubeIO/rubix-ai` → `docs/scope/frontend/version-history-scope.md` (**NubeIO/rubix-ai#55**)
- Stage: S10 follow-on (sibling of the undo retrofit)
- Status: done (unreleased — see Follow-ups)

## Goal

Ship the engine half of entity version history: a capped, per-entity ring of full after-image
snapshots of every dashboard, flow, and rule, written automatically at the depth-0 dispatch
chokepoint, with `versions.list` / `versions.get` / `versions.restore` / `versions.config.get|set`
verbs — generic by construction, so adding a kind is adding a row to a plan table. Plus the
structural `Secret<T>` never-in-a-snapshot guard that `undo-exposure-scope.md` names as the hard
prerequisite for widening any captured floor.

## What changed

**The prerequisite, first (it gates everything else):**

- `crates/store/src/snapshot_guard.rs` — `snapshot_safety(table, value)`. Two structural layers:
  the **type** (`lb_telemetry::Secret<T>` has no `Serialize`, so secret material cannot become the
  JSON a snapshot is made of — a compile error, the strongest form available) and the **JSON
  boundary** (a snapshot is refused if the table is part of the secret plane, or if the value
  carries a non-empty string under an unambiguous secret key name at any depth).
- `crates/store/src/reserved.rs` — `entity_version` + `versions_config` are reserved (host-owned):
  a forged version row becomes a write to the real entity the moment someone restores it.
- `crates/store/src/capped.rs` — added `ulid_timestamp_ms`, so exactly one crate knows the ULID
  format (see the `ts` decision below).

**The subsystem** — `crates/host/src/versions/`, one responsibility per file:

| file | responsibility |
|---|---|
| `plan.rs` | the KIND PLAN TABLE + the pure classification of a dispatched call |
| `record.rs` | the `entity_version` row, the ring key, the stable snapshot hash |
| `cap.rs` | cap resolution: const → workspace → per-kind, node-clamped |
| `store.rs` | the only place the ring table is queried |
| `capture.rs` | the depth-0 after-image capture (sibling of `undo_capture`) |
| `list.rs` / `get.rs` / `restore.rs` / `config.rs` | one verb each |
| `descriptor.rs` | the JSON Schemas the catalog + arg validator serve |
| `error.rs` | the service error, opaque denial |

**Wiring:**

- `tool_call.rs` — `"versions."` added to `HOST_NATIVE_PREFIXES`; a `versions.` arm in
  `run_host_verb` (threading `depth`, because restore re-enters the dispatcher); and the capture
  hook at the depth-0 chokepoint, sequenced **after** the undo-capture wrapper.
- `undo_capture/plan.rs` — a `versions.restore` arm resolving `(table, id)` **through the versions
  kind plan table**, so a restore is undoable and a new versioned kind becomes undoable with no
  edit here. The three versions reads joined `is_read_only`.
- `system/catalog/versions.rs` + `FAMILIES`; `tools/descriptor.rs` extends with the five schemas.
- `authz/builtin_roles.rs` — `versions.list` / `.get` / `.config.get` at VIEWER tier,
  `versions.restore` at AUTHOR tier, `versions.config.set` in `ADMIN_ONLY_CAPS`.
- `role/gateway/src/routes/versions.rs` + `server.rs` — five typed routes (`kind` travels as an
  opaque path segment).

## Decisions & alternatives

Everything below is a judgment call the scope left to implementation, or one the implementation
forced. Recorded because the diff shows the *what*.

1. **The secret guard refuses, it does not redact.** A redacted snapshot *looks* restorable and is
   not — restoring it would write `"***"` over a live credential, which is the silent-wrong-restore
   class the undo scope exists to prevent. A refusal costs the ring one version and logs loudly.
   Rejected: redact-and-store; rejected: adding a `Serialize` impl to `Secret<T>` that emits `***`
   (it would trade a compile error for a silent one).
2. **The secret-key list is chosen for unambiguity, not coverage.** `dsn`/`url`/`endpoint` are
   deliberately absent: they are ordinary fields on records the platform snapshots today, and a
   false positive costs a silently-missing version. Widening the list is a deliberate act with a
   test, exactly like widening the capture floor.
3. **`ts` is decoded from the row's ULID, not read from a clock.** Core verbs take their logical
   `now` from arguments (the undo journal writes `ts: 0`), but a version list is a *human* surface —
   "2 minutes ago — ada" is the point — and two of the three v1 save verbs carry no `now` to borrow.
   The ring already mints a ULID per row for FIFO ordering, and a ULID encodes its mint time, so
   decoding it adds no clock call to any verb and cannot disagree with the ring's own ordering.
   Unix **millis** on the wire.
4. **`hash_ignore` on the plan table — the dedupe would otherwise never fire.** Discovered by a
   failing test, not by reading: a dashboard stamps `updated_ts` on every save and a flow BUMPS its
   `version` counter on every save, so re-saving an unchanged record produced a byte-different
   snapshot and burned a ring slot. Excluding those *from the comparison only* (they stay in the
   stored snapshot — a restore must write them back) is what makes "a no-op save must not burn a
   slot" true. Declared per kind in the plan table so nothing downstream matches on a kind name.
5. **The snapshot hash is FNV-1a over a canonically-key-sorted rendering**, not `DefaultHasher`:
   these hashes are persisted and compared across node restarts and Rust releases, and
   `DefaultHasher`'s output is stable across neither. Key order is normalised (the store's JSON map
   preserves insertion order); array order is content.
6. **`is_head` compares CONTENT, not "is it the newest row".** Computed per read against the live
   record, never stored. This keeps the "current" marker honest after a dedupe skip or a trim, and
   makes it correctly absent for a deleted entity.
7. **Restore takes an optional `now`, and derives a FORWARD one when absent.** The restored record's
   `updated_ts` is "when it was restored", not "when the old version was saved", so the version
   row's own `ts` is deliberately not reused.
8. **The ring's `id` wins over the snapshot's.** `restore_input` overwrites `id` with the entity the
   ring row belongs to, so a snapshot that disagrees cannot redirect the save at another record.
9. **The no-escalation check runs BEFORE the snapshot read.** Both orderings refuse, but only the
   pre-check refuses *identically* for a real version id and a fictional one. Checking the snapshot
   first turns the deny path into an existence oracle — "denied" means real, "no such tool" means
   not — letting an unauthorized caller enumerate another author's history one guess at a time.
   (This ordering was initially untested; see Dead ends.)
10. **`versions.config.set` rejects an out-of-range cap rather than silently clamping it.** An admin
    who types 500 must be told the ceiling is 100, not quietly given 100 and left believing
    otherwise. It also rejects an unknown kind in `per_kind`: a typo that persists *looks*
    configured and does nothing.
11. **`versions.get` returns `is_head: false` by construction.** The marker is a list-level fact;
    computing it on a single get would cost a second read for a flag the caller already has.
12. **Rule 10 posture.** The plan table names core-owned *verb families* (`dashboard.save`), never
    extension ids — the host may name its own verbs. Nothing below `plan.rs` sees a kind name: the
    gateway treats `kind` as an opaque path segment, the descriptors derive their `enum` from the
    table, and `undo_capture` reaches the table through `table_for_kind`. A future
    extension-declared kind is a row, not an edit.
13. **`SELECT` enumerates columns.** `SELECT *` returns SurrealDB's own `id` (a `Thing`, not JSON)
    and fails to decode; and the `ORDER BY` idiom must be literally selected, so the list orders by
    `version_id` — the same ULID `capped_insert` writes into `seq`.

## Tests

Real infra throughout (rule #9): booted `Node`, in-memory store, dispatched through the actual
`call_tool` MCP bridge, real save verbs. **No test writes an `entity_version` row by hand** — every
row in every assertion came from the capture seam, which is the only way these tests can prove
capture happens at all. (The one exception is deliberate and labelled: the validator test overwrites
a stored snapshot in place, because "a snapshot that WAS valid and is not now" cannot be produced by
any sequence of real calls inside one test.)

Mandatory categories: **capability-deny** ✅ (four tests, including the scope's named deny), plus the
downstream **degradation contract** (a viewer-tier caller must SEE `versions.list` in its own
`tools.catalog` and NOT see `versions.restore` — that catalog read is what the shell's
`versionsAvailable()` probe hides every entry point on, so a cap bug there would look like an old
node rather than a bug),
**workspace-isolation** ✅. **Offline/sync**: n/a beyond what already holds — version rows are
ordinary workspace-scoped store records and a restore is a plain forward save, so they inherit the
store's sync behaviour with no new authority or predicate (the scope says as much); the undo scope's
`undo_sync_test` covers the conditional-restore case that *does* have one. **Hot-reload**: n/a — no
extension surface.

```
$ cargo test -p lb-host --lib versions::
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 356 filtered out

$ cargo test -p lb-store --lib snapshot_guard
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out

$ cargo test -p lb-host --test versions_test --test versions_authz_test --no-fail-fast
running 9 tests
test restoring_a_deleted_rule_recreates_it ... ok
test versions_verbs_are_in_the_catalog_with_validating_args ... ok
test a_snapshot_the_validators_now_refuse_is_not_written ... ok
test an_identical_save_is_deduped_by_snapshot_hash ... ok
test restoring_a_dashboard_round_trips_and_appends_a_new_head ... ok
test restoring_a_flow_round_trips_and_the_counter_keeps_climbing ... ok
test the_per_kind_cap_override_is_honoured_by_capture ... ok
test twenty_five_saves_leave_exactly_twenty_ring_rows_newest_first ... ok
test concurrent_saves_of_one_entity_never_over_grow_the_ring ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 8 tests
test the_builtin_role_bundles_carry_the_versions_caps ... ok
test a_viewer_tier_caller_sees_versions_list_in_its_own_tool_catalog ... ok
test config_set_is_admin_only_while_config_get_is_not ... ok
test undo_after_a_dashboard_restore_returns_the_pre_restore_record ... ok
test a_denial_reveals_nothing_about_what_exists ... ok
test get_is_refused_without_its_own_grant ... ok
test rings_are_invisible_across_workspaces_and_a_cross_ws_restore_is_refused ... ok
test restore_is_refused_without_the_kinds_save_cap ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-store            # every suite: 10+6+6+3+1+4+3+3+1+2 passed, 0 failed
$ cargo test -p lb-host --lib       # 376 passed; 0 failed
$ cargo test -p lb-host --test undo_test --test undo_dashboard_test --test undo_autocapture_test \
      --test catalog_mcp_test --test authz_test --no-fail-fast
  7 / 8 / 4 / 4 / 5 passed; 0 failed        # the suites this change could plausibly break
$ cargo test -p lb-role-gateway --lib versions
test result: ok. 3 passed; 0 failed        # the restore-route body parsing (see Dead ends)
$ cargo fmt --all --check                    # clean
$ cargo build --workspace                    # clean
```

**Verified in the product, not only in the suite.** The downstream shell (`NubeIO/rubix-ai#55`) drives
these verbs through a real browser against a real node running this code via a local `[patch]`, on a
dedicated scratch rig (its own store and ports, never the shared dev node):

```
$ E2E_SPEC=e2e/version-history.spec.ts bash scripts/e2e-schema-node.sh
  ✓ a dashboard's ring lists every save newest-first, diffs an old one, and restores it
  ✓ a flow's ring carries the flow's own version counter, and the counters differ per row
  ✓ a deleted rule's history is still reachable, and restoring it puts the rule back
  ✓ a viewer sees the history read-only: no Restore button, and a forced restore is refused
  4 passed (13.7s)
```

That fourth walk is the one worth naming: it mints a REAL second identity, revokes its
system-granted `role:member`, grants the built-in `role:viewer`, and asserts the live session holds
`versions.list` but neither `versions.restore` nor `dashboard.save` — then that the dialog opens
read-only and `POST /versions/dashboard/{id}/{vid}/restore` with that token is a `403`. Negative
controls were run both ways (grant member → the caps guard fails; grant restore+save → the
"no Restore button" assertion fails), so the deny is not a tautology.

**Revert-checked** (a test that passes against reverted code is not a test):

| reverted | test that went red |
|---|---|
| the `versions.restore` arm in `undo_capture/plan.rs` | `undo_after_a_dashboard_restore_returns_the_pre_restore_record` |
| the `versions.restore` arm in `versions/plan.rs::classify` | `restoring_a_dashboard_round_trips_and_appends_a_new_head` |
| `hash_ignore` (before it existed) | `an_identical_save_is_deduped_by_snapshot_hash` |
| the no-escalation pre-check in `restore.rs` | `restore_is_refused_without_the_kinds_save_cap` — **only after the test was strengthened**, see below |

## Debugging

No `debugging/` entry: nothing that shipped broke. Two implementation defects were found and fixed
*within* the session (both by a test), and are recorded in Decisions #4 and Dead ends rather than as
debugging entries, since neither ever existed outside this branch.

## Public / scope updates

The scope's Open questions were already "none — decision-complete"; the thirteen calls above are
additions the implementation forced, recorded here rather than back-ported, since none contradicts
the scope. Promotion to `doc-site/content/public/versions/` is **deliberately held** until the
release: the surface is unreleased (no `node-v*` tag), and `public/` is "what shipped".

## Skill docs

n/a this session — the drivable surface exists but has not been exercised against a released node.
`docs/skills/versions/SKILL.md` is owed by the release session, grounded in a live run (the rule is
that a skill is written from real payloads, and the only live node running this code is a local
`[patch]` build).

## Dead ends / surprises

- **A `400` that beat the capability gate — found only by driving the real HTTP route.** `POST
  .../restore` took its body as `Option<Json<RestoreBody>>`. Every field of that body is optional, so
  "no body" is the ordinary call — but axum's JSON extractor rejects an empty payload with a
  `content-type: application/json` header, in the EXTRACTOR, before the handler and therefore before
  the cap gate inside the verb. Two bugs in one: a valid request got a `400`, and the ordering
  inverted the rule this codebase is careful about elsewhere (`tool_call.rs` documents at length why
  arg validation runs *after* the caps gate — answering "is your request well-formed?" for a caller
  who may not call the verb at all is an oracle). Fixed: the route takes raw `Bytes` and parses
  inside the handler, so absent/empty means "no options" and only a genuinely malformed body is a
  `400`. Three unit tests pin it. **No suite would have caught this** — every host-level test calls
  the verb through `call_tool`, which never sees an HTTP body.

- **A deny test that passed against reverted code.** `restore_is_refused_without_the_kinds_save_cap`
  survived deleting the no-escalation pre-check entirely — because the *nested* `dashboard.save`
  denies too, so the outcome was identical either way. The test was asserting a refusal that would
  happen regardless, i.e. it was not testing what its name claimed. Fixed by asserting the property
  only the pre-check has: a real version id and a fictional one must produce the *identical* error.
  That version does go red when the check is removed. This is exactly the "a test asserting a plan
  never proves it's executed" trap; the revert-check is what caught it.
- **The dedupe silently didn't work** (Decision #4) — and would have shipped looking fine, because
  the ring still filled and trimmed correctly. Only the explicit "save the same thing three times,
  assert one row" test found it.
- **Two SurrealDB shapes** cost a cycle each: `SELECT *` returns a `Thing` id that will not decode
  into `Value`, and an `ORDER BY` idiom must be literally present in the selection. Both are already
  documented in `capped.rs`'s module doc — read it before writing the next capped consumer.
- **Flows have no `edges` field** — topology is `nodes[].needs`. The first draft of the validator
  test poisoned a field that does not exist, so `flows.save` accepted the snapshot and the test
  "passed" the wrong way (it failed on a later assertion, which is the only reason it was noticed).

## Follow-ups

- **Release** (the listed, deliberate remainder): tag `node-v*`, bump the pin in `NubeIO/rubix-ai`'s
  `Cargo.toml`, and drop the `[patch]` in that repo's git-ignored `.cargo/config.toml`
  (WORKFLOW-LB.md §4a). Nothing is committed or tagged by this session.
- `docs/skills/versions/SKILL.md`, from a live run against the released node.
- Promote to `doc-site/content/public/versions/` at release.
- **Restoring another author's dashboard needs `dashboard.save_any`, not `dashboard.save`** — a real
  product edge the live walk surfaced. `dashboard.save` is owner-gated (gate 3) *on top of* its
  capability, while the no-escalation check re-demands only the capability; so a member restoring a
  colleague's board passes both versions gates and is refused by ownership at the nested save. The
  behaviour is correct and not silent (a standard refusal surfaces), but the downstream UI renders a
  Restore button that will `403`. The clean fix is upstream, here: return a per-entity `can_restore`
  on `versions.list` — the node already knows the answer, and it keeps the client free of the
  ownership semantics it deliberately does not model. Deferred rather than rushed: it is a wire
  addition, and the honest `403` is what the downstream scope specifies for v1.
- **Extension-declared kinds** — the plan table is shaped for a manifest-fed row; deferred by the
  scope, unchanged here.
- **`versions.config.set` has no UI** (the downstream scope's v1 non-goal); the verb + route exist.
- **Surface `hash_ignore` on the wire so the diff can de-emphasise save metadata.** The live walk's
  screenshot shows the dashboard diff carrying an `updated_ts` group beside the real `title` change.
  The node already knows that field is save metadata, not content — it is the `hash_ignore` column
  the dedupe uses — but the client cannot know it without per-kind knowledge it deliberately does not
  have (`kind` is opaque end to end). The fix is generic and small: return the kind's `hash_ignore`
  on `versions.list` (e.g. `meta_fields: ["updated_ts"]`) and let the diff sort those groups last.
  Deliberately NOT done in this session: it is a wire change, and the live walk was mid-flight
  proving the current shape. The group renders collapsed by default, so the noise is contained.
- STATUS.md updated ✅.
