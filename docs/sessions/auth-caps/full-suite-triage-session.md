# Session — full-suite triage on `master` (`57684b1`)

- **Date:** 2026-07-14
- **Branch:** `master` (no feature slice in flight; the tree carried the in-progress ingest
  producer-restart fix, folded in as finding 5)
- **Ask:** run the Rust workspace test suite and triage what is red.
- **Outcome:** all 5 unexplained failures resolved (one a real security leak) — **and then 4 of the
  7 "long-known, out of scope" binaries turned out not to be real**: three were one line in the wrong
  Cargo section hiding for weeks (including the devkit scaffold template, so every generated wasm ext
  was born unbuildable), and `agent_routed_test` was simply stale (3/3 green). Genuinely red:
  **3 binaries** (persona catalog/coding, reminder), plus two pre-existing federation stack
  overflows. The list was wrong in both directions at once.

## The authoritative numbers

`cargo test --workspace --no-fail-fast -j 4` on `57684b1` + this session's fixes — **390 test
binaries**, cargo naming **12 failed targets**. Re-running exactly those 12 against the fixed tree on
a **quiet box** (one invocation, no parallel cargo — see `rules_test` below):

| target | sweep | after |
|---|---|---|
| `proof_panel_test` | 17 failed | **23/23 ok** |
| `native_test` | 5 failed | **5/5 ok** |
| `fleet_monitor_test` | 2 failed | **2/2 ok** |
| `ingest_test` | 1 failed | **4/4 ok** |
| `build_test` | 1 failed | **2/2 ok** |
| `devkit_e2e_test` | 1 failed | **2/2 ok** |
| `rules_test` | hung | **22/22 ok** |
| `reminder_test` | 1 failed | 1 failed — genuinely red |
| `agent_persona_catalog_test` | 6 failed | 6 failed — genuinely red |
| `agent_persona_coding_test` | 2 failed | 2 failed — genuinely red |
| `federation_test` / `federation_sqlite_test` | stack overflow | pre-existing, excluded (no federation source touched) |

**12 red binaries → 3.** Exit 101 on the re-run is correct: the three survivors are the real
known-red set. `rules_test` passing 22/22 in that same quiet invocation is what proves its sweep hang
was my own parallel-cargo contention, not a defect.

## What this session was

Not a feature slice. The ask was "run the tests"; the value was in refusing to wave failures through
as known-red. The stale memory list said "7 binaries fail on clean master, everything else green" —
the real number at `57684b1` was **12**. Five failures were unaccounted for, and treating the list as
complete would have buried a live capability leak.

## Environment (not the point, but it shaped the session)

- Disk hit **100% (2.8M free of 916G)**; `rust/target` was 407G. Cargo could not link test binaries
  (`could not parse/generate dep info`). Mid-session the whole `target/` tree was removed by
  something outside this session, freeing 503G; the user then confirmed they had fixed the disk.
- `cargo test --workspace` OOM-killed (exit 137): 28 cores × parallel linking with debuginfo vs 31G
  RAM. **Use `-j 4`/`-j 6`** for workspace test builds on this box.
- Reminder: plain `cargo test` is fail-fast ACROSS binaries — `--no-fail-fast` is what enumerates.

## Findings

### 1. Capability leak — schema validation ran before the cap gate (FIXED)

The headline. `viewer_reach_test` failed `400 ≠ 403` on `dashboard.save`. The obvious reading was
"stale payload, the verb now requires `now`" — and adding `now` **does** turn it green, which is the
trap: it routes around the gate rather than testing it.

Real cause: `host/src/tool_call.rs::dispatch_at_depth` ran the JSON-Schema validator BEFORE
`authorize_tool`. `BadInput` → 400, deny → opaque 403, so any verb declaring an `input_schema` told
an unauthorized caller their args were malformed — a shape oracle contradicting the contract the code
states in two places. Only schema-declaring verbs leaked, which is why exactly one of seven rows
failed. The extension tier was never affected (`call_with_ctx` authorizes first by design).

The bug had **disabled the assertion that would have caught it**. Fix: validate after the gate.
Detail + fail-before/pass-after table:
`debugging/auth-caps/schema-validation-preceded-cap-gate-leaks-400.md`.

### 2. `store.schema` hid real columns (FIXED)

`store_query_test::schema_reports_tables_and_denies_and_isolates` expected `seq`/`payload` on
`series`; schema reported only `["ts"]`. `store_query/schema.rs::table_columns` treated "has any
`DEFINE FIELD`" as "is SCHEMAFULL" and returned the declared fields alone. But `series` is
**schemaless** with one typed field — `ingest/src/schema.rs` defines only `ts` (datetime, for the ts
index); `seq`/`payload` are undeclared yet present and selectable (`SELECT seq FROM series` works,
proven by the sibling `row_cap_enforced` test).

So the schema browser and any agent reading `store.schema` were shown one column of a six-column
table. Fix: UNION declared fields (authoritative types) with one sampled row's keys (typed `any`)
instead of early-returning. 6/6 green, incl. the cap-deny + ws-isolation cases.

### 3. `lb-viz` transforms — Cargo feature unification, not a regression (FIXED)

Two transform tests failed in `--workspace` but passed under `cargo test -p lb-viz` (77/77, matching
what was recorded at P2). `cedar-policy-validator` + `jsonpath_lib_polars_vendor` enable
`serde_json/preserve_order`; a workspace build unifies that in and `serde_json::Map` flips from
sorted `BTreeMap` to insertion-ordered `IndexMap`. The tests asserted alphabetical order with the
comment `// keys alphabetical (serde_json Map)` — an assumption that only ever held in an isolated
build that never ships (the node depends on cedar, so **production is insertion order**).

Fix: assert the real invariants (time field leads; label/value stay paired) and sort before
comparing, so they pass under BOTH resolutions. Verified with `-p lb-viz --lib` and
`-p lb-viz -p lb-host --lib`.

### 4. `lb-prefs` drift tests pointed at a deleted tree (FIXED — guard retired, not re-pointed)

`generated_catalog_test` / `generated_ts_test` asserted byte-identity between the generators and a
checked-in TypeScript twin at `ui/src/lib/prefs/*.generated.ts`. Commit `678503f "deleted the ui"`
removed that tree: stale tests pointing at a deleted target, not drift. Fails in isolation too.

**Decision: delete the two file-identity tests; do not re-point them.** lb is a library now and the
consuming client lives out of tree (`lb-library-posture-retention`), so there is nothing in this
repo to be identical *to*. Re-pointing at a freshly regenerated file would assert only that the
generator matches itself — a test that can never fail, which is worse than no test because it reads
as coverage. The drift guard is real work, but it now belongs to whichever consumer **vendors** the
output; that is where the two copies actually exist and can diverge.

What survives, deliberately: each file also held a **second, still-valid test** with no `ui/`
dependency, so deleting the files wholesale would have silently dropped real coverage —
`builtins_parse_and_share_keys` (en/es key-for-key alignment ⇒ the es→en fallback is total) and
`dimension_and_unit_counts_match_declared_all` (the `ALL` arrays stay in step with the enums — `ALL`
is what the generator *and* every exhaustive test enumerate). Each file now holds just that
invariant, with a header saying why the drift guard is gone so it is not "restored" later.

The generators themselves are kept — they are still how an out-of-tree consumer emits the twin — but
their doc comments promised the drift test and pointed at `../ui/`; both headers were corrected.
Stale instructions are how a deleted test comes back.

### 5. `series.latest` pinned to a pre-restart sample (FIXED — found live, not by the suite)

Not a suite failure: found in live `ems` behaviour, folded in here because it is the same lesson.
A live meter read stale for hours after its sidecar restarted. `seq` is monotonic per
`(series, producer)` **only**, but `ingest.write` flattened every stream of one principal onto the
bare `principal.sub()` and `latest` ordered the whole series by `seq DESC` — so a producer whose
in-memory seq restarted at 0 re-entered below its own high-water mark and lost to its pre-restart
`seq=807` forever.

Fix: **root** the producer rather than flatten it (`{principal}/{declared}`, prefix host-stamped and
un-spoofable, untrusted leaf sanitised) + order by the axis the streams share (`ts DESC, seq DESC`).
Full entry: `debugging/ingest/latest-pinned-to-pre-restart-sample.md`.

It belongs in this session's record because **no test could have caught it**: the `sample()` helper
tied `ts: seq`, so the two axes could never disagree — the same shape as finding 1, where the bug had
disabled its own assertion.

The source fix landed mid-session as `d08adc7` (`crates/host/src/ingest/write.rs`,
`crates/ingest/src/{latest,sample}.rs` + the `lb-ingest` crate's own tests). It did **not** carry
`crates/host/tests/ingest_test.rs`, the host-layer test the contract change breaks — that test
hardcoded `producer: "ignored"` with the comment "value here is ignored", true only while the stamp
discarded the wire value outright, and it failed
`left: "client:pi-7/ignored" != right: "client:pi-7"`. Closed here: the fixture now declares no
sub-namespace (`producer: ""`, the back-compat default), and a new test
`a_declared_producer_is_rooted_under_the_principal_and_cannot_forge_another` covers the half the old
fixture made unexpressible — a hostile declaration (`../client:other/epoch-9`) must stay exactly one
level beneath its own root and can never name another principal's namespace. Rooting the id is what
turned forgery into a real question, so it needed a real test. `lb-ingest` 7/7 + host `ingest_test`
4/4.

### 6. Three of the seven "long-known" reds were ONE line, hiding for weeks (FIXED)

The list said seven binaries fail on clean master and are out of scope. Reading their failures rather
than accepting them, **three were never code failures at all**: `proof_panel_test` (17/23),
`build_test` (1/2) and `devkit_e2e_test` (1/2) all died at the same `build.rs:12:10` —
`DEP_LB_SDK_WIT … NotPresent`.

`lb-sdk` was declared under **`[build-dependencies]`** ("a build-time dep only" — intuitive, exactly
backwards). Cargo passes `DEP_<links>_*` to a build script only for its own package's NORMAL deps, so
the var was unset and `wit_bindgen::generate!` never ran. `extensions/hello` had the **byte-identical
`build.rs`**, the same SDK tag, and built fine — its manifest comment states the rule verbatim.

The second copy is the serious one: the same line sat in
`crates/devkit/templates/wasm/Cargo.toml.tmpl`, **the scaffold template — so every wasm extension
anyone generated with `devkit scaffold` was born unbuildable**, the devkit's whole promise broken at
step 2, for every user, since the SDK split. `build_test`/`devkit_e2e_test` had been reporting
exactly that all along; being on the accepted list is what stopped anyone reading them.

Fix: move the dep to `[dependencies]` in both copies. **proof_panel 23/23, build_test 2/2,
devkit_e2e 2/2** (the last scaffolds → builds → publishes → calls a fresh extension: the real proof).
Entry: `debugging/extensions/proof-panel-wit-dep-in-wrong-section.md`.

Same pass, same class: `fleet_monitor_test` (0/2) and `native_test` (0/5) were **unbuilt binary
prereqs** wiped with `target/`, each panicking with an explicit `missing … — run: cargo build -p …`.
Built them → 2/2 and 5/5. Not code failures; not known-red; just unread messages.

### 7. `federation_sqlite_test` — pre-existing, and it narrows a known bug

Stack overflow in `federation_end_to_end_sqlite`. Not ours (the diff touches no federation source;
a runtime recursion cannot come from a cap-gate reorder). It matches the documented
`federation_end_to_end_postgres` overflow — but **sqlite needs no docker and no postgres**, which
kills that entry's "masked on docker-less boxes" hypothesis and puts the recursion in the *shared*
plan/pushdown path, with a container-free repro. Folded into the existing entry.

### 8. `agent_routed_test` was never red either

It sat on the known-red list as "1/3 — no in-house model configured". The authoritative
`--workspace` sweep ran it **3/3 green**; it does not appear in cargo's failed-target list at all.
Stale entry, nothing to fix.

That makes **4 of the fabled 7 not real**: three were one Cargo line (finding 6), this one was
simply out of date.

## Left open

- `agent_persona_catalog_test` 6/8, `agent_persona_coding_test` 2/10, `reminder_test` 1/4 — the
  genuinely-red remainder (three, not seven), unchanged and out of scope here.
- The two federation e2e stack overflows (postgres + sqlite) — pre-existing; owner is federation.
- `rules_test` hangs under heavy box load, green when quiet — a harness fragility (27 concurrent
  `worker_threads = 1` runtimes starve into a real deadlock), newly written up at
  `debugging/rules/rules-test-hangs-under-load.md`. I caused it here by running parallel cargo
  invocations against the sweep; it is not a regression.

## Lessons

0. **Three of the five findings were bugs that had made themselves untestable.** The cap leak
   disabled the `dashboard.save` deny row the moment the verb gained a schema; the ingest restart bug
   was unexpressible because the `sample()` helper tied `ts: seq`; the viz transforms asserted an
   order that only ever held in a build that never ships. This is the pattern worth carrying: a
   defect that silences its own assertion leaves a suite that is green *and* wrong, and no amount of
   re-running it helps. **Ask what a passing test would still catch, not just whether it passes.**
1. **A known-red list is a starting hypothesis, never a verdict — and it lies in BOTH directions.**
   Its "everything else green" nearly buried a live capability leak (7 vs 12 the real count). Then
   three of the seven it *did* list as code failures were one line in the wrong Cargo section, fixed
   in minutes once someone read the panic instead of the label. A known-red list is precisely where
   a cheap fix hides indefinitely, because "expected to fail" stops anyone looking. **Read the
   failure, not the list.** A test that dies on a missing artifact or inside a build script is not
   testing the code and is evidence of nothing.
2. **A deny test failing with a non-403 error status has stopped testing the deny.** Never top up
   the payload to make it pass — check whether the request reached the gate.
3. **"Green in isolation, red in the workspace" is a feature-unification tell**, not flakiness. The
   isolated build can be the unrepresentative one.
4. **The tempting fix and the correct fix pointed in opposite directions** in two of three cases
   (payload top-up; and "the test expects the wrong order"). Both would have shipped green over a
   real defect.
5. **A test whose subject left the repo should be retired, not re-aimed.** Re-pointing the drift
   guard at a regenerated file would have produced a test that cannot fail — coverage-shaped, but
   inert. Deleting a stale test is honest; keeping a tautology is not.
6. **Read what a stale test file actually contains before deleting it.** Both prefs files mixed one
   dead test with one live invariant; `rm` on the filename would have taken the invariant too.
7. **A bug in a scaffold template is not one bug — it is one per user, on their first minute.** The
   `build_test`/`devkit_e2e_test` reds were the alarm for exactly that, firing correctly since the
   SDK split. A red template test reports on everyone's future project, not on itself; it deserves
   the *highest* triage priority, and it got the lowest because it was on a list.
