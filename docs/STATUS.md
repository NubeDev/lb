# STATUS — where the project is right now

The single **"where are we"** dashboard. One screen, always current. Read this first at the
start of any session; update it at the end of any session that changed state.

> This is a **living snapshot**, not a log. It is overwritten in place — it always describes
> *now*, never history. The history lives elsewhere, on purpose:
> - **per-feature narrative** → `sessions/<topic>/…` (the messy middle of each session)
> - **bug history** → `debugging/README.md` (append-only symptom → fix memory)
> - **what shipped** → `public/` (the trimmed source of truth)
>
> So there is **no `LOG.md`** — those three already are the log, each at the right altitude.
> STATUS.md just points at them and says "this is the front line."

---

> ⚠️ **Repo posture (2026-07-11): lb is a LIBRARY now.** Consumed by product hosts via the `lb-node`
> embed seam; extension SDKs + product extensions + product UI shell have moved out-of-tree.
> Authoritative posture + retention window: [`../MIGRATION.md`](../MIGRATION.md).
>
> 🚫 **`ui/` IS DELETED (2026-07-15, commit `678503f`) — never recreate it.** The React/Tauri shell
> is gone for good; product UI lives out-of-tree (`rubix-ai`), reusable frontend in `packages/*`,
> app-side TS in `app/`. AI sessions have repeatedly re-added code and `mkdir`'d dirs under `ui/`:
> that is **always wrong**. Everything below this banner mentioning `ui/src/...` is **history — a
> record of where code lived when that session ran**, not a live path and not permission to restore
> the tree. Rule + rationale: [`../CLAUDE.md`](../CLAUDE.md) § "Never recreate `ui/`".
> The in-tree `rust/extensions/*` are still **retained temporarily** (reference/fallback).

---

## Current stage

**In flight 2026-07-17 — [#75](https://github.com/NubeDev/lb/issues/75) + [#76](https://github.com/NubeDev/lb/issues/76):
an lb-hosted browser shell can log in.** (branch `scope/browser-shell-hosting`, code + tests green,
not yet merged)
[#75 scope](scope/frontend/spa-static-hosting-scope.md) ·
[#76 scope](scope/frontend/browser-session-scope.md) ·
[session](sessions/frontend/browser-shell-hosting-session.md) ·
[public](../doc-site/content/public/frontend/browser-shell-hosting.md)

A host that hands lb its shell via `static_root` could serve the whole UI and **not log in** — two
independent bugs, both invisible on a dev box because Vite fills in the missing halves, both found on
ems's armv7 CM4 target ([`NubeIO/ems#8`](https://github.com/NubeIO/ems/issues/8)). **#75:** `GET /login`
405'd, because lb registers `POST /login` and axum's `fallback_service` fires only when *no* route
matched — so the login *page* never rendered. Now the gateway content-negotiates on the
method-mismatch path (GET/HEAD + `Accept` explicitly prefers `text/html` → `index.html`, else the 405
with `Allow` intact); `Accept: */*` deliberately does **not** count, so `curl -X GET /mcp/call` still
405s. **#76:** nothing terminated `/api/*` in production — the seam existed only as dev-only Vite
middleware in ems *and* cc-app — so the *credential* never posted. Now an opt-in
`BootConfig::browser_session` mounts a store-backed cookie session: `/api/auth/{login,select,switch,
logout,session}` + a mediated `ANY /api/{*rest}` that resolves the sid to its JWT and dispatches
**internally** into the same router a CLI hits. The token never reaches the browser; `/api/*` is
excluded from permissive CORS and gated on `Sec-Fetch-Site`/`Origin`.

Both are **off unless configured** — `static_root`/`browser_session` unset ⇒ today's router
byte-for-byte, so rubixd and rubix-ai are untouched (their "no cookies" scopes are annotated, not
reversed). 26 integration + 18 unit tests green, including the scope's own merge gate (a cross-origin
POST with a valid cookie is rejected) and the mandatory capability-deny + workspace-isolation pair
driven through `/api/*`.

**Next:** one tag carrying these + the pending `seed_email` + `/auth/*` work (master is 21 commits
ahead of `node-v0.4.6`); ems#8 step 2 follows it. ems must **not** unblock with a `[patch]` — that
regresses the tags-only property the ARM/Pi build rests on.

---

**Shipped 2026-07-16 — [#72](https://github.com/NubeDev/lb/issues/72): the gateway has a `/health` route.**
[scope](scope/deploy/health-route-scope.md) ·
[session](sessions/deploy/health-route-session.md)

The gateway served no health route, so nothing an LB/orchestrator could probe answered "is this node
up?" without a session token — `fly-deploy` and `containerize` both had to probe `GET /` instead and
recorded it as a known concession, and `rubixd`'s rollback-health gate could only fall back to
`tcp:<port>` against a product host (a socket accepting is not a node working). Verified against a
live `rubix-ai` node: `/health`, `/healthz`, `/api/health` all 404. **Now:** one unauthenticated
`GET /health` on the gateway port, implementing the fleet contract `containerize-scope.md` already
ratified — `200 {"status":"ok","version":…,"detail":{"store":"ok","gateway":"ok"}}` serving /
`503 {"status":"degraded",…}` alive-but-not-serving; `/health`, **never `/healthz`**; one route, no
`/livez`/`/readyz`. Registered first, outside the auth wall (an LB has no bearer — same posture as
`/login`); always on when `GatewayMode::Addr`; no `BootConfig` field.

**The two load-bearing calls.** (1) **No store ping in the handler** — the contract's sharpest rule
is "a health check that can hang is a health check that lies", and `store.query` would both hang-risk
and contend for the global session mutex. The handler reads two `AtomicBool`s, nothing else. (2)
**The 503 path is a real seam, not faked** — `HealthGate` (one atomic per subsystem the contract
names: `store`, `gateway`) defaults both to serving, which is the *honest* answer at this layer (the
store handle is alive once `Node::boot` opened it; `system-map-scope.md` already says "the handle
exists" is not real liveness, and this route does not pretend otherwise). `set_store`/`set_gateway`
are the attachment point a FUTURE in-process monitor (store-down detection, drain-on-shutdown) flips;
no caller flips them today, and the scope doc says so plainly rather than dressing always-200 as
detection. **Version** = `env!("CARGO_PKG_VERSION")` of the gateway crate (the lb-gateway build an
embedder shipped — what an LB pins a matcher on).

**Tested (rule 9, real gateway/node):** `health_route_test` **6/6** — 200 shape + version + detail on
a bare GET with no Authorization header; leaks-nothing body (exactly `{status,version,detail}` +
`detail` exactly `{store,gateway}`, values `"ok"|"degraded"` only); a garbage `Bearer` header returns
the same 200 (never reaches the auth wall); `/healthz`/`/livez`/`/readyz`/`/startupz`/`/api/health`
all 404; `set_store(false)`/`set_gateway(false)` ⇒ 503 with the right subsystem degraded; recovery
back to 200 after a clear. The mandatory cap-deny/ws-isolation categories do not apply (unauthenticated
+ workspace-agnostic by design). **No regressions:** `gateway_test`/`gateway_routes_test`/
`login_hardening_test` green after the new `Gateway.health` field; `cargo build`/`fmt`/`clippy --lib`
clean on the new files. **Follow-ups (named, not done):** flip `fly.toml`'s check to `GET /health`
matcher 200; drive `HealthGate` when a real monitor exists; `rubix-ai`/`ems-node` bundle specs flip
`tcp:` → `http:` in those repos once they bump the lb tag.

---

**Shipped 2026-07-15 — [#67](https://github.com/NubeDev/lb/issues/67): the commit log stays bounded on a RUNNING node (online compaction + observability).**
[scope](scope/store/online-compaction-scope.md) ·
[session](sessions/store/online-compaction-session.md) ·
[public](../doc-site/content/public/store/store.md) ·
[skill](skills/store-compact/SKILL.md)

The spike-first order paid off twice. (1) It **disqualified live-handle compaction by
demonstrated data loss** (surrealkv 0.9.3 has NO directory lock — a second handle opens fine,
compacts "fine", and every live-handle write after it is lost). (2) It **caught a P0 in the
already-shipped boot compaction**: at the pinned engine, a compaction merge is applied at the
next open with the append-log already open, so that session appends into unlinked inodes —
**every boot from the third onward silently destroyed all writes made since the previous
boot** ([debugging](debugging/store/compaction-merge-eats-next-sessions-writes.md), fixed via
the merge-completion rule in `compact_log`, regression-tested fail-before/pass-after).

Shipped shape 2 (mutex-held handle swap): the session mutex now CARRIES the `Surreal<Db>`
handle, `lb_store::compact` quiesces writes → swaps → fd-poll release (74–240 ms measured;
timeout = skip, never compact under a live handle) → shared `compact_log` → reopen → swap
back. Surface: `store.status` (read, `store:status:read`; threshold advisory at 256 MiB, same
posture as the over-cap warnings) + `store.compact` (**a job**, admin `store:compact:run` — a
new `run` cap action so `store:*:write` can't pause the node) + the drain/advisory reactor
(30 s tick; drains operator-enqueued jobs only — no compaction-on-a-tick). Isolation suites
incl. `concurrent_ns_test` pass UNMODIFIED; concurrent writers land across the swap (16-way);
crash artifacts (`.merge`/`.tmp.merge`) reopen clean; boot dividend asserted on a compacted
vs uncompacted copy.

**Also shipped 2026-07-15: flows read-back hardening — deployed flows now return values consistently
past any workspace size.** The "values sometimes missing after deploy" report traced to ten flows
call sites reading a shared ws table through ONE 200-row `lb_store::scan` page and filtering in
code (`node_state` ×2, `scan_run_slots` — the drive frontier AND finalisation — both retained-input
readers, `runs.*` ×2, `flows_list_internal` (feeds the reactors), the `Skip`-concurrency guard, the
orphan sweep): green under 200 rows/table, silent row loss past it — values null, runs stuck
pending, overlapping runs. Fixed via `host/flows/scan_all.rs` (drain every page; prefix early-exit
deliberately unsound — the scan cursor is the `⟨⟩`-bracketed `<string>id`). Same pass: an `Err`
firing now merges `lastError` onto `flow_node_state` (was Ok-only — a broken node showed its stale
last-good value as current forever), lifted as `error` on each `flows.node_state` entry, cleared by
the next Ok. Multi-input/multi-output audit confirmed working by design (any-join fan-in, fan-out
to all dependents; barrier + carry-forward edges documented in the session doc). **Tested (real
node, rule 9):** new `flows_scan_paging_test` (4: 240 real rows seeded past the page boundary for
node_state + step slots; err-marking keeps the good value; next-Ok clears) + `lb-flows` unit **96**
+ all 16 host flows suites green (152 incl. `flows_run_test` 49, `flows_triggers_test` 17,
`flows_runtime_control_test` 12), verified in a clean worktree (main tree mid-edit by the
concurrent rules session). Debugging:
[`flows/single-scan-page-drops-rows-past-200.md`](debugging/flows/single-scan-page-drops-rows-past-200.md)
(+ the test-infra
[`three-run-e2e-sequence overflow`](debugging/flows/three-run-e2e-sequence-overflows-default-test-stack.md)).
Session [`flows-readback-hardening-session.md`](sessions/flows/flows-readback-hardening-session.md);
public [`public/flows/flows.md`](../doc-site/content/public/flows/flows.md) (new "Value read-back"
section). **Named follow-up — [#69](https://github.com/NubeDev/lb/issues/69) (resolved 2026-07-15):** the same
single-page pattern lived outside flows (rules, dashboard, panel, report, nav, brand,
render_templates, insight) — swept onto one shared `lb_store::scan_all` drain; see the roster
block below.

**Also shipped 2026-07-15: [#69](https://github.com/NubeDev/lb/issues/69) — roster lists no longer
silently drop rows past 200 (the single-page sweep outside flows).** The flows read-back fix found
the same single-`lb_store::scan`-page pattern in every host roster read: `rules.list` +
`dashboard`/`panel`/`report`/`nav`/`brand`/`render_templates` `scan_*` each did ONE 200-row page
and filtered in code; `insight/notify::load_subs` was the one listed site already draining. Green
under 200 rows/table, partial past it — a workspace with >200 of any config listed only the first
200, no error. The `MAX_DASHBOARDS`/`MAX_PANELS`/… "caps" were literally `= MAX_SCAN_LIMIT` (200)
and `scan` clamps every request to 200 server-side — a cap above 200 is silently 200. Fix:
promoted the cursor loop to ONE canonical cross-crate seam — `lb_store::scan_all`
(`store/src/scan_all.rs`) — moved all seven host roster sites + `insight/notify` onto it, and made
`host/flows/scan_all.rs` a thin re-export so flows and rosters share the one implementation;
removed the misleading `MAX_*` aliases (no external consumers; none a genuine product cap — every
caller treats the result as the full set). Full-drain-then-filter with NO silent backstop (a
partial return just relocates the bug to a larger N); the scan cursor is the `⟨⟩`-bracketed
`<string>id` whose ordering disagrees with the display id, so a prefix early-exit is unsound.
`insights/table_scan.rs` keeps its own bounded `MAX_ROWS` variant by design (out of scope).
**Tested:** new `roster_scan_paging_test` (3 — the canonical drain at 250 rows; `dashboard.list`
strict-decode + visibility filter past 240 tombstoned fillers; `rules.list` loose-decode/authz past
240 junk fillers) + all affected suites green (dashboard 12, flows_scan_paging 4, insights 22, nav
30, panel 10, render_templates 6, report 9, rules 22). Debugging:
[`store/single-scan-page-drops-rows-past-200-non-flows.md`](debugging/store/single-scan-page-drops-rows-past-200-non-flows.md);
session [`roster-scan-paging-session.md`](sessions/store/roster-scan-paging-session.md).

**Also shipped 2026-07-15: rules 10x — long-running job-backed runs (pause/resume) + the data stdlib.**
[scope](scope/rules/long-running-rules-scope.md) ·
[data-stdlib scope](scope/rules/data-stdlib-scope.md) ·
[session](sessions/rules/rules-10x-longrun-datastdlib-session.md) ·
[public](../doc-site/content/public/rules/rules.md) ·
[skill](skills/rules/SKILL.md)

A rule body was synchronous-only (10s / 5M-op governors) and un-pausable — pause/resume existed for
flows and agent jobs but not for a rule. Six gated verbs now cover the background form (`rules.run_async`,
`rules.runs.get`/`.list`/`.suspend`/`.resume`/`.cancel`; one cap each, on the member built-in role +
the system catalog). A run is a durable `lb-jobs` job (kind `rule-run`) driven by a detached worker on
its own governor profile (10min / 500M ops / 64 AI calls / 200k tokens / 256 writes).

**Load-bearing decision: resume = REPLAY over checkpoints, never a VM snapshot** (snapshotting a live
rhai VM is dishonest — it was rejected, not deferred). The body re-runs from the top; `job.step(key, ||…)`
values persisted in the job transcript short-circuit, and messaging writes replay onto their original
deterministic ids (pinned `ts` + write ordinal) and upsert ⇒ **exactly-once**. Pause/cancel are
cooperative via a shared `RunControl` (AtomicU8) the cage's `on_progress` governor observes — it bites
within one bytecode op with no author cooperation; cancel outranks pause. **No auto-resume of orphans**
(needs a stored principal — deliberately refused): orphans show `live:false` and resume under the
RESUMER's caps. The in-cage `job` handle is durable when job-backed and ephemeral in sync `rules.run`,
so one body works in both modes.

Same slice, the **data stdlib** went from scope-only to real (~180 fns, all in the `rules.help` catalog):
`time` (39) + `duration` (8) — on the injected logical clock, with rhai's `timestamp()` **disabled** as the
determinism contract; `json` (24); `stats` + `window` (52, incl. `sample`/`shuffle` requiring a seed);
`mathx` (12); `job` (8); and `frame` (~60) — polars in the cage (`select/filter/group_agg/join/pivot/rolling/
f.sql("… FROM self")`). Frame caps (`max_frame_rows` 200k / `max_frame_cells` 2M) are enforced on construct
**and** on join/vstack/pivot outputs, because the wall-clock governor cannot interrupt a native polars call.

**Tested (rule 9, real store):** `lb-rules` **80 lib + 89 integration** (frames on) · `lb-frame` **53/9 files**
(incl. `sql_security_test`) · `lb-host` **54** (`rules_longrun_test` 9 — incl. the mandatory cap-deny per verb,
read≠control, and ws-isolation — `rules_test` 22, `rules_ai_wiring_test` 8, `rules_workflow_convergence_test`
14, `rules_buildings_examples_test` 1). Headline proofs: `suspend_mid_run_then_resume_finishes_without_respending_steps`
(pause → durable checkpoints → resume → memoized step does NOT re-run → replayed `channel.post` upserts to ONE
message) and `suspended_run_resumes_after_a_restart` (a fresh `Node` over the same disc store). Debugging:
[`rules/pause-token-lost-in-error-display.md`](debugging/rules/pause-token-lost-in-error-display.md) (rhai's
`ErrorTerminated` Display OMITS the abort token — match the variant + downcast, never the message text) and
[`jobs/order-by-idiom-must-be-selected.md`](debugging/jobs/order-by-idiom-must-be-selected.md) (SurrealDB
rejects `ORDER BY data.ts` unless the idiom is projected).

**Pinned:** polars `=0.54.4` is a **SECURITY pin**, not a stability pin — a minor bump can widen the SQL
namespace with I/O fns. `f.sql` registers ONLY `self`; never register a table provider or a polars scan.

**Deliberate non-goals (not gaps):** no `rules.runs.watch` SSE (poll `runs.get`); no UI for background runs
(verbs only); the generic `job.*` family ([job-control-scope](scope/jobs/job-control-scope.md)) should route
kind `rule-run` to these same owner hooks when it lands.

**In flight — [#65](https://github.com/NubeDev/lb/issues/65): a series grows until the disc is full.**
**Code complete on a branch, TESTING INCOMPLETE — not merged, not shipped.**
[scope](scope/ingest/series-sample-cap-scope.md) ·
[session](sessions/ingest/series-sample-cap-session.md) ← **read this before picking it up; it lists
exactly what is unverified.**

Nothing bounds the **committed** series plane on any axis. Retention (#58) shipped a *time* horizon,
but (1) there is no **count** bound — time doesn't bound bytes, **rate** does, and rate is the
producer's choice; (2) **`run_gc` has no driver** — called only by tests and the on-demand verb, so
shipped retention evicts *nothing* at boot (the same missing-driver class as the drain bug below);
and (3) the default is **keep-forever**. Any one alone fills a disc. Measured **~700 bytes/sample**:
50 series @ 1/sec ≈ 3GB/day ≈ a 64GB disc in ~3 weeks.

**Built (release 1):** `max_samples` FIFO cap on `Policy` (serde-default `0` = unbounded) +
`cap.rs` + a cap step in `run_gc` reporting `capped_raw` + `spawn_retention_reactors` wired into node
boot at 300s + longest-prefix-wins (a latent GC bug: a series matching `fleet.` and `fleet.eu.` was
processed twice). `DEFAULT_MAX_SAMPLES = 100_000` is **advisory only** — warns, evicts nothing
without an explicit policy. **Release 2 flips the default, and is part of this slice's definition of
done** (forgetting it is how gap #3 was born).

**Verified:** `lb-ingest` 16/16, `lb-host --test series_cap_reactor_test` 4/4, and 3 revert-checks
(a `seq`-ordered cap, a reactor that never calls `run_gc`, and dropped prefix precedence each fail
their test as they must).
**NOT verified — required before merge:** the full workspace sweep never completed (killed by
harness timeouts, never `--workspace`); **the live-node run was never done**; disc-growth plateau
unmeasured; the 300s cadence is a guess, not a measurement (the scope says measure it).
**Known hole:** deleting the boot wiring from `node/src/reactors.rs` breaks **no test** — the one
line that makes the feature real on a node is as untested as it was for the drain bug and for
retention itself. This bug class is repeating at the meta level; the live run is the only proof.

**Load-bearing:** eviction orders by **`ts`, never `seq`** (`seq` is per-`(series,producer)` —
ordering by it is exactly what caused #63); `seq` is a tiebreak within an equal `ts` only.

**Shipped 2026-07-15 — `ingest.write` no longer pays for the workspace backlog.** A producer pushing
ONE sample to a backlogged workspace blocked for tens of seconds and never confirmed: measured live
at `node-v0.4.5`, one sample against a 4,671-row backlog took **18,569ms**; the same call at backlog
0 took 21ms. `ingest.write` called `drain_workspace`, which loops until staging is EMPTY — so the
caller committed every *other* producer's rows. Self-sustaining (a timed-out client abandoned only
the wait; the backlog stayed and the next push blocked again) and self-concealing (the first write
heals the backlog, so a second probe reads ~20ms and looks fine). Root cause: **the commit worker the
ingest scope has always named never had a driver** — `drain.rs` said so outright — so every caller
became the worker; the outbox, the pattern ingest mirrors, has had `relay_reactor.rs` all along.
Fixed by bounding each caller's drain to its own batch (`own_batches`, applied at all four call
sites — the MCP verb, `POST /ingest`, webhook accept, and `federation/mirror.rs`, which was draining
the whole backlog *per mirrored row*) and wiring the missing `spawn_ingest_reactors` into node boot.
**Measured 900.7ms → 66.0ms** at the reported backlog on a real on-disk store. The blamed `ORDER BY`
superlinearity was **measured and disproven** before implementing — no index added, staging stays
index-free as designed. Revert-checked on both halves; 8/8 stable.
[scope](scope/ingest/drain-backpressure-scope.md) ·
[session](sessions/ingest/drain-backpressure-session.md) ·
[debug](debugging/ingest/write-drains-whole-workspace-backlog.md).
**Spun out:** [`scope/store/session-concurrency-scope.md`](scope/store/session-concurrency-scope.md) —
the global session mutex serializes every query node-wide (reproduced: 18 concurrent writers, each
its own workspace, = 7.0ms = 18 × 0.4ms, zero parallelism). Real and the next structural ceiling, but
**not** this bug and deliberate (it holds the workspace wall). Tracking only — spike before coding.

**Previously shipped 2026-07-14 — native extensions now survive a node restart (issue #64).** A published
`tier="native"` extension never came back after a restart: `reconcile` planned the start, `load_enabled`
skipped it ("the node Launcher's job"), and **no node implemented that path** — the plan's native
actions were computed and dropped, silently. Fixed by mirroring the wasm half: `ext/boot_spawn.rs::spawn_enabled`
(+ `ext/install_dir.rs`, shared with publish so the `(ws,ext)`→dir rule cannot drift), called from the
node's boot *after* the gateway block (the token-key ordering the role mounts already document). No new
persistence or trust — the artifact cache already held exactly what `install_native` takes; the grant
comes from the durable `Install.granted`, so a restart can't re-approve what an admin narrowed. Open
question resolved as **log-and-continue** (a hard-fail makes one bad extension an unbootable node, and
recovery runs *through* that node) — the silence, not the continuing, was the cost, so an
enabled-but-not-brought-up extension now names itself and its reason every boot. Regressions:
`ext_boot_spawn_test.rs`, 4 tests over a real supervised OS child on a real store that outlives the
node; **revert-checked — all 4 fail with the bug's own empty-boot-log signature**. Verified live: publish
→ restart the node only → the sidecar is back with no republish. Session
[`native-boot-respawn`](sessions/extensions/native-boot-respawn-session.md); debugging
[`native-ext-never-respawns-at-boot`](debugging/extensions/native-ext-never-respawns-at-boot.md).
**Next up:** no start/restart endpoint exists (`/extensions/<ext>/enable` returns 204 but does not spawn),
and boot bring-up covers only `cfg.workspace` — both tiers, pre-existing.

**Test baseline (2026-07-14, full-suite triage; uncommitted working tree): the Rust suite is down to
3 genuinely-red binaries, from 12.** A `cargo test --workspace --no-fail-fast -j 4` sweep (390 test
binaries) plus triage of every red. What the "known-red list" actually was:

- **5 unexplained failures — all real bugs, all fixed.** A **capability leak** (the `/mcp/call`
  schema validator ran *before* the cap gate → schema-declaring verbs answered denied callers with
  `400` instead of an opaque `403`), `store.schema` hiding 5 of 6 `series` columns, two `lb-viz`
  transform tests asserting an order that only holds in a build that never ships, and the ingest
  `series.latest` producer-restart bug (a live meter read stale for hours).
- **4 of the 7 "long-known, out of scope" reds were not real.** Three (`proof_panel_test`,
  `build_test`, `devkit_e2e_test`) were **one line in the wrong Cargo section** — `lb-sdk` under
  `[build-dependencies]`, so `DEP_LB_SDK_WIT` was never set. The second copy sat in
  **`crates/devkit/templates/wasm/Cargo.toml.tmpl`**, meaning *every wasm extension anyone scaffolded
  was born unbuildable* since the SDK split. `agent_routed_test` was simply stale (3/3 green).
- **Genuinely red (3):** `agent_persona_catalog_test` 6/8, `agent_persona_coding_test` 2/10,
  `reminder_test` 1/4. Plus two **pre-existing federation e2e stack overflows** (postgres *and* —
  newly found — sqlite, which proves the recursion is in the shared plan path, not a driver, and
  gives a container-free repro).
- **Not failures at all:** `fleet_monitor_test`/`native_test` needed `cargo build -p fleet-monitor -p
  echo-sidecar` (binaries wiped with `target/`); `rules_test` hangs *only* under heavy box load
  (27 concurrent `worker_threads = 1` runtimes starve into a real deadlock — green when quiet).

**The lesson, since it cost weeks:** a known-red list lies in *both* directions — its
"everything else green" nearly buried a live capability leak, and its own entries hid a one-line fix.
**Read the failure, not the list**; a test dying on a missing artifact or in a build script is not
testing the code at all. Full narrative:
[`sessions/auth-caps/full-suite-triage-session.md`](sessions/auth-caps/full-suite-triage-session.md);
entries in [`debugging/README.md`](debugging/README.md).

**Just shipped (2026-07-14): viz Grafana-parity Phase 4 — JSON import/export (the interop edge)
(`docs/scope/frontend/dashboard/viz/import-export-scope.md`; uncommitted working tree).** The user's
literal ask — *"export a dashboard from Grafana as JSON and import here, and back"* — as two host
verbs + one bidirectional mapper consuming the P3 `grafana-map` pin. New module
`crates/host/src/dashboard/grafana/` (one responsibility per file): `view_alias.rs` (panel.type↔view
id both ways, legacy aliases), `to_cell.rs` (`grafana→cell`: gridPos/type→view/targets→sources/
fieldConfig/transforms/options, unknown→passthrough), `to_grafana.rs` (inverse; passthrough first,
**mapped fields overlay it** — the "fills only gaps" rule), `datasources.rs` (collect refs + apply
remap), `import.rs`/`export.rs` (the verbs + descriptors), `mod.rs` (report types + the `&Node` MCP
bridge). One additive model field `Cell._grafana` (bounded passthrough, skip-if-null, byte-stable)
+ `MAX_GRAFANA_PASSTHROUGH=8KB` rejected-not-stored in `bounds.rs`. **`dashboard.import
{json,mappings?,id?,now}`** = 2-phase (preview w/o mappings → `{report}` no write; commit → UPSERT
via `dashboard_save_meta`), needs BOTH `mcp:dashboard.import:call` (member) AND
`mcp:dashboard.save:call`; **`dashboard.export {id}`** = a read (`mcp:dashboard.export:call`, viewer)
+ the three-gate get. **Tenancy (the hard wall):** workspace from the TOKEN never the JSON; every
datasource `mappedTo` verified against the caller's workspace-walled `datasource.list` — a ws-B
source is invisible in ws-A → import refused `403`. **Honest degradation:** unsupported panel →
`json` placeholder + `options.unsupportedType` + full panel in `_grafana` (re-exports its ORIGINAL
type); unmapped datasource / unsupported variable preserved + reported; nothing faked. Wired:
dispatch (`tool_call.rs`, `&Node` branch before the store-only one), catalog (descriptor + host-tool
rows), caps (`builtin_roles.rs`), gateway `POST /dashboards/import` + `GET /dashboards/{id}/export`
(+ the generic `/mcp/call`). Rule-10 clean: branches on Grafana panel/datasource VOCABULARY only,
never an ext id/role; JSON is interchange, never stored raw. Tests: **48 new green** — host mapper
12 + bounds 3 + gateway integration 4 (real gateway/store/seeded datasource; Grafana JSON=fixture:
preview→commit→export round-trip, **ws-isolation wall**, **caps-deny**, v2 rejected) + `grafana-map`
29 unchanged; **no regressions** (dashboard 34, authz 7, credentials 3, dashboard_routes 6);
`cargo build --workspace`/fmt/clippy clean. Follow-ups (named): datasource `tool`-fill is the binding
step's job (matches native v3 targets), bulk import = a future job, the import UI is downstream
rubix-ai, `__elements`→`panelRef` mapping deferred. Docs:
[session](sessions/viz/grafana-parity-import-export-p4-session.md). **This closes the viz
import/export arc (P1→P4).**

**Previously shipped (2026-07-14): viz Grafana-parity backend P3 — the import pin as code
(`docs/scope/viz/grafana-parity-backend-scope.md`, P3; uncommitted new crate).** New dep-light
crate `rust/crates/grafana-map` (deps `serde_json` + `thiserror` only — **no host/store/bus dep**,
the lb-viz posture), consumed identically by the standalone converter (git dep) and the future
`dashboard.import` verb. Public `pin(root, input_values) -> PinReport` runs **detect → migrate →
resolve**, one responsibility per file: `detect.rs` (v1/v2/snapshot discriminator — accept v1,
reject v2 [`dashboard.grafana.app/*` apiVersion or `elements`+`layout`] and snapshots with a
`classic`-pointing error), `migrate/` (the ported v33 subset: `datasource_ref.rs` string→`{uid}`
ref [structural half only — `type` filled later by the import verb, which owns the federation
datasource list] + `panel_type.rs` `graph`→`timeseries`, `singlestat`→`stat`/`gauge`), `inputs.rs`
(Grafana's `dash_template_evaluator.go` ported: **name-keyed `${NAME}` substitution, no `DS_`/`VAR_`
prefix magic**, `__expr__` auto-fill, unresolved reported+left-verbatim never blanked, all three
envelopes [`__inputs`/`__requires`/`__elements`] stripped — our delta from Grafana's strip-only-
`__inputs`). Order matters: migrate wraps `${DS_*}` into a ref, then resolve substitutes the token
*inside* it → `{"uid":"fed-prom"}`. `MigrateReport.degraded` fires for schemaVersion <21 / missing
(applied blind) — the ported subset is an honest floor, never the silent full `DashboardMigrator`
chain. Rule-10 clean: branches on Grafana panel-type/datasource vocabulary only, never an ext id or
role; JSON is interchange, never stored. Tests: **29/29 green** (`cargo test -p grafana-map`: 27
unit + 2 integration over **real export fixtures** — a pre-v33 Prometheus export with `__inputs`/
string ds/graph+singlestat/row/template-var/`__requires`, and a v2beta1 export rejected); `cargo
clippy -p grafana-map` clean. Both scope open questions decided (crate = `grafana-map`, no host
dep). Not built here: the `dashboard.import`/`export` verbs (the real consumer — pin is ready for
them), `type`-fill on refs, `__elements` library-panel→`panelRef` mapping (mapper's job). Grafana
reference clone absent this session — ported from the scope's pinned descriptions + P1/P2 knowledge,
flagged for re-verify. Docs: [session](sessions/viz/grafana-parity-backend-p3-session.md).

**Previously shipped (2026-07-14): viz Grafana-parity backend P2 — lb-viz tranche 2a + reduce calcs
(`docs/scope/viz/grafana-parity-backend-scope.md`, P2; transforms/reducer already committed in a
parallel snapshot, the one e2e test fix uncommitted).** Six new transforms, one-per-file under
`crates/viz/src/transforms/`, Grafana-verbatim ids/options, pure, each unit-tested:
`renameByRegex`, `filterByRefId`, `convertFieldType`, `extractFields`, `labelsToFields`,
`concatenate`. `reducer.rs` gains the tranche-2 calcs (`diff`, `diffperc` [ratio], `delta`, `step`,
`median`, `variance`/`stdDev` [population ÷n], `distinctCount`, `changeCount`, `allIsZero`,
`allIsNull`) plus the **general `pNN` pattern (1–99, nearest-rank floor)** so any imported
percentile computes rather than degrades. Deps: `regex` (workspace) + `chrono` added to lb-viz,
crate stays pure (parsing only). Honest bounds pinned in the session doc: `extractFields` column
order is alphabetical-per-cell (serde_json Map sorts keys); `convertFieldType` time parsing is
RFC3339 + two bare-UTC shapes only, NOT the dayjs `dateFormat` grammar. Tranche 2b built only as a
fixture demands — none did, so none added (unknown id carried opaque, `viz.query`
skip-with-notice). Tests: `lb-viz` **77/77 green** (transforms + the calc table incl. null/skip
discipline); the P2 e2e pin (`viz_query_test::tranche_2a_pipeline_runs_end_to_end`) runs a
`renameByRegex` + `p90` reduce through the **real** `viz.query`/`store.query` on seeded rows +
proves the unknown-id carry — `viz_query_test` **13/13 green**. The prior session left that pin
red-and-unrun; the bug was the test's SQL (`ORDER BY` on an unselected column, which SurrealDB
rejects → `viz.query` degraded the target to an empty frame), not the transforms — fixed by
ordering on the selected column. Six wider-suite failures are all pre-existing/environmental and
unrelated (persona catalog/coding, cross_node_routing flaky, devkit_e2e wasm, proof_panel,
store_query schema — the last verified failing identically at pre-session `9a4b7041`). P3 (the
import-pin crate) is now shipped — see the block above. Docs:
[session](sessions/viz/grafana-parity-backend-p2-session.md).

**Previously shipped (2026-07-14): viz Grafana-parity backend P1 — model fields + the `queryOptions` hole
(`docs/scope/viz/grafana-parity-backend-scope.md`, P1; uncommitted in the working tree).** The opener
verified-then-fixed a shipped silent-data-loss bug: the editor-parity UI's top-level cell field
`queryOptions {maxDataPoints, minInterval, relativeTime}` was dropped by serde at the `dashboard.save`
tool boundary (closed `Cell` struct, no catch-all) — confirmed on the real save→get path BEFORE fixing,
per the scope's risk note; every shipped save carrying it lost the data (unrecoverable)
([debugging entry](debugging/dashboard/query-options-silently-dropped-on-save.md)). Shipped, all
additive/serde-default/null-tolerant: typed `Cell.queryOptions` (the UI trio +
`timeFrom`/`timeShift`/`hideTimeOverride`, skip-if-empty), `Cell.transparent` + `Cell.links` (opaque),
`Dashboard.timezone` (record-carries-import, prefs-wins-at-render — the scope's open question,
decided), `Variable.description`/`skipUrlSync`/`allowCustomValue`, and `viz.query` now applies the
panel time override at target dispatch (`viz/time_override.rs` — Grafana `applyPanelTimeOverrides`
semantics pinned in the session doc; bounded to numeric epoch `from`/`to` args, never another tool's
vocabulary). Tests: the headline save→get regression pin (fails on pre-fix code), model round-trip +
v1/v2/v3 additive guards, override unit tests, and an end-to-end `viz.query` override test over
really-seeded `series.read`; every touched suite green (three pre-existing/environmental failures in
this checkout are unrelated and verified against a pre-session commit — see the session doc's suite
note; the `agent_persona_catalog_test` persona→skill gap needs an owner). Next: release as a
`node-v*` tag (rubix-ai bumps
its pin), then **P2** (lb-viz tranche 2a transforms + reduce calcs) and **P3** (the import pin crate).
Docs: [session](sessions/viz/grafana-parity-backend-p1-session.md).

**Previously shipped (2026-07-14): series-plane readiness — schema, keyset paging, decimation, retention
(`series-plane-readiness`, issues #55–#58, one PR).** Four slices take the `series` plane from
demo-shaped to production-shaped. **#55 schema/time:** `ts` is a real `datetime` (idempotent
migration for legacy numeric rows), named `(series, seq)` / `(series, ts)` indexes, wire `labels` →
tag edges once per series at commit (closes the 2026-06-27 "series.find finds nothing ingest wrote"
debt), per-workspace **series cardinality cap** (over-cap → dead-letter, never silent), and
wall-clock `{from,to}` bounds on `series.read` (the shape ems already sends). **#56 keyset paging:**
`series.read {limit, cursor, direction}` → `{samples, next_cursor, prev_cursor}`, seeking the unique
`(seq, producer)` composite (tie-safe, O(page)); default/max limit 10 000; the cursor is an opaque
versioned bookmark — re-authorized every page, inert under another ws's token. **#57 decimation:**
`mode:"buckets"` → `{t, min, max, avg, last, count}` (≤ budget, cap 2 000, sparse; spikes survive in
min/max) — computed as a chunked fold over the pager (SurrealDB 2 lacks an ordered `last` aggregate;
documented deviation). **#58 retention:** per-prefix policies + admin-capped
`series.retention.set/list/delete/gc`; GC rolls raw into stored tiers (sum+count → exact
re-aggregation) then evicts, tier horizons evict rollups, bucketed reads merge tiers where raw is
gone — the table stops growing forever. 11 new tests (7 crate + 4 host MCP) incl. the mandatory
capability-deny (every verb, both read modes) + workspace-isolation (cursor replay, policy/gc); full
workspace green. Docs: [retention scope](scope/ingest/series-retention-scope.md),
[session](sessions/ingest/series-plane-readiness-session.md), paging/decimation scopes marked
shipped, public in `doc-site/content/public/datasources/datasources.mdx`.

**Previously shipped (2026-07-13): subject-scoped `bus.watch` grants + revoke-terminates-stream
(`bus-watch-subject-scope`, issue #49, tagged `node-v0.4.3`).** Closes two data-isolation gaps on the
generic bus motion plane so an embedder can stream a per-entity feed safely. **Gap 1:** a
`bus:<subject>:watch` scoped grant (new `Action::Watch`, `Surface::Bus`, wildcard-capable) narrows
`bus.watch` — coarse `mcp:bus.watch:call` unchanged, then "present ⇒ required, absent ⇒ open" (fully
backward-compatible; the scoped read is a live store read, so a post-login grant authorizes on next
subscribe). Converges the generic path with the channel `bus:chan/*:sub` subject-cap grammar onto one
model. **Gap 2:** an open SSE stream re-checks its grant on a bounded tick (`WatchRecheck`, 3s;
node-local, symmetric-safe) and closes when the grant is revoked — mode-sticky so revoking a caller's
*last* grant denies (never re-opens). Additive: no WIT/ABI/SDK change; one new host file
(`bus/scoped.rs`) + one gateway file (`events/recheck.rs`). 12 host + 2 gateway (real node/bus/gateway)
+ 5 unit tests green, incl. the mandatory capability-deny and workspace-isolation. Unblocks cc-app
`care.feed.watch` (milestone 10) to upgrade from reach-check-at-subscribe to platform stream isolation.
Docs: [scope](scope/bus/bus-watch-subject-scope-scope.md),
[session](sessions/bus/bus-watch-subject-scope-session.md), public in
`doc-site/content/public/auth-caps/auth-caps.md`.

**Previously shipped (2026-07-12): the pack toolchain is published for embedders (`pack-toolchain-publish`,
tagged `node-v0.3.3`).** `lb-devkit` + `lb-pack` dropped `publish = false` — the artifact
packager/signing idiom is now git-tag-consumable (`cargo install --git …lb --tag node-v0.3.3 lb-pack`),
unblocking cc-app's `make dev` wall (`cargo build -p lb-pack`: no such package). The load-bearing part
was the **API audit**: `lb-devkit`'s published contract is minimized to the pack surface
(`sign_artifact` / `load_or_create_key` / `publisher_trust_line` / `LoadedPublisherKey` / the signed
`Artifact` re-exported from `lb-registry`; the internal build-listing struct renamed `BuildArtifact`);
everything else moved behind the default-on `devkit-full` feature — explicitly NOT an embedder
contract until the `lb-ext` CLI stabilizes it. Trust model unchanged (signing is local; trust stays
node-side in `LB_TRUSTED_PUBKEYS`). Proven by real-binary tests (`rust/tools/pack/tests/`):
pack→`verify_artifact` round-trip, untrusted-key deny, tamper, determinism, and a publishable-chain
metadata check (fails on old master) now also a CI step. Docs: dev-flow section in
`public/extensions/extensions.md`, `docs/skills/lb-pack/SKILL.md` (grounded in a live git-install +
pack run), [session](sessions/extensions/pack-toolchain-publish-session.md).

**Previously shipped (2026-07-12): agent loop hardening — all five slices, in-house runtime
(branch `agent-loop-hardening`, one commit per slice, not yet merged).** The in-house loop now
(D) gets **typed provider faults** (`ProviderFault`: status + `Retry-After` + overflow
discriminant; `ModelAccess::turn` → `Result`; transient → bounded retry *below* step accounting,
fatal → job **Failed** + `RunFinish(Failed)`, never a fault dressed as a completion; the gateway
never caches a fault; `MockProvider::scripted()` failure arm); (A) **context compaction** —
chars/4 preflight incl. tool schemas, whole-turn-group drops (system/goal/latest-user protected),
one cumulative breadcrumb, provider overflow → compact + continue the SAME run
(`agent.config.compact_budget`, default 48k); (C) the **dangling-tool-call invariant** — every
transcript append through ONE chokepoint (`TranscriptWriter`), new additive
`ToolCancelled` transcript/run events, dead turns resolve pending proposals, load-time heal of
pre-fix orphans **appended at the cursor (never renumbered)**; (B) a **loop detector**
(window 20 default, `agent.config.loop_window`, 0=off; exact-repeat/ping-pong/interleaved
no-progress; warn → block → break ladder with reset-on-progress) + a **graceful ceiling exit**
(one tools-free summary completion, persisted); (E) **`emits_external` taint** on
descriptors/manifests (self-declared, opaque — rule 10) + `agent.config.exfiltration_guard`
(menu exclusion AND dispatch deny). Zero new verbs/tables; three additive `agent.config` axes,
each proven ws-walled. External-runtime coverage of B/E explicitly waits for the capability wall.
~21 new Rust tests green (fault table, compaction properties, heal/renumber, ladder, exfil deny)
+ the agent regression suites; pre-existing `agent_persona_catalog_test`/`agent_persona_coding_test`
failures verified identical on clean master (not chased). Scope
[`scope/agent/agent-loop-hardening-scope.md`](scope/agent/agent-loop-hardening-scope.md); session
[`sessions/agent/agent-loop-hardening-session.md`](sessions/agent/agent-loop-hardening-session.md);
public [`doc-site/content/public/agent/agent.md`](../doc-site/content/public/agent/agent.md)
("Loop hardening"). **Named follow-ups:** wall-level detector/guard for external runtimes
(capability-wall scope); budget-gate the ceiling summary + retry usage when close-out A/B ship;
`lb-ext-sdk` manifest gains the optional `emits_external` authoring field.

---

**Earlier (2026-07-12): flows plain wiring — the `link` pair removed, every port fires per
message (`flow-plain-wiring`, branch `flow-plain-wiring`).** Plain wiring is now the whole story —
exactly the Node-RED model: N wires onto ANY node's input port ⇒ one firing per arriving message, no
barrier, no binding demand, no policy question; one output port fans to every wired downstream.
**The default join policy flipped to `any` for every node kind** at all four sites together
(`join_of` — the Sink branch deleted, not inverted; the run-store fallback; the save lint's policy
read; the UI `joinOf` mirror). `JoinPolicy::All` survives only as a descriptor-level opt-in
(`[[node.input]] join = "all"`); **no built-in declares it** (audited), so the funnel/merge glyphs
vanish from the default authoring surface (a `PolicyMark` renders ONLY on an explicit-`all` port).
**Removed:** `link-out`/`link-in` + all machinery (`builtins/link.rs`, the resolver/validator
`link.rs`, five link `DagError` variants, the coordinator/save call sites, the `link-in` dispatch
leg; `flows.nodes` = **33** built-ins, no `Links` category) + the dead code
(`indegrees_within_by_port`, `edges_into`, `ready_frontier`, `UnboundJoin`). **The blocker fix:** a
matched `switch` released its dependent through the barrier path unconditionally — under universal
`any` that HANGS the mainline topology (switch + 2 plain wires into one node seeds a Pending
indegree-3 barrier slot the any-firings never touch). Matched release is now policy-aware through
the one `release_one_dependent` seam (an `any` dependent gets a normal minted firing,
`triggered_by = switch`; explicit-`all` keeps the barrier) — fail-before verified (the new test
hangs on the reverted path); debug entry
[`debugging/flows/matched-switch-hangs-run-after-any-default-flip.md`](debugging/flows/matched-switch-hangs-run-after-any-default-flip.md).
**Two engine refinements the flip forced:** (1) a **single-wire `any` port PROPAGATES** the incoming
`fctx` (only a ≥2-wire port mints) — a linear chain never grows its lineage and keeps byte-identical
claim keys; (2) **`${steps.X}` resolves along the firing lineage** (`is_ancestor` whole-segment
prefix walk, nearest settle wins) — a grandparent binding down a chain resolves instead of silently
binding null, and a genuine cross-branch reference is a new **save lint** (graph-ancestor check via
`referenced_step`). Plus a **run-load unknown-kind guard** in `coordinator::start`/`drive` (the
cron/source reactors never re-save, so an armed persisted link flow fails with a clear
unknown-kind error, not a tool denial); version-pinning order intact. **Tests (real store/caps/
gateway, rule 9):** `lb-flows` unit 96; `flows_run_test` **49** (headline transform-funnel +
reactive posture, THE blocker + gated + explicit-all-barrier switch cases, lineage binding,
cross-branch lint, duplicate-wire pin, output fan-out, suspend/resume-between-any-firings slot
rebuild, run-load guard, per-firing cap-deny/outbox-dedup/ws-isolation — explicit-`all` via a real
`record_install` fixture); all 15 host flows binaries green; UI `flowGraph` 22, `FlowsCanvas.gateway`
15/15 (pins 33 built-ins, no link kinds, no built-in `all` port), `flowsDebug`/`FlowsRuntimeControl`/
`FlowDashboardBinding` gateway green. Scope
[`scope/flows/flow-plain-wiring-scope.md`](scope/flows/flow-plain-wiring-scope.md); session
[`sessions/flows/flow-plain-wiring-session.md`](sessions/flows/flow-plain-wiring-session.md); public
[`doc-site/content/public/flows/flows.md`](../doc-site/content/public/flows/flows.md). **Named
follow-ups:** mixed-port extension nodes (explicit-`all` + other wires) still hit port-blind barrier
counting + a primary-port-only lint (only reachable via the opt-in); collect-join detection carries
over.

---

**Just shipped (2026-07-12): a native sidecar learns WHO called it + can reach ABOUT a subject →
`sdk-v0.4.0` + host.** Two generic platform gaps blocked a native (Tier-2) sidecar from enforcing
per-caller row visibility (the childcare product's guardian-isolation invariant): **(A)** the native
call frame carried no caller — `CallParams {tool, input}` was all the host sent, so every dispatch
defaulted to a synthetic admin that bypasses the row filter; **(B)** `authz.check_scoped`/
`scope_filter` answered only about the caller's OWN token, but a sidecar holds the *extension's*
token, not the guardian's. Both closed generically + additively (rule 10, no product named). **A:**
`CallParams` gains an additive `caller: Option<Caller>` (`{sub, ws, role, delegated}`, non-replayable,
`#[serde(default)]` → an old host/child is unaffected, no `PROTOCOL_MAJOR` bump); the host projects
the already-authorized `&Principal` into it through `CallContext` → `SidecarDispatch`; `Tools` gains a
`call_with_caller` default-method (forwards to `call`, so identity-unaware extensions need no change).
**B:** `authz.check_scoped`/`scope_filter` gain an optional `subject`, gated by the new marker cap
`mcp:authz.delegate_reach:call` — present ⇒ resolve the subject's reach; absent ⇒ byte-for-byte
today's behaviour; a `subject` without the cap **fails closed** (403, never a fallback to the caller's
own reach). Decisions: subject-reach **(a)** (parameterize, not a sibling verb); projection **minimal**.
Real-infra tests green (rule 9): a REAL spawned `echo-sidecar` reflects the stamped caller
(`native_caller_identity_test`); `delegated_reach_test` **5/5** (allow, the sacred deny, absent-subject
unchanged, cross-ws isolation) over the real gateway; SDK unit **21/21** (incl. two backward-compat).
**Release:** push `sdk-v0.4.0` then cut the node tag; a downstream embedder bumps both, requests
`mcp:authz.delegate_reach:call` at install, and flips its rule-7 chokepoint on with no call-site change.
See [native-caller-identity session](sessions/extensions/native-caller-identity-session.md).

**Earlier (2026-07-11): native host-callback client PUBLISHED through the SDK →
`sdk-v0.3.0` + `node-v0.3.0`.** An out-of-tree native (Tier-2) extension could speak the host→child
control wire (`lb-ext-native`) but had no way to call BACK into the host's MCP surface — the callback
client (`SidecarClient`) lived only in the lb monorepo as a path crate. Now it's a first-class SDK
crate (`lb-sidecar-client`, `NubeDev/lb-ext-sdk`), **re-exported from `lb-ext-native`** so a native
extension carries one dependency for both directions of the wire; lb consumes it **back** by git tag
(dropped the in-tree `crates/sidecar-client`). Verb-agnostic (rule 10) — the motivating consumer is a
downstream native authz chokepoint calling the generic `authz.check_scoped`/`authz.scope_filter`.
Host end (`/mcp/call`) unchanged; no WIT/grammar change. Real-infra tests green: SDK
`host_callback_test` (round-trip + 403→`Denied`) + lb `native_callback_test` (3/3 real-gateway:
round-trip, capability-deny, workspace-isolation), all building against the git tag. Also fixed a
latent `lb-ext-native` serve-test EOF hang. See
[native-callback-sdk-export session](sessions/extensions/native-callback-sdk-export-session.md).

**Earlier (2026-07-11): `updates-to-core` RELEASED.** The branch's two
release-blocking gaps are closed and the branch is merged to `master` and tagged:
**`node-v0.2.0`** (lb core + node), **`minimal-shell-v0.2.0`** (`@nube/minimal-shell` 0.2.0),
**`ui-v0.7.0`** (`@nube/ext-ui-sdk` 0.7.0, sibling repo).

- **Relay boot wiring (the blocker):** node boot now spawns the outbox relay — a generic
  `RouterTarget` (opaque `effect.target` dispatch, rule 10) registering `EmailTarget` +
  `PushTarget`, providers injected via the additive `BootConfig.outbox_providers` seam (unset ⇒
  logging no-op: never crash boot, never strand effects). Drain-at-boot proven end to end in
  `rust/node/tests/relay_boot_test.rs`.
- **i18n (en+es, the one catalog engine everywhere):** invite `locale` (record + `invite.create`
  param + pre-auth `GET /public/invite/verify` + copied to the member's `language` pref on
  accept); invite email rendered through the catalog (`invite.email.*` in `en.mf`/`es.mf` — the
  "no templating in core" non-goal is overturned, recorded in the invites scope); `notify.send`
  catalog key+args with **per-recipient** render in `PushTarget`; `@nube/ext-ui-sdk` i18n seam
  (`resolveLocale`/`makeTranslator`/`catalogParity`) + fully-catalogued minimal-shell with a CI
  key-parity gate. Tests: `invite_i18n_test` (5), `push_i18n_test` (3), `relay_boot_test` (2),
  shell vitest 9, SDK vitest 20.
- **Known allowed failure on the tag:** pre-existing `lb-cli reminder_test` deny — logged at
  `debugging/cli/reminder-create-denied-in-cli-round-trip-test.md`, not chased.
- **Deferred (explicit):** media HTTP Range, real WebPush VAPID / FCM / APNs / SMTP providers,
  orphaned-upload GC — all behind shipped traits/seams.

Scope: [`scope/release/updates-to-core-release-scope.md`](scope/release/updates-to-core-release-scope.md);
session: [`sessions/release/updates-to-core-release-session.md`](sessions/release/updates-to-core-release-session.md).
Downstream: cc-app milestone 00 unblocked (pins `node-v0.2.0` / minimal-shell 0.2.0).

---

**Earlier the same day (2026-07-11): the five cc-app platform-gap scopes — entity-scoped grants, invites,
media, push-target, minimal-shell.** Five scopes built end to end for the downstream cc-app
childcare product, each with full verb surfaces, capability-deny + workspace-isolation tests, and
session docs. All on branch `updates-to-core`.

1. **Entity-scoped grants** — row-level reach inside a workspace. Additive `Scope` selector on
   the grant record (`All` default = zero migration; `Ids` narrows to listed record ids).
   `resolve_caps_scoped` unions per-cap scopes; `check_scoped`/`scope_filter` are the host-facing
   read API extensions reach via `host.call-tool` (no WIT change — more additive than the flagged
   host-callback pair). Scope:
   [`scope/auth-caps/entity-scoped-grants-scope.md`](scope/auth-caps/entity-scoped-grants-scope.md);
   session: [`sessions/auth-caps/entity-scoped-grants-session.md`](sessions/auth-caps/entity-scoped-grants-session.md).
   Tests: lb-authz 15 + lb-host 7.

2. **Invites** — token onboarding for people who don't exist yet. `Invite` record (hash-at-rest,
   single-use, workspace-scoped). Admin verbs: create/list/revoke/resend (gated
   `mcp:invite.create/list:call`). Pre-auth accept route (`POST /public/invite/accept`) — the
   atomic onboarding chain: verify token → create-or-match identity → set credential →
   membership.add → apply grants → mint session. Email outbox `Target` + `EmailProvider` trait
   (the one sanctioned external). Email-match takeover prevention. Scope:
   [`scope/auth-caps/invites-scope.md`](scope/auth-caps/invites-scope.md);
   session: [`sessions/auth-caps/invites-session.md`](sessions/auth-caps/invites-session.md).
   Tests: lb-host 11.

3. **Media** — resumable chunked upload + variant jobs + streaming serve. Protocol: `begin` →
   `PUT /media/{id}/chunk/{n}` → `commit` (SHA-256 verify, flip to Ready, derive thumbnail via
   the `image` crate). Serve route (`GET /media/{id}?variant=thumb`) with ETag. Per-mime max size
   (the governed knob — the 413 lesson). One datastore (SurrealDB — rule 2). Scope:
   [`scope/files/media-scope.md`](scope/files/media-scope.md);
   session: [`sessions/files/media-session.md`](sessions/files/media-session.md).
   Tests: lb-host 9.

4. **Push target** — push as an outbox `Target` (WebPush first). Device records (per-member,
   self-only). `notify.send` verb enqueues a push effect. `PushTarget` impl `Target`: resolves
   audience → live devices, checks quiet-hours prefs (`push_muted` axis), auto-disables on
   `TokenGone`. `PushProvider` trait (one sanctioned external). Scope:
   [`scope/inbox-outbox/push-target-scope.md`](scope/inbox-outbox/push-target-scope.md);
   session: [`sessions/inbox-outbox/push-target-session.md`](sessions/inbox-outbox/push-target-session.md).
   Tests: lb-host 9.

5. **Minimal shell** — the publishable minimal host for 100%-extension UIs. `@nube/minimal-shell`
   package (~15 files): auth screens (login + invite-accept API), `ext.list` discovery,
   full-screen scoped mount via `@nube/ext-ui-sdk` federation seam, SSE hub, theme-token
   provider, PWA manifest. No lb chrome. Extension id is opaque config data (rule 10). Retires
   vendor-the-whole-shell. Scope:
   [`scope/frontend/minimal-shell-scope.md`](scope/frontend/minimal-shell-scope.md);
   session: [`sessions/frontend/minimal-shell-session.md`](sessions/frontend/minimal-shell-session.md).
   Tests: 2 unit.

**Test totals (new scopes):** 15 + 7 + 11 + 9 + 9 + 2 = **53 new tests**, all green. All existing
authz/admin_crud/builtin_roles/catalog tests green (no regressions). `cargo fmt` clean. `cargo
build --workspace` clean.

**Peer-review hardening pass (2026-07-11, same branch):** an independent review of the five
scopes found and fixed real holes in each — entity-grants **fail-open widening to `Scope::All`**
(malformed selector + cross-table union; new additive `Scope::Tables` variant, gateway scope
passthrough), invites **accept race** (credential written before redemption was claimed; now a
store-level CAS claim) + rate limit on the public accept route + email proven through the real
relay, media **unchecked chunk PUT** (uncapped, unvalidated, post-commit tampering; now a gated
host verb) + Range/304 serve + multi-chunk resume test, push **hardcoded `"acme"` workspace in
`deliver()`** (rule-6 hole; ws now rides the effect payload) + per-device retry dedup + ULID
effect ids + 7 relay-driven tests (deliver had zero) + the `push_muted` prefs axis was silently
dropped by the store schema, minimal-shell **SSE subscribe missing auth header** + 401
stale-session + `getSession` snapshot loop. Every fix has a regression test; deviations from the
scope docs (variant-jobs inline, WebPush adapter, workspace pick, publishing, hardcoded media
limits, no GC) are now recorded honestly as deferred items in each scope doc instead of ✅s.
Debugging entries: see `docs/debugging/README.md` rows dated 2026-07-11 (6 new). Review-fix
sessions in `docs/sessions/{auth-caps,files,inbox-outbox,frontend}/*review-fixes*`. Known
pre-existing red (NOT this branch): `agent_persona_catalog_test` (personas grounding — zero
persona/agent/assets files touched by these scopes).

**Previously (2026-07-11): `federation` promoted to a first-class core crate.** The federation
datasources sidecar moved out of the misleading `rust/extensions/` folder to
[`rust/crates/federation/`](../rust/crates/federation/) — it is **core, not a product extension**
(fails the rule-10 swap test: the host holds a first-class `federation.*` surface + `FED_ENDPOINTS`;
shares `lb-supervisor` verbatim; is platform datastore-federation surface). It is **still** a supervised
Tier-2 sidecar: its DB drivers (datafusion/postgres/rusqlite) link ONLY into this crate — `cargo tree`
confirms `lb-node`/`lb-host` link zero DB drivers, and the host still spawns it as a separate 311MB ELF
process over stdio from the shared `target/` dir (source-relocation only; manifest/caps/`exec`/wire/
`-p federation` all unchanged). Proven live: `federation_sqlite_test` green (real node → real sidecar →
register datasource → SELECT real rows + cap-deny + `net:*` deny + ws-B isolation, no-Docker sqlite
path); the moved `include_str!` manifest paths pass across the host federation suites. So the upcoming
`rust/extensions/*` cleanup does NOT touch federation. Scope:
[`scope/extensions/federation-promote-to-core-scope.md`](scope/extensions/federation-promote-to-core-scope.md);
session: [`sessions/extensions/federation-promote-to-core-session.md`](sessions/extensions/federation-promote-to-core-session.md).

**Previously (2026-07-10): shell chrome layout — header style + top-nav mode.** Two new
**appearance axes** on the existing Layout tab (Settings → Theme → Layout), additive and
migration-safe, riding the same `ui_theme` prefs blob as every other Layout axis (no new verb/cap/
table/MCP surface — reuses `prefs.set` / `set_default` / `resolve`). **(1) Header style**
(`ThemeLayout.header: "band" | "breadcrumbs"`): `band` (default) is today's `AppPageHeader`
icon-chip band, pixel-identical and untouched; `breadcrumbs` is a **clean shadcn/ui `Breadcrumb`**
header rendering `Workspace / <Surface>` — the shadcn look exactly (no icon chip, no gradient, no
sub-line), with the trailing actions slot (workspace chip + Settings gear) preserved. **(2) Nav mode**
(`ThemeLayout.nav: "sidebar" | "topmenu"`): `sidebar` (default) is today's left `NavRail`; `topmenu`
is a horizontal **shadcn `Menubar`** mounted above the content — the rail is omitted entirely (the
chosen renderer is the *only* nav mounted). Each `SURFACE_GROUPS` bucket becomes a `MenubarMenu`; a
resolved/curated nav renders the same way (flat entries fold into a leading "Menu", `group`s become
their own menus); Pinned + Extensions get their own menus when non-empty; the no-lockout escape
hatch + Sign out live in a right-aligned account menu. The top menu is a **second renderer** over the
exact same resolved-nav data the rail consumes — not a new source of truth; ext ids stay opaque
`ext:<id>` (rule 10). When `nav==="topmenu"` the sidebar-only controls (Variant/Collapsible/Position)
are marked "sidebar only" but keep their values (no hidden state). **shadcn primitives added:**
`breadcrumb.tsx` (reaches `@radix-ui/react-slot` — no new radix dep) + `menubar.tsx` +
`dropdown-menu.tsx` (two NEW `@radix-ui/*` deps), all themed to the shell tokens (`bg-panel`/
`text-fg`/`border-border`/`text-muted`/`accent`) like `sidebar.tsx`. **`itemRef` extracted** to
`nav-item-ref.ts` so both renderers share the one hide/pin grammar (no drift). **AppPage uses
`useThemeOptional`** so standalone `/panel` renders (no provider) fall back to `band` gracefully.
**Tests (real store/bus/gateway/caps, rule 9):** unit 25 files / **127 green** across `lib/theme` +
`features/theme` + `features/shell` + `components/app` — incl. the `normalizeLayout` migration-safety
guarantee (an old stored theme with no header/nav stays `band`/`sidebar`), the LayoutTab "sidebar
only" hint + value-retention, `TopMenuNav` (fallback-as-menus, resolved nav, Pinned/Extensions,
opaque ext ids, escape hatch, hidden-set) and `HeaderBreadcrumbs` (the shadcn trail + actions-slot
parity); gateway `theme-prefs.gateway.test.ts` **6/6** — the WIDENED blob now carries
`header:"breadcrumbs", nav:"topmenu"` and round-trips through a fresh-boot re-resolve (the
prefs-closed-struct class of bug), with the existing **capability-deny** + **workspace-isolation**
cases covering the new axes. `pnpm exec tsc --noEmit` clean. Scope
[`scope/frontend/shell-chrome-layout-scope.md`](scope/frontend/shell-chrome-layout-scope.md) (OQs all
resolved as recommended); session
[`sessions/frontend/shell-chrome-layout-session.md`](sessions/frontend/shell-chrome-layout-session.md);
public [`public/frontend/shell-chrome-layout.md`](public/frontend/shell-chrome-layout.md). **Named
follow-ups (not gaps):** breadcrumb depth beyond two levels; top-menu responsive collapse on narrow
viewports; a per-page sub-title registry for richer crumb trails.

---

**Just shipped (2026-07-09): flow input ports — Slices 2–4 (the `any` runtime + `fctx`, and the
per-port canvas).** *(Partially overturned 2026-07-12 by `flow-plain-wiring`, above: the `link-out`/
`link-in` pair and the `all`-barrier default shipped here were removed; the structural seams below
stand.)* **The seam (Slice 2):** a **firing context (`fctx`)** — a per-message identity carried in an
additive envelope field, minted per multi-wire release (deterministic per `(node, upstream, parent)`;
nested fan-ins extend it segment-by-segment), so every claim key + `${steps.*}` resolution is scoped
by `(node, fctx)` and multiplicity survives downstream without a per-event fan-out storm. Empty
`fctx` ⇒ `{run}:{node}` byte-for-byte; non-empty ⇒ `{run}:{node}@{fctx}`. The outbox dedup tripwire
is threaded: a sink's effect id is `{run}:{node}@{fctx}` ⇒ N firings are N idempotent deliveries.
**Slice 4 — the canvas:** per-named-input-port handles (a multi-port node stacks them, primary
anonymous + non-primary `id = portName`); `flowToEdges` labels a named-port wire with its `toPort`
(the wire-inspector surface). Debug:
[`debugging/flows/multi-input-node-fires-once-not-per-message.md`](debugging/flows/multi-input-node-fires-once-not-per-message.md).
Scope [`scope/flows/flow-input-ports-scope.md`](scope/flows/flow-input-ports-scope.md) (shipped,
partially overturned); sessions [`slice2`](sessions/flows/flow-input-ports-slice2-session.md) /
[`slice3`](sessions/flows/flow-input-ports-slice3-session.md) /
[`slice4`](sessions/flows/flow-input-ports-slice4-session.md).

---

**Previously shipped (2026-07-09): flow input ports — Slice 1 (the data-model foundation).** The first of
four slices of `flow-input-ports-scope.md` — the Node-RED multi-input model, done right. An edge
gains a **target input port** (`to_port`, additive `inputs` metadata on the node; `None` ⇒ the
primary input), and each input port declares a **join policy** (`all` barrier | `any` funnel) in a
new descriptor table (`inputPorts` / `[[node.input]]`). **Slice 1 ships the data model only** — the
port label + the policy ride as data the editor renders; the run engine still treats every input as
an `all` barrier, so behaviour is **byte-identical** and the existing join lint (≥2 upstreams must
bind `payload`) is **unchanged** → **no silent gap** (a 3-wire-into-`debug` flow still saves-rejected
until Slice 2 honours `any`). Built: `lb-flows` `InputEdge{from,to_port}` + `Node.inputs` (additive,
pre-ports flows load with empty ⇒ primary) + `JoinPolicy`/`InputPort` + `NodeDescriptor.input_ports`
+ `join_of()`/`primary_input()` + per-port graph math (`edges_into`, `indegrees_within_by_port`) +
`[[node.input]]` manifest parse; `lb-host` registry-aware save lints (undeclared-port + no-input-port
errors); UI wire types (`JoinPolicy`/`InputPort`/`InputWire`) + `flowGraph` round-trips `to_port` on
the React Flow `targetHandle` (primary wires stay implicit). **Slicing recorded:** the `any` default
flip + the lint relaxation MUST land with the runtime that honours them (Slice 2) — declaring `any`
without the runtime would let a funnel save green then silently join. **Tests (real store/caps/gateway,
rule 9):** `lb-flows` unit **91** (+5 port: edge round-trip, pre-ports-node-loads, `edges_into`,
`indegrees_within_by_port`, port/needs-agreement); `lb-host` flows integration **127 across 13
binaries** (the test sweep — every `Node{}` literal gained `inputs`; +4 new port-lint cases in
`flows_run_test` incl. the no-silent-gap join-lint-still-holds guard → 29 there); UI `flowGraph.test`
**15** (+3: `targetHandle` load, named-port round-trip, pre-ports clean shape). `cargo fmt` clean;
`pnpm exec tsc --noEmit` adds no new flows errors. **Pre-existing red fixed in passing:**
`flows_nodes_test` BUILTINS const was missing `debug` (stale since the debug node shipped) — corrected
(verified red on master first). **Pre-existing reds NOT this slice:** `DebugValueView.test.tsx` (2 —
identical on master). Scope
[`scope/flows/flow-input-ports-scope.md`](scope/flows/flow-input-ports-scope.md) (slice-progress note
added); session
[`sessions/flows/flow-input-ports-slice1-session.md`](sessions/flows/flow-input-ports-slice1-session.md);
public [`public/flows/flows.md`](public/flows/flows.md). **Next up:** Slice 2 — the `any` runtime +
the propagated firing-context (`fctx`) seam (the load-bearing piece: a per-firing id carried in the
envelope that scopes every claim/binding/job/outbox key so multiplicity survives one hop past the
funnel; empty in the all-`all` case ⇒ today's key byte-for-byte).

---
**Just shipped (2026-07-09): the standalone `full` desktop build persists by default.** Reported: a
desktop restart lost all user work. Cause: `Node::boot` opens `Store::memory()` unless
`LB_STORE_PATH` is set, and the desktop shell never set it — every launch was a fresh, ephemeral
node (a known, already-recorded non-goal, not a regression — but not acceptable for a shipped app).
Fix: `ui/src-tauri/src/store.rs::ensure_store_path()` resolves the OS-standard per-user data dir
(`dirs::data_dir()/lazybones/store`) and fills `LB_STORE_PATH` **at the windowed binary boundary**
(`desktop.rs::run`, before `NodeHandle::boot`) if it's unset — so `open_store` opens the persistent
SurrealKV engine it already supports for `make cloud`/`edge`. Deliberately NOT set inside
`Node::boot`/`NodeHandle::boot`: those are called directly by the shell's own tests, which must stay
ephemeral + isolated (a shared on-disk path would cross-contaminate concurrent runs) — the windowed
`run()` path is the one only the shipped app takes. An explicit `LB_STORE_PATH` (custom/portable
location, or empty for ephemeral) always wins. Boot seeders are unchanged (already idempotent
LWW-upserts), so they refresh built-ins across app updates without touching user data. **Tests (real
on-disk SurrealKV, rule 9):** `full_persist_test.rs` — boot `full` against a temp store path,
register a datasource the seeders don't create, drop the node+gateway, RE-BOOT at the same path,
assert it's still listed (the regression); `full_loopback_test`/`full_federation_test` stay green
with no `LB_STORE_PATH` set, proving the default lives at the right seam. **Manually verified by the
reporting user on a real `linux-full` build.** Non-goal (recorded): the gateway signing key is still
fresh per launch, so a restart re-logs the user in even though their data is intact — a sibling
follow-up. Scope
[`scope/desktop/desktop-persistent-store-scope.md`](scope/desktop/desktop-persistent-store-scope.md);
session [`sessions/desktop/desktop-persistent-store-session.md`](sessions/desktop/desktop-persistent-store-session.md).

**Previously shipped (2026-07-09): the federation datasources sidecar is bundled into the `full`
desktop build.** The standalone `full` binary booted node + gateway (so login/MCP/SSE/agents/flows/rules
worked standalone) but had **one hole**: datasources. A user could `datasource.add` over the
loopback gateway, but `datasource.test` / `federation.query` returned an opaque "denied" — the
federation native sidecar that serves those verbs was not shipped in `full`, so no federation
`Install` record existed and `enforce_endpoint` refused. This slice **bundles the sidecar** into the
`full` package (`build.sh`/`build-windows.sh` build it sqlite-only — `rusqlite` bundled, no TLS/C
dep; the desktop `Makefile` copies `federation(.exe)` beside the shell) and **auto-installs +
supervises it at boot** (`ui/src-tauri/src/federation.rs::mount_federation`, called from `boot_full`
after the signing-key install) with a sqlite-loopback grant (`net:tls:127.0.0.1:0:connect` +
`secret:federation/*:get`), then pre-registers the shipped `demo-buildings.db` — so a double-clicked
binary registers **and** queries a sqlite source with zero setup. The install path is a **shared,
extension-agnostic helper** (`lb_host::install_federation`, taking manifest+grant+seed as opaque
data — CLAUDE §10) that both `node/src/federation.rs` (env-driven, `make dev`) and the desktop boot
call, so the grant/token computation lives in one place (no copy-paste drift — the bug class that
motivated this). **Desktop default = sqlite-only**; postgres registers but is refused pre-connect
until an admin widens the grant (deferred). **Tests (real sidecar/store/gateway, rule 9):**
`full_federation_test.rs` boots `full`, and over the loopback gateway proves the regression
(`datasource.test` → green, `federation.query` → rows), DSN redaction, the mandatory
capability-deny (unapproved postgres endpoint refused *with* the sidecar present), and
workspace-isolation (an `acme`-only source is unresolvable from ws `other`). `cargo fmt` clean.
Scope [`scope/desktop/desktop-federation-bundle-scope.md`](scope/desktop/desktop-federation-bundle-scope.md);
session [`sessions/desktop/desktop-federation-bundle-session.md`](sessions/desktop/desktop-federation-bundle-session.md).

**Previously shipped (2026-07-08): the desktop standalone full-stack build mode (`full` feature).** The
`lazybones-shell` desktop binary shipped in a **thin IPC mode** (Tauri window + 5 `channel_*`/
`agent_invoke` commands over a hardcoded demo principal) — the React UI bundled into the webview
is built for the HTTP/SSE gateway, so login and every gateway verb had nothing to answer them.
This slice adds a **second build mode** (not a rewrite): the cargo feature **`full`** (implies
`desktop`) mounts the SSE/HTTP gateway **in-process on `127.0.0.1:8800`** + runs the boot seeders
(`user:ada` → workspace-admin of `acme`, the core-skill/agent/persona catalogs, the four background
reactors), so the packaged binary is a 100% standalone node — login, MCP, SSE, agents, flows,
insights — with no external node. The webview talks to the loopback gateway over HTTP exactly as
the browser does against `make dev`; one transport-priority flip in `invoke.ts` (an explicit
`VITE_GATEWAY_URL` wins over Tauri IPC) makes the same UI work in both modes. **No `if desktop` in
any core crate** (rule 1) — the only switch is the shell crate's own cargo feature; the gateway is
reached through the same gated HTTP surface as the browser (rule 5/7). Make entrypoints:
`make -C desktop linux-executable` (thin, unchanged) / `make -C desktop linux-full` (full) /
`make -C desktop smoke-full` (xvfb boot + `curl /login` → token → real `POST /mcp/call`); Windows
peers (`windows-full`). **Tests (real store/bus/gateway, rule 9):** the optional-dep seam holds
(`cargo test -p lazybones-shell` feature-off 2/2 — the property that keeps every CI lane webkit-free);
`cargo test --features full` **6/6** incl. the headline `full_loopback_test.rs` — a NON-windowed boot
of `boot_full` on `127.0.0.1:0` + reqwest: login returns a real signed token for `user:ada`/`acme`,
that token drives a real `tools.catalog` `POST /mcp/call`, AND the mandatory deny (`user:stranger` →
403, the wall holds). `cargo fmt` clean. One host-only snap `libpthread` quirk blocked the local
WINDOWED smoke (not a code bug — runs clean in the container; the non-windowed test is the portable
proof) — logged. **Non-goals (recorded):** persistent store/key (fresh state per launch, seeders
idempotent), native sidecars (federation/control-engine — `make dev` for those), runtime port
(`8800` fixed, baked into the UI). Scope
[`scope/desktop/desktop-standalone-backend-scope.md`](scope/desktop/desktop-standalone-backend-scope.md);
session [`sessions/desktop/desktop-standalone-backend-session.md`](sessions/desktop/desktop-standalone-backend-session.md);
debug [`debugging/desktop/full-binary-snap-libpthread-crash-at-window-init.md`](debugging/desktop/full-binary-snap-libpthread-crash-at-window-init.md).

---

**Previously shipped (2026-07-08): flow editor UI polish — "less is more" chrome consolidation.** The flows editor's header dropped from ~10 always-visible controls to Deploy · Run⇄Stop (one morphing button) · Pause⇄Resume (one mid-run toggle) · Debug · `⋯` (new `FlowOverflowMenu`: Enable/Disable, Live values, Undo, Export…, Import…, Delete — with a header **"Disabled" badge** so the safety state never hides). Config and Debug became tabs in ONE resizable right dock (new `RightDock.tsx` — they can no longer co-render side by side). The node config panel got a real header/status-line/sticky-footer design with ONE context-aware primary action (`Save node`, or `Patch run` mid-run; `Save flow` dropped — Deploy is the only whole-flow write). Export/Import became a dialog (`FlowTransferDialog.tsx`): JSON preview, pretty⇄compact, Copy/Download, selected-nodes scope with a loud stripped-wires count, paste-or-file import through the real save path. **Found + fixed en route:** the shipped debug panel was never in the repo — a bare `debug` `.gitignore` pattern swallowed `ui/src/features/flows/debug/` at commit time; anchored the pattern and REBUILT the panel for real (SSE tail + type-aware rows + per-node filter; `debugging/frontend/flows-debug-panel-swallowed-by-bare-gitignore-pattern.md`). UI-only — no verb/descriptor/runtime changes. **Tests:** flows suite 10 files / 67 green (incl. new RightDock never-co-render + transfer-dialog cases), `tsc` + eslint clean. Scope
[`scope/flows/flow-ui-polish-scope.md`](scope/flows/flow-ui-polish-scope.md);
session [`sessions/flows/flow-ui-polish-session.md`](sessions/flows/flow-ui-polish-session.md).

---

**Previously shipped (2026-07-08): external-agent authoring S1–S4 — the MCP-shim bridge, persona unlock, themed UI template, Studio card.** A spawned external agent (Open Interpreter / codex-family) can now call the node's MCP tools — and only those — over a stdio MCP shim that forwards every `tools/call` to `POST /mcp/call` under a run-scoped token. The gateway re-checks `caps::check` on every call exactly as it does for the UI. **S1 (bridge):** new crate `lb-mcp-shim` (`rust/crates/mcp-shim/` — stdio JSON-RPC ⇄ HTTP), run-token mint (`role/external-agent/src/token.rs` — 5-min TTL, carries the derived principal's caps + the caller's `constraint` + `run_id`), gateway run-status gate D3 (`verify_token` refuses a terminal run's token even if unexpired — hard cancel is instant), refresh route `POST /agent/runs/{id}/token/refresh` (D2 — proactive at 60% TTL + one-shot 401 self-heal), per-wrapper MCP config (`CodexWrapper::mcp_config` writes `config.toml` under `<scratch>/codex-home/`). `Claims` gained `constraint` + `run_id` (both `#[serde(default)]` — legacy tokens deserialize unchanged). **S2 (persona unlock + Ask-over-bridge):** `extension-builder` `runtimes` unlocked for the external ids; `/mcp/call` enforces the persona Ask floor for run-scoped tokens — `ext.publish` returns "awaiting approval" as the tool result (the agent reports it and ends; the human decides in the dock/Studio; the effect fires from the suspension). **S3 (themed template):** the devkit's `ui` template is correct-by-construction themed — Tailwind v4, scoped shadcn-token aliasing (`.lbx-<id>`, never `:root`), recharts `LineChart` colored from `--chart-1`, no preflight. **S4 (skills + card):** skill docs gained the theming/bridge sections; the Studio Build tab gained a "Generate with agent" card (sugar over `agent.invoke` with `persona:"builtin.extension-builder"`). **Tests (real gateway, rule 9):** `mcp_bridge_test.rs` **8/8** (tools/list + tools/call round-trip + the MANDATORY cap-deny + D3 cancelled-run + ws-isolation + token-not-in-stdout + Ask-over-bridge); `run_token_refresh_route_test.rs` **6/6**; shim lib **10/10**; scaffold **6/6** (themed shape); contract-mirrors guard **5/5**; `tsc --noEmit` clean. **S5 (the two live E2E runs) gated on user confirmation** — the deterministic skeleton is proven; the model-in-the-loop confirmation needs a real Open Interpreter binary + `make dev EXTAGENT=1 DEVKIT_BUILDER=container`. Scope
[`scope/external-agent/agent-ext-authoring-scope.md`](scope/external-agent/agent-ext-authoring-scope.md);
session [`sessions/external-agent/agent-ext-authoring-session.md`](sessions/external-agent/agent-ext-authoring-session.md).

---

**Just shipped (2026-07-07): Field-tab baseline audit — what works vs what's dead.** The panel editor's
**Field** section was "overwhelming and half the options do nothing" (the user's report). The audit
proves it: a real-gateway test (`fieldTabBaseline.gateway.test.tsx`, 24 green) classifies every
registered Field-tab option as LIVE (setting it changes the rendered output observably) or DEAD (stored +
round-tripped, but no renderer reads it → zero visible effect). **The headline: ~half the `timeseries`
Field-tab options are DEAD** (`mappings`, `links`, `custom.lineInterpolation`/`gradientMode`/`showPoints`/
`spanNulls`/`axisPlacement`/`stacking.mode`/`thresholdsStyle.mode`) and **all of `table`'s per-column
`custom.*`** (width/align/cell-type/filter). The single-stat family (`stat`/`gauge`/`bargauge`/
`piechart`) is in good shape — the shared `valueFieldOptions` + `formatValue` + `applyMappings` bridge
covers the standard options. Root cause: the editor-parity phase registered Grafana's full surface to
make the EDITOR complete; the matching render work only landed for the standard bridge + a few graph
styles, and `registryRoundTrip.test.ts` only checks the de/serializer, not the renderer — so the gap was
invisible to the green suite. The baseline test IS the contract for the next step. **Built (Phase 1):** the
**panel wizard** — a stepped create flow (Source → Chart type → Options → Transform/Save) on its own route,
a thin shell over the existing `cellEditorState`/`writeOption`/`usePanelData` engine (no second surface →
no drift). The Options step is a **compact grouped form beside ONE pinned `OptionFocusPreview`** (scope
resolved decision #3): hovering/editing an option points the single preview's `optionFocus` at it; dead
options surface their honest "renderer pending" note in the row. (An earlier cut mounted a chart per option
card — ~20 simultaneous renders; redesigned to the one-preview surface, see
[`sessions/frontend/panel-wizard-one-preview-redesign-session.md`](sessions/frontend/panel-wizard-one-preview-redesign-session.md).)
Presentation-option edits re-shape cached frames via the shipped `viz.query` fetch/shape split (no backend
hit); only data steps re-query. Next: SourceStep discoverability (SQL/datasource picking) + the Phase-2
Field-tab port-back. Scope
[`scope/frontend/dashboard/viz/panel-wizard-scope.md`](scope/frontend/dashboard/viz/panel-wizard-scope.md)
(no open questions — all long-term decisions taken), handover
[`sessions/frontend/HANDOVER-panel-wizard-build.md`](sessions/frontend/HANDOVER-panel-wizard-build.md).
Debug entry
[`debugging/frontend/field-tab-options-that-do-nothing.md`](debugging/frontend/field-tab-options-that-do-nothing.md),
baseline session
[`dashboard-field-tab-baseline-session.md`](sessions/frontend/dashboard-field-tab-baseline-session.md).

**Just shipped (2026-07-06): agent context basket — dock Tools mode + `context_items`.** The dock
gained an **Ask | Tools** toggle that mounts the SHARED channel `CommandPalette` against the dock
session (same catalog / JSON-Schema arg rail — zero duplication), and a **context basket**: a
paperclip on every dock row gathers durable items (query results, rich responses, notes); the next
ask carries their ids as the new additive `AgentPayload.context_items` (refs, not bodies), and the
agent worker resolves + fences the bodies into the run's goal at drive time
(`crates/host/src/channel/context_items.rs` — ws/channel-scoped store reads, 8-ref reject cap,
8 KB/item truncation, untrusted-data fence; the sibling of the page-context fence). Rust lib 136
green (incl. the mandatory ws-isolation + cross-channel fence tests); dock gateway suite 10 green
incl. the end-to-end gather→ask→ref case. Scope
[`agent-context-basket-scope.md`](scope/agent/agent-context-basket-scope.md), session
[`agent-context-basket-session.md`](sessions/agent/agent-context-basket-session.md). **Next up:**
the same gather seam on the channels surface; asset/PDF refs once ingest exposes item handles.

**Also shipped (2026-07-06): `federation.sample` — one AI-ready snapshot of a datasource.** New MCP
verb `federation.sample {source, tables?, limit?}` returns, in ONE bounded call, every table's
columns, its **real foreign keys** (new best-effort `Source::foreign_keys` — SQLite
`pragma_foreign_key_list`, Postgres `information_schema`; default `[]`), and up to `limit` (10,
cap 50) sample rows — long cells truncated, `password`/`secret`/`token`-like columns redacted — the
context a model needs to write correct SQL without N+1 `federation.schema` probes (which carry no
FK metadata at all; the ERD infers joins by naming). Rides `federation.schema`'s exact pipeline +
the same `mcp:federation.query:call` cap (gate alias in `tool_call.rs`); descriptor in the
palette/agent catalog. e2e in `federation_sqlite_test.rs` (real-FK + 12-row + redaction fixture,
cap-deny + ws-isolation). Scope
[`datasource-samples-scope.md`](scope/datasources/datasource-samples-scope.md), session
[`datasource-samples-session.md`](sessions/datasources/datasource-samples-session.md), skill
[`datasources`](skills/datasources/SKILL.md). **Next up:** a "Copy AI context" button on
`DatasourceDetail`; the ERD real-FK upgrade.

**Also shipped (2026-07-06): saved queries on the Datasources page.** `DatasourceDetail`'s SQL editor
header (beside Run) gained Save / Saved-queries dialogs riding the existing `query.*` verbs — no new
verb/cap/table. `useDatasourceQueries.ts` filters the `query.list` roster client-side to
`target === "datasource:<name>"` and saves `lang:"raw"` records; loading resolves the full record via
`query.get`. Shipped truth: `public/datasources/datasources.md` §"Saved queries". The `querydef.*`
chain sketched in the query-builder scopes is dead — those scopes now build on `query.*`.

**Follow-up shipped (2026-07-06): the Open dialog gained copy + expand-to-view.** Each saved-query
row in `SavedQueriesDialog` now has (a) a clipboard copy button and (b) a chevron that expands the
row to render the SQL inline in the SAME read-only `SqlEditor` the workbench's Code mode uses (real
syntax highlighting, not a flat `<pre>`). Both lazy-load via the shipped `query.get` (one fetch per
row, cached for the dialog session). Session
[`saved-queries-copy-expand-session.md`](sessions/datasources/saved-queries-copy-expand-session.md).

**Also shipped (2026-07-06): the Query Builder is common across dialects — federation sources get the visual builder.**
A LOCAL TABLE source (SurrealDB, `store.query`) gets the interactive Builder⇄Code editor; an external
DATASOURCE (`federation.query` — postgres/timescale/sqlite, e.g. `demo-buildings`) USED to get only a
raw-SQL textarea. The lift was a deferral recorded in
[`scope/frontend/dashboard/viz/datasource-binding-scope.md`](scope/frontend/dashboard/viz/datasource-binding-scope.md)
("Deferred: `federation.datasource.schema` … a federation target uses the raw-SQL editor until it
lands") — liftable once `federation.schema {source, table?}` shipped. The builder UI is now **one
component for every datasource kind**: the dialect is behind a seam, never a fork. **(1) The emitter
seam** — `panel-kit/sql/dialect.ts` `emitSql(dialect, query)` dispatches between `toSurrealQL.ts`
(UNCHANGED — `math::sum(col)`, bare identifiers, `count()`) and a NEW `toStandardSql.ts` (ANSI SELECT
— `SUM("col")`, double-quoted identifiers, `COUNT(*)`, single-quoted string literals). The `dialect`
is keyed on the target's datasource `kind` (config data, never a hardcoded datasource name — rule 10).
**(2) The shared state stays put** — `panel-kit/sql/query.ts` (`SqlBuilderQuery`) is unchanged.
**(3) The editor becomes transport-agnostic** — `SqlQueryEditor` accepts `dialect: SqlDialect` +
`schema: Schema` props and stops importing `readSchema` itself; the host (`QueryTab.tsx`) decides the
source + the dialect. **(4) Federation dropdowns from `federation.schema`** — two new hooks
(`useLocalSchema`, `useFederationSchema`) wrap the shipped `readSchema()` / `discoverTables`+
`describeTable` and project to ONE `Schema` shape (the editor consumes one shape regardless of
dialect; `federation.schema` is workspace-pinned, gated under `mcp:federation.query:call`, lazy
per-table column fill). **(5) Migration** — a federation cell now carries `options.sql` (the builder
state) so reopening returns to the builder (the round-trip surreal had all along); a pre-slice
federation cell (no `options.sql`) reopens to Code mode with the saved SQL preserved — no fabricated
builder query. **Nothing else moves:** the wire shape (`federation.query {source, sql}`) is
unchanged; the render path (`viz.query`) is unchanged; NO new MCP verb / cap / table / outbox target /
host change. Surreal-path behaviour is preserved byte-for-byte (pinned by a surreal-regression
gateway test: `math::avg` preview, not `AVG("…")`). **Tests (real gateway, rule 9):** UI unit
**737/737** (was 705; +15 dialect/standard-SQL goldens, +2 cellEditorState federation round-trip
cases — builder-carried + pre-slice migration); new gateway
`queryBuilderCommon.gateway.test.tsx` **4/4** — HEADLINE (a federation target opens `SqlQueryEditor`,
NOT the legacy textarea; `federation.schema` fires) + surreal-regression (dialect=surreal confirmed)
+ the MANDATORY capability-deny (no `mcp:federation.query:call` → schema discovery denied → empty
dropdown, editor still renders) + MANDATORY workspace-isolation (ws-B's `datasource.list` does not
include ws-A's source). The federation sidecar is NOT spawned in the UI test env (a true external a
UI test cannot cheaply run) — `federation.schema` resolves to an honest typed error and the editor
DEGRADES to empty (the system-catalog deny contract); the real-row round-trip stays
`rust/crates/host/tests/federation_sqlite_test.rs`'s job (unchanged, stays green). `pnpm exec tsc
--noEmit` clean (only the pre-existing `transformDebug.gateway` red remains); `pnpm exec eslint` clean
on touched files. **Pre-existing reds (NOT this slice's — verified by `git stash` + rerun on clean
master):** `sqlSource.gateway`, `SystemView.gateway`, `WorkflowView.gateway`, `ProofPanel.gateway`,
`CommandPalette.reminders.gateway`, `App.gateway`, `PanelPage.gateway`, `AuthoringPanel.gateway`,
`McpServiceView.gateway`, `InboxView.gateway` — all fail identically on the stashed baseline; all
QueryTab/SqlQueryEditor-touching gateway tests (`panelEditor.gateway`, `DataStudioBuilderFlow.gateway`,
`DataStudio.gateway`, `flowsPanelEditor.gateway`) are GREEN. Scope
[`scope/frontend/query-builder-common-scope.md`](scope/frontend/query-builder-common-scope.md) (OQs
all resolved as recommended); session
[`sessions/frontend/query-builder-common-session.md`](sessions/frontend/query-builder-common-session.md);
public [`public/frontend/data-studio.md`](public/frontend/data-studio.md) ("Query builder — common
across dialects"). **Named follow-ups (not silent gaps):** the rail Sources-tree drill into
federation tables/columns (compose a `readFederationSchema` loader onto `@nube/source-picker`'s
`SourceLoaders`); a per-dialect time-bucket emit for the chart `time-series` format hint (the natural
trigger for splitting `toStandardSql.ts` per dialect); a CodeMirror standard-SQL grammar (cosmetic).

---

**Just shipped (2026-07-05): Data Studio 10x — Dockview workbench, pages-as-panes, query-first builder.**
The four-phase UI refresh of the multi-pane data workbench, landed end to end: **(1) engine swap** —
`flexlayout-react` is gone, `dockview-react` (MIT, React-first) is the one dock engine (tabs, nested
splits, floating groups, maximize, popout, JSON serialize/restore; theme via `--dv-*` CSS vars aliased
to the shell tokens under `.dockview-theme-lb`; tab titles ellipsize with full-title tooltips; double-
click rename via prompt). The persisted layout record is **versioned by engine** (`{engine:"dockview",
model}`); a legacy flexlayout blob falls back to the default workbench + a **one-time "layout was reset"
notice** (no silent draft loss). **(2) pages-as-panes** — a "+ Open view" header menu lists the core
surfaces (Flows, Rules, Data, Datasources, Ingest) and opens each as a dock pane mounting the **REAL
routed view component** (`FlowsView`/`RulesView`/…): same code path, same gateway, same caps — never a
re-implementation. An `embedded` mode on `AppPage` suppresses a view's own full-width header inside a
pane (the dock tab is the title bar); standalone routes keep it. One pane per view kind (the menu
re-activates an open pane); Data Studio itself is excluded (no recursive embedding); the kind set is
opaque data (rule 10 — the dock never branches on a host subsystem id). **(3) query-first builder** —
picking a source / opening a library panel / **New panel** opens ONE stacked builder tab whose stage 1
is a compact toolbar (inline title, Run, freeze/table/inspect, ONE Save split-button) + the focused
query editor; NO preview / viz pills / options until rows exist. Rows returned → stage 2 reveals the
live preview + a **viz gallery** (one thumbnail card per widget type; the 6 chart-likes get a live
mini-render through the ONE `viz.query`/`WidgetView` path, the 3 labeled cards Table/AI-widget/Template
don't; shape-gating mirrors `VizPicker`) + a collapsed searchable Options drawer (stage 3). The "Saved
as library panel …" banner is now a compact badge; Save-as-library lives behind the Save split-button's
caret menu (gated by `mcp:panel.save:call`). The gallery proves the ONE-query invariant — preview + 9
thumbnails share one `vizQueryKey` cache entry (the view is not part of the key); asserted in tests.
**(4) Demo data, honestly seeded (rule 9)** — when a query returns 0 rows AND the seeded SQLite
`demo-buildings` datasource exists, the empty preview offers "Preview with demo data" (real records
through the real `federation.query` engine, same render path); demo state is badged and AUTO-YIELDS the
moment the user's query has rows. The rail's Sources tab is now a `CatalogExplorer` host (the workspace
system catalog, shipped same day) — datasources → tables → columns, series, channels, insights as ONE
honest tree; click → builder tab with the studio's `onSelect` mapping. **No new verb / cap / table /
host change** — pure UI composition over shipped `layout.*` / `panel.*` / `viz.query` / each pane's own
verbs. **Tests (real store/bus/gateway/caps, rule 9):** UI gateway `DataStudio.gateway` **8/8**
(incl. the MANDATORY capability-deny + workspace-isolation + the legacy-layout fallback),
`DataStudioPanes.gateway` **5/5** (REAL view mounts, AppPage embedded mode, layout round-trip, deny
via `allowed`), `DataStudioBuilderFlow.gateway` **5/5** (query-first staging, ONE-query gallery
asserted via the `ipc.invoke` spy pattern, panel.save round-trip, demo integrity, viz.query deny);
units **33/33** across 5 new files (record versioning, pane registry, gallery type-mapping, drawer
disclosure, demo state machine — `useDemoPreview`'s only fake is the pure `useDatasourceList` seam per
the system-catalog precedent); `panelEditor.gateway` + `flowsPanelEditor.gateway` split-layout parity
**10/10** green. Frontend unit total **705/705** (was 672); `pnpm exec tsc --noEmit` clean (only the
pre-existing reds remain: FlowsCanvas.gateway, transformDebug.gateway). **One bug surfaced + fixed:**
the CODE-ONLY session's new files reintroduced bare `rounded` (banned by the radius-scale guard
shipped 2026-07-04); six offenders mapped to token-derived stops (`rounded-md` for menu items,
`rounded-sm` for tight chips) — debug entry
[`debugging/frontend/data-studio-10x-bare-rounded-radius-guard.md`](debugging/frontend/data-studio-10x-bare-rounded-radius-guard.md).
Scope [`scope/frontend/data-studio-10x-scope.md`](scope/frontend/data-studio-10x-scope.md) (open
questions OQ1/OQ3/OQ4 all resolved as recommended); session
[`sessions/frontend/data-studio-10x-session.md`](sessions/frontend/data-studio-10x-session.md); public
[`public/frontend/data-studio.md`](public/frontend/data-studio.md).

---

**Just shipped (2026-07-05): `@nube/source-picker` grew into the workspace system catalog.** The
package went from a *picker* (the shipped combobox) to a catalog with **two UI skins** — the
existing combobox AND a new browsable explorer tree (`<CatalogExplorer>`), extracted from the rules
panel's `DataExplorer`. **One loader seam (`SourceLoaders`), one orchestration (`loadCatalog`/
`useCatalog`), two projections**: the picker collapses a deny into an empty group (its existing
contract), the explorer surfaces a deny VISIBLY ("Not permitted.", never a fabricated roster). Per
section the explorer shows an HONEST tri-state — loading skeleton, "Not permitted." deny, teaching
empty, ready rows (incl. a table→column tree for `store.schema`). **Four new optional loaders land
over already-shipped verbs** — `readSchema` (`store.schema`), `listChannels` (`channel.list`),
`listInsights` (`insight.list`), `listInbox` (`inbox.list`); absent loader ⇒ absent section, so a
host composes which subsystems its surface shows. Sections are registry-driven data; the click yields
a `CatalogEntry` (a tagged row) the HOST maps onto its snippet (the rules panel: `source("name")` /
bare identifier / `history("series","name","24h")`). **The rules `DataExplorer` is now a thin
loaders-adapter + Rhai-snippet mapping; `useDataExplorer` is deleted; `ui/src/components/schema/` is
deleted (the package's `CatalogSchemaTree` is the one tree).** Honor the non-goals: no new node
verbs, no query execution/editing in the package (its one job is *enumerate + pick*), no
outbox/webhook sections (no roster verbs — named follow-ups). Self-themed via scoped `--sp-*` tokens
under `.sp-root.sp-catalog` (the @nube/panel discipline). **Tests (rule 9):** package unit **46/46**
(`useCatalog` per-section state + `CatalogExplorer` every-state rendering, with an injected fake
LOADER OBJECT — a pure function seam, NOT a fake backend); the real path proven by the host suites
— `AuthoringPanel.gateway.test.tsx` **7/7** (the parity gate), UI unit **672/672**, picker consumers
(DataStudio / framesIn / rulesSource / fieldNamePicker) untouched and green. Scope
[`scope/frontend/system-catalog-scope.md`](scope/frontend/system-catalog-scope.md) (open questions
all resolved); session
[`sessions/frontend/system-catalog-session.md`](sessions/frontend/system-catalog-session.md); public
[`public/frontend/frontend.md`](public/frontend/frontend.md) ("The workspace system catalog").

---

**Just shipped (2026-07-05): sidebar icon colors — Settings → Theme → Icon colors.** One click
auto-assigns every rail icon a distinct color from a **prefilled 100-color palette** (golden-angle
hue spread, frozen data); each icon is then individually editable via an in-DOM swatch popover
(10×10 grid + custom hex), and **Clear all** fully reverts. Rides the existing `ui_theme` prefs blob
— one new optional `iconColors` axis, zero backend change (no new verb/cap/table). Application is a
single inline `style={{ color }}` on the lucide `<Icon>` in `NavRail` (works in expanded and
collapsed rail). In-DOM popover, not a native `<input type="color">` — sidesteps the WebKitGTK
silent-no-op the appearance scope flagged. Scope
[`scope/frontend/theme-icon-colors-scope.md`](scope/frontend/theme-icon-colors-scope.md); session
[`sessions/frontend/theme-icon-colors-session.md`](sessions/frontend/theme-icon-colors-session.md).
Frontend green: 109 files / 672 tests; `pnpm exec tsc --noEmit` adds no new errors.

---

**Just hardened (2026-07-05, later the same day): the widget-builder E2E loop now works live.**
"add a widget for avg meter usage" (GLM-4.6, in-house runtime, `/dashboards`) now runs clean end to
end: schema discovery → proven query → one `dashboard.save` appending to the OPEN dashboard, owned
by the asking user. Five root causes fixed along the way: orphan `role:"tool"` messages made the
model ignore tool errors (conformant assistant-echo wire shape); `information_schema` probes are now
ANSWERED read-only instead of steered away; an error REPLY from the federation sidecar was treated
as a crash and burned the restart budget (five failed queries took federation dark); agent-created
dashboards were owned by the derived `agent:session` sub and invisible to the user
(`Principal::owner_sub` delegation root); `dashboard.save`/`share` arg ergonomics (descriptors,
stringified-JSON/null tolerance, `widget_type` default). Persona now targets the open dashboard
(page-context `search.d`) and must PROVE a query returns data before saving. `MAX_STEPS` 8 → 16.
Session [`sessions/agent/widget-builder-e2e-hardening-session.md`](sessions/agent/widget-builder-e2e-hardening-session.md);
five new debugging entries under `debugging/agent/` + `debugging/federation/`.

---

**Just fixed (2026-07-05): the agent menu was starved — `tools.catalog` now serves the full
host-native inventory.** A persona run (widget-builder on `/dashboards`) saw only 3 tools because
the catalog only enumerated the ~11 guided-palette descriptors, so `reachable_tools` could never
advertise `datasource.*`/`store.query`/`series.*`/`viz.query`/`flows.*`/… regardless of caps or
persona; the `HOST_TOOLS` inventory had also drifted (whole dispatched families missing, stale
hand-copied coverage list). Fixed at the catalog layer (name-only rows ∩ `is_host_native` ∩ the
same per-verb `authorize_tool`), inventory completed, dispatch families now shared consts both the
dispatcher and the coverage test derive from. Persona resolve/narrow was proven innocent. Regression
`persona_menu_full_catalog_test`; debugging entry
[`debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md`](debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md);
session [`sessions/agent/persona-menu-full-catalog-session.md`](sessions/agent/persona-menu-full-catalog-session.md).
Follow-up (recorded): inventory rows carry no arg schema until a verb grows a palette descriptor.

---

**Just shipped (2026-07-05): insights — the durable data-finding record + adaptive notify.** The
one missing record type is in: a persisted, queryable data finding (`insight:{ws}:{id}` — severity,
origin provenance, dedup-keyed occurrence counting, `open → acked → resolved` lifecycle) raised by
any principal via `insight.*` MCP verbs, discovered through the tag graph, and surfaced on an
Insights page with the agent dock + `builtin.insights-analyst` persona. Three sub-features compose
onto it: **occurrences** (a per-insight transaction ring — last N firings, 2 KB-capped, `oseq`),
**subscriptions** (a member subscribes a channel to all / a rule / an identity / a tag facet / a
severity floor; fire-time re-checked stored principal; deny ⇒ dormant + owner note), and **notify**
(the anti-spam digest ladder — `L0 immediate → … → L4 monthly`; breakthroughs always deliver; ack
suppresses; one digest per `(sub, window)`; per-sub `throttle_override`/`muted`; per-member prefs
kill switch; durable reactor over the injected clock). New crate `lb-insights`; host `insight.*`
verbs (every verb over `POST /mcp/call`); `/insights…` REST + `/insights/events` SSE;
`ui/src/features/insights/`; `builtin.insights-analyst` grounded by `core.insights` (the seeded
SKILL.md). Domain-free (rule 10): core never learns "fraud"/"HVAC". **Tests (real store/bus/
gateway, rule 9):** ladder unit **10/10**, host integration **14/14** (per-verb cap-deny +
ws-isolation + dedup/ring/2 KB-reject/matcher/ladder/digest-idempotency/kill-switch), gateway routes
**4/4**, UI gateway **4/4**; `core_skills_test` 11/11 (`core.insights` seeds + resolves); `pnpm test`
631/631. Follow-ups (recorded, not gaps): the rhai handle
([`scope/insights/rule-raises-insight-scope.md`](scope/insights/rule-raises-insight-scope.md) —
a rule raises/acks/closes an insight inline; scoped 2026-07-09) + flow `insight` sink producer
doors; InsightDetail origin deep-link + typed body renderer; retention/purge. Scope
[`scope/insights/insights-scope.md`](scope/insights/insights-scope.md) (umbrella + 3 sub-scopes,
index at [`scope/insights/README.md`](scope/insights/README.md));
shipped [`public/insights/insights.md`](public/insights/insights.md); skill
[`skills/insights/SKILL.md`](skills/insights/SKILL.md); session
[`sessions/insights/insights-session.md`](sessions/insights/insights-session.md).

---

**Just shipped (2026-07-05): rules + workflow converged onto flows + the webhook source node.** Flows
is now the **one** automation spine — the rules engine and the GitHub "workflow" module folded in, and
every GitHub/coding-specific piece **hard-deleted** (never used in prod, no data to preserve). **(1)
Rules are flow nodes:** new `rules.eval` verb (flow message envelope in → `{output, findings, log}`
out, same cage + per-source caps as `rules.run`); the `rhai` node runs an inline rule, the new `rule`
node runs a **saved** rule by id. **(2) Engine guards:** a per-flow `concurrency` policy
(`skip`/`queue`/`restart`, default `queue`) enforced at every fire seam (cron + manual), and a per-node
`timeout_ms` that settles `err:"timeout"`. **(3) Workflow's generic machinery survives as
provider-free flow nodes/reactors:** the `approval` gate (parks the run until a reviewer resolves a
`needs:approval` item; the flow-approval reactor resumes on `Approved` / cancels on `Rejected`), the
outbox sink + relay reactor (`sink(target=outbox)` stages a must-deliver effect; `spawn_relay_reactors`
drives `relay_outbox` over a provider-free `Target` with retry/backoff/dead-letter — the generic
replacement for the deleted github-workflow driver), and the reactor directory. `Target` + `relay_outbox`
relocated to `outbox/`. **(4) Deleted:** `crates/host/src/workflow/*` (github/coding parts), the roles
`github-workflow`/`github-target`/`github-webhook`, `node/src/github.rs`, the gateway `workflow.rs`
route + `mcp:workflow.*` grants, the `workflow_*`/`github_bridge_*` tests. **(5) The webhook SOURCE
node:** a generic built-in `webhook` source (config `{webhook_id}`) fires a run per hit via a durable
series-event reactor over `webhook:{ws}:{id}` — the ONLY flow-facing inbound surface, **no Slack/GitHub
node, no provider name in any core crate** (rule 10). **Tests (real store/caps/jobs/ingest/reactors,
rule 9):** `rules_workflow_convergence_test.rs` **14/14** — incl. the mandatory capability-deny
(`rules_eval_denied_without_the_cap`) + workspace-isolation (ws-B reactor never touches a ws-A run/hit)
categories; frontend `FlowsCanvas.gateway.test.ts` asserts the picker lists `webhook`/`rule`/`approval`
from the real registry (`pnpm test` 631/631; `test:gateway` 13/13). `cargo build --workspace` +
`cargo fmt` clean; the rhai→`rules.eval` rewire + 3 new spine nodes + the `queue` concurrency default
rippled into existing flows tests (all fixed: flows_run/flipflop/multi_trigger/triggers/nodes/plc green).
**Pre-existing reds — NOT this session (verified on base master `4c733cd`):** `panel_test` (dashboard
"STALE" view), `agent_routed_test`, `proof_panel_test` (needs its wasm ext built). Scope
[`scope/flows/rules-workflow-convergence-scope.md`](scope/flows/rules-workflow-convergence-scope.md)
(open questions resolved); session
[`sessions/flows/rules-workflow-convergence-session.md`](sessions/flows/rules-workflow-convergence-session.md);
public [`public/flows/flows.md`](public/flows/flows.md). `scope/coding-workflow/*` **retired**.

**Just shipped (2026-07-08): the debug node + debug panel — Node-RED's debug sidebar over the shipped
plane.** Drop a built-in `debug` node on a wire, open the sidebar, watch messages stream past live.
**Motion-only (rule 3 made literal):** the node publishes each wire message onto a workspace-walled
**per-flow** Zenoh subject `flow_debug:{ws}:{flow}` (fire-and-forget, **no SurrealDB record, no new
table**); a late-attaching panel tails from attach (deltas-only, no replay). One new built-in `debug`
(`kind = sink`, runs inside `flows.run` — no new exec cap), one new live-feed verb
`flows.debug.watch {flow_id}` (cap `mcp:flows.debug.watch:call`) + gateway SSE route
`GET /flows/{id}/debug/stream`. Format (`auto|json|text|markdown`) resolved **host-side**; the canvas
panel renders JSON as a collapsible tree, text as `<pre>`, markdown via the shared `MarkdownView`,
auto-collapsing long values. A per-node sliding-window governor (default 50/s) caps a hot source and
flushes a `dropped: k` sentinel. The `catch`/`status`/`complete`/`link` pack stays deferred (sibling
scopes). **Tests (real store/bus/caps — rule 9):** `flows_debug_test.rs` **7/7** — incl. the
**motion-only regression** (no debug-log record written), **capability-deny** + **workspace-isolation**
(the mandatory categories), **late-attach deltas-only**, and the **publish governor**; `lb-flows`
unit 81 (the bumped `builtins_in_one_shape` at 33 nodes); UI `DebugValueView.test.tsx` 9 (format
dispatch + auto-collapse + dropped sentinel); UI `flowsDebug.gateway.test.ts` 2 (real gateway: the
`debug` node ships in the palette under `Observability`, a flow with one runs to terminal).
`cargo build --workspace` + `cargo fmt` clean (my files); `pnpm exec tsc --noEmit` clean; flows UI
tests 57/57. **Pre-existing branch reds — NOT this session:** a parallel session is mid-`Claims`
migration and left several test files + the `test_gateway` seed on the old shape; I made the one-line
fix to `test_gateway_seed.rs` so the gateway harness builds, the rest are theirs. Scope
[`scope/flows/debug-node-scope.md`](scope/flows/debug-node-scope.md) (open questions resolved);
session [`sessions/flows/debug-node-session.md`](sessions/flows/debug-node-session.md); public
[`public/flows/flows.md`](public/flows/flows.md) (new "debug node + panel" section); skill
[`skills/flows-debug/SKILL.md`](skills/flows-debug/SKILL.md).

**Also shipped (2026-07-05): webhooks — a first-class inbound-HTTP surface, keyed and mediated.** A
webhook is a **named, workspace-walled, credential-protected inbound endpoint** the platform owns
end to end: an admin creates one through the new admin routes (`/admin/webhooks` CRUD), the
platform exposes a stable URL `POST /hooks/{ws}/{id}`, and every authenticated hit becomes exactly
one ingest `Sample` on `webhook:{ws}:{id}`. Anything that subscribes to that series — a flow (a
`trigger` with `mode=event` watching it today; a dedicated `webhook` source node is a named
follow-up), a rule, a dashboard tile, a raw `series.read` — consumes it. The webhook service is a
**producer**, not a second store; the endpoint + credential + URL outlive any one flow.

**Two auth modes**, admin-selected per hook: `bearer` (reuses the apikey credential verbatim —
`apikey_create` mints a `lbk_{ws}.{keyid}.{secret}` scoped to `key:webhook:{id}` with one narrowed
cap `mcp:ingest.write:call`; the webhook row carries `bearer_key_id` so revoke/rotate reach the
linked apikey; the presented keyid must match it — a sibling key cannot impersonate the hook) and
`signature` (the caller signs the raw body with a shared secret using HMAC-SHA256, sends
`sha256=<hex>` in an admin-picked header — `X-Signature` by default; the shared secret lives in
`lb-secrets` at `webhook/{id}` under Workspace visibility, mediate-read by the host on verify; v1
ships the universal `hmac-sha256` shape only — no Slack/GitHub/Stripe node anywhere in core, rule
10). The route captures the **raw body before any JSON parse** (load-bearing — HMAC verify over the
exact received bytes; a re-serialized body breaks every real signature, the most-common-webhook-
integration bug, pinned by a body-tamper test). **The inbound route is the only
unauthenticated-by-session surface** — a third-party caller presents the hook's own credential, not
a JWT; every failure (unknown id / disabled / wrong-secret / cross-ws URL) collapses to the same
opaque `404` so the public route is not a webhook-id oracle; a revoked hook is `410 Gone`. Replies
`202 Accepted { id, series, seq }`.

**Tests (real gateway + real store + real caps + real ingest buffer, rule 9):** Rust host `webhook::*`
units **18/18** (HMAC verify over raw bytes incl. body-tamper + malformed + missing + whitespace;
shared-secret entropy; `parse_bearer_key_id` shape; record/view derivation; payload preservation
for JSON / non-JSON / empty / invalid-UTF8); gateway `webhook_routes_test.rs` **16/16** —
capability-deny (per management verb + the no-widening refusal when the creator lacks
`mcp:ingest.write:call`), workspace-isolation (cross-ws URL is opaque 404; ws-B list sees only ws-B;
a forged ws-mismatched bearer is refused), **bearer end-to-end** (create → POST with bearer → 202 →
`series.read` returns the sample — the round-trip headline), **signature end-to-end** (sign raw
body → POST → 202 → `series.read`; wrong-sig opaque 404; missing-header opaque 404;
**body-tamper-breaks-signature** — sign compact, post pretty-printed with same JSON value, must
NOT verify), **rotate** (old secret dead, new works), **revoke** (route 410s, no further samples),
and **no-secret-leak** (list/get dump JSON; asserts neither secret nor any
`secret`/`hash`/`bearer_key_id`/`secret_ref` field appears). Apikey regression `apikey_routes_test`
**8/8** still green. `cargo build --workspace` (incl. `--features lb-role-gateway/test-harness`) +
`cargo fmt` clean. Scope [`scope/ingest/webhooks-scope.md`](scope/ingest/webhooks-scope.md) (open
questions all resolved); session [`sessions/ingest/webhooks-session.md`](sessions/ingest/webhooks-session.md);
public [`public/ingest/webhooks.md`](public/ingest/webhooks.md). **Named follow-ups (not silent
gaps):** the admin-UI Webhooks wizard (backend ready, UI is the next slice); the flow `webhook`
source node (needs generic source-series templating in `flows/source.rs` — today a `trigger` with
`mode=event` + `series=webhook:{ws}:{id}` covers it); per-hook `seq` counter (`now_ms` is the v1
floor; same-ms dedups); multi-node revoke cache-bust broadcast (lazy expiry + local bust are the
security floor, both tested).

---

**Just shipped (2026-07-05): the agent dock — a persistent, page-context-aware AI side panel.** A
shell-mounted, resizable, **non-modal** right dock on every authenticated page (StatusBar launcher +
run pip, global `mod+j`, `Escape` closes + refocuses launcher, mobile auto-close). It **survives
navigation** (mounted beside `<Outlet/>` in `RoutedShell.tsx`; the page reflows) and is a **THIN CLIENT**
over three shipped pieces — **no new persistence/transport/agent plumbing**: (1) storage/history =
channels, one session per `dock-{user-slug}-{ulid}` channel (create-on-post; the `-` separator keeps the
id ONE capability segment so the member's `bus:chan/*:pub` grant covers it — the dotted form was a silent
403, see debugging); (2) the answer = the durable channel agent worker resolving the workspace's **active**
agent; (3) progress = the run-event SSE folded into **six honest states** (Sent→Working→Answering→Stalled→
Done→Error; degrades honestly to "no live deltas + durable answer" without `mcp:agent.watch:call`). Each
message captures router **page context** (`{surface, path, search}`, tenant-stripped) that the host
**fences into the run's goal as untrusted, client-reported context** (labelled block, **4 KB cap that
REJECTS oversize**, absent ⇒ byte-identical) on the ONE seam both agent doors reach (`invoke_via_runtime`)
— so the channel `kind:"agent"` payload and `POST /agent/invoke` fence identically. **The ONLY host
change is one additive optional `context` field** on the agent item payload + `InvokeRequest` — **no new
verb, cap, or table**; the host never knows the `dock-` prefix (the wall is caps, not the name). UI
feature `ui/src/features/agent-dock/` (one responsibility per file), built on `@nube/panel`'s **non-modal
primitives** (`useResizable`+`ResizeHandle`), NOT its modal `Panel`/`Sheet`.

**Tests (real gateway + store, rule 9):** Rust host `agent_page_context_test.rs` **3/3** (context fenced;
>4 KB rejected before any model call; absent byte-identical); gateway `agent_invoke_route_test.rs`
**5/5** (incl. the two new context accept/oversize cases); host units `agent::page_context` 5/5,
`channel::agent_job`/`payload` green; the 5 affected `invoke_via_runtime` caller tests **25/25** after
threading `context`. UI **gateway `AgentDock.gateway.test.tsx` 7/7** (create-on-post; Done via drain;
history-restore on remount; new-session mints a second; channels surface excludes `dock-*`; MANDATORY
capability-deny — no pub → 403 error state; MANDATORY workspace-isolation — ws-B can't read ws-A dock
history). UI **units 30/30** (dockId/pageContext/stall-timer/dockRunState/pendingRun). `cargo build
--workspace` + `cargo fmt` clean. **Pre-existing red NOT this slice:** `radius-scale.guard` flags a bare
`rounded` in another session's in-flight `TemplateSourceField.tsx`; `sqlSource.gateway`/`SystemView.gateway`
fail on clean master. Scope [`scope/frontend/agent-dock-scope.md`](scope/frontend/agent-dock-scope.md);
session [`sessions/frontend/agent-dock-session.md`](sessions/frontend/agent-dock-session.md); public
[`public/frontend/frontend.md`](public/frontend/frontend.md) ("The agent dock"); debug
[`debugging/frontend/dock-channel-id-dotted-cap-deny.md`](debugging/frontend/dock-channel-id-dotted-cap-deny.md).

**Follow-up shipped same day — run controls (stop / pause / resume) in the dock.** ONE new cap
`mcp:agent.control:call` (member-level, distinct from `agent.watch`) + ONE new route
`POST /runs/{job}/{op}` (`cancel|pause|resume`) — a thin authorized front door onto the shipped
run-job lifecycle (`lb_jobs`), **no new table**: stop=`cancel` (worker posts honest `run stopped`);
pause=`suspend` (loop honors it at the turn boundary via a new `is_paused` check → `RunFinish(Suspended)`,
transcript/cursor intact, worker posts nothing); resume=`unsuspend` + re-arm the channel enqueue job so
the reactor re-drives from the cursor under the original asker's authority. Host
`run_events/control.rs`, loop `agent/run.rs`+`step.rs`, worker lifecycle classification
(`channel/agent_worker.rs`), gateway `routes/run_control.rs`, UI `lib/channel/run.control.ts` +
Pause/Stop/Resume in `DockRunStatus`. **Tests:** host `run_control_test.rs` **6/6** (pause→resume→complete
lifecycle, stop, worker classification, MANDATORY cap-deny + ws-isolation), gateway
`run_control_route_test.rs` **5/5** (deny→opaque 403, unknown-op 400, ws-isolation), UI
`DockRunStatus.test.tsx` **5/5** + dock gateway now **9/9**; `channel_agent_worker`/`agent_watch` stay green.

---

**Just shipped (2026-07-04): Widgets Slice C — result-render coverage (`descriptor.result` on the
tabular tools).** Closes G1 of the widget umbrella: today only `reminder.list` declared a
`descriptor.result` render envelope; the remaining tabular host tools (`federation.query`,
`query.run`) now declare their own `result = table` envelope — so the channel CAN render them
descriptor-driven (the `kind:"rich_result"` → `ResponseView` → `WidgetView` path), the AI discovers
the render via `tools.catalog`, and Slice B's `dashboard.pin` can pin them with ZERO tool-specific
code in the pin path. **This is BACKEND CONFIG** — a `result:` field on each tool's descriptor, no
new verb / cap / table / WIT, no UI component. The headline: pinning `federation.query`'s NEW
`result` envelope mints a persisted `pin-federation-query` cell that reloads via `dashboard.get`
and renders through the real `WidgetView`, treating the tool id as opaque data (rule 10 — Slice B's
mint proven, re-asserted for the new envelope).

**Backend** — `rust/crates/host/src/federation/query.rs`: `query_result_render()` (the
`x-lb-render` envelope) + `query_descriptor()` now carries `result: Some(query_result_render())`.
The envelope is `{ v:2, view:"table", source:{tool:"federation.query", args:{}},
tools:["federation.query"] }` — same shape `reminder/descriptor.rs::list_render()` established; the
`source.tool` names the tool itself (the re-runnable read); the palette interpolates collected
`source`/`sql` into `source.args`; `viz::frame::result_to_rows` zips the verb's `{columns, rows}`
into named row objects. `rust/crates/host/src/query/descriptors.rs`: `run_result_render()` +
`run_descriptor()` carries the same envelope for `query.run` (carries `{id}` verbatim → an edit to
the saved query propagates to the dashboard). Per-tool descriptor unit tests assert each envelope's
shape.

**Why `agent.invoke` is deferred to Slice D** (recorded, not silently dropped): the agent's render
is streaming + nondeterministic (run feed → durable `agent_result`); a pinned cell that RE-RUNS the
agent on every dashboard load is semantically wrong (cost, changing data). Slice D snapshots the
agent's one-shot ANSWER as a `data`-backed envelope, pin THAT. The shipped `kind:"agent"` palette
route carries the streaming workflow a static descriptor cannot replace.

**Reframes rich-responses follow-up #5** (rendering ≠ routing): the RENDERING half — a tool's
answer can mount via `WidgetView` from a descriptor-declared envelope — is CLOSED for
`federation.query`/`query.run` (proven by the UI gateway test). The ROUTING half — the palette
emits a specific payload KIND per tool (`kind:"query"` for the async query-worker, `kind:"agent"`
for the streaming run) — is INTENTIONAL: those payload kinds carry workflow semantics a static
descriptor template cannot express. The palette routing branches STAY; the descriptor-driven
`kind:"rich_result"` path is NEWLY available for the new envelopes. Nothing is deleted.

**Tests (real gateway + store, rule 9):** Rust `widget_result_render_test.rs` **8/8** — the catalog
serves the new envelopes to a granted caller AND HIDES them when the tool cap is absent (the menu IS
the permission model, extended to the `result` envelope); the HEADLINE (pin `federation.query`'s
NEW `result` → reload → cell intact, ZERO federation-specific code in the pin path);
generic-over-tool-id (an arbitrary `__test__.*` mints); workspace-isolation (a ws-B principal can't
read ws-A's pinned cell); shell-vs-headless-path-parity; `query.run` envelope parity
(`pin-query-run`); idempotent re-pin (replacing, not duplicating). Per-tool descriptor units
(`federation::query::tests::query_descriptor_carries_the_table_render`,
`query::descriptors::tests::run_descriptor_carries_the_table_render`,
`query::descriptors::tests::save_and_compile_do_not_declare_a_render`). UI
`ResponseViewResultRender.gateway.test.tsx` **3/3** (real spawned gateway) — the HEADLINE (a
`federation.query` `rich_result` mounts through `ResponseView`, NOT `QueryCard` — the
`PinToDashboard` affordance is the structural marker) + `query.run` parity + an arbitrary
unknown-tool-id envelope also mounts (rule 10). Slice A `widget_catalog_test` 8/8 + Slice B
`widget_pin_test` 10/10 stay GREEN. `pnpm test` (unit) **561/561** green. `cargo build --workspace`
+ `cargo fmt` clean. **Pre-existing red surfaced (NOT this slice's):**
`CommandPalette.gateway.test.tsx` (6 cases) + `CommandPalette.agent.gateway.test.tsx` (2+ cases)
fail with `useTheme must be used within ThemeProvider` from in-flight motion/theme work in the tree
(Slice C touched none of that; the failing files fail identically in isolation, and the four sibling
gateway tests that mount `<MessageItem>` directly — including Slice C's new file — are green).
Logged at
[`debugging/frontend/channel-palette-gateway-useTheme-not-in-provider.md`](debugging/frontend/channel-palette-gateway-useTheme-not-in-provider.md).
Scope [`scope/widgets/result-render-coverage-scope.md`](scope/widgets/result-render-coverage-scope.md);
session [`sessions/widgets/result-render-coverage-session.md`](sessions/widgets/result-render-coverage-session.md);
public [`public/frontend/dashboard.md`](public/frontend/dashboard.md) (Slice C section); skill
[`skills/dashboard-widgets/SKILL.md`](skills/dashboard-widgets/SKILL.md) (§ "Which tools declare a
`result` render today"). **Next up:** Slice D (channel-origin AI authoring — response → widget →
preview → `dashboard.pin`) and Slice E (extension capability introspection).

**Just shipped (2026-07-04): Widgets Slice B — pin a tool result-render to a dashboard (`dashboard.pin`,
branch `master`).** The keystone for "widgets are system-wide": a GENERIC host-side path that takes ANY
`x-lb-render` envelope (a tool's `ToolDescriptor.result`, or a live channel `rich_result` body) and mints
a persisted `dashboard:{id}` cell via a new `dashboard.pin` verb. **The reminder widget
(`reminder.list`, which already declares a `result = table` render) is dashboard-addable with ZERO
reminder-specific code in the pin path** — the envelope is opaque data, the tool id never branched on
(rule 10). Closes G2 of the widget umbrella (`widget-platform-scope.md`). The umbrella's open question
("client-compose vs a server-side mint verb") is RESOLVED: a **server-side mint**, because the host is
the only boundary every writer crosses (Slice A's thesis applied to the persist path) — a headless
`POST /mcp/call` / external AI can pin a `result` envelope without a shell, and the envelope↔cell
mapping lives in ONE host function (not mirrored across web/RN/AI clients). The channel render path
(`ResponseView.buildCell`) is UNTOUCHED — `dashboard.pin` is the persist-time twin, host-side.

**Backend** `rust/crates/host/src/dashboard/pin.rs` (NEW): `dashboard_pin` + `mint_cell_from_envelope`
+ `slug` + `pin_descriptor`. The mint mirrors `ResponseView.buildCell` field-for-field so a pinned cell
renders identically to the channel response (the cross-surface fidelity invariant): `view`/`source`/
`action`/`options`/`fieldConfig` copied verbatim; the envelope's extra `tools[]` (row-control write
verbs) become hidden `sources[]` so `cellTools(cell)` covers `render.tools` (the bridge leash). Cell
`i = pin-{slug(source.tool||view)}` by pure string ops (rule 10) — idempotent: re-pinning the SAME
envelope REPLACES the cell (preserves layout via `existing: Option<&Cell>`); a different envelope
appends at `next_free_y`. Reuses the Slice A validation chain (`check_cells_bounds` →
`check_genui_cells` → `check_view_cells` → `validate_and_strip_refs`) → `write_dashboard` → hydrate.
Gated `mcp:dashboard.pin:call` (its own cap, distinct from `.save`; the `.pin` wildcard trap, same as
`.catalog`); owner-only-update on an existing dashboard. The `dashboard.pin` MCP dispatch arm + a
`POST /dashboards/{id}/pin` gateway route (uses `gw.now()` so the REST client passes no `now`); a
`pin_descriptor()` in `host_descriptors()` so `tools.catalog` lists it (an AI discovers it can pin).

**Frontend** (1) `ui/src/features/dashboard/views/table/RowControls.tsx` (NEW) — the shared
actions-column renderer, extracted from `ResponseTable`, now used by BOTH the dashboard `TablePanel`
(a pinned cell is fully interactive on the grid — enable switch + run-now + delete) and the channel
`ResponseTable` (a live response). One row-control renderer, two surfaces — the cross-surface fidelity
invariant. `TablePanel` renders the actions column when `options.rowControls` is present (`useMemo`
hoisted above the early returns). (2) `ui/src/features/channel/PinToDashboard.tsx` (NEW) — the
"Pin to dashboard" affordance mounted by `ResponseView` beside a rendered `rich_result`: picks a target
dashboard (from `dashboard.list` + a "New dashboard" option) and calls `pinDashboard` over the real
gateway. The client passes the ENVELOPE through; the host constructs the CELL (no cell construction in
the client). (3) `pinDashboard` in `dashboard.api.ts` + `dashboard_pin` in `http.ts`.

**Tests (real gateway + real store, rule 9):** Rust `widget_pin_test.rs` **10/10** — capability-deny
(opaque) + plain-member happy path (proves the grant, not an admin bypass) + non-owner deny +
workspace-isolation + the HEADLINE (pin `reminder.list`'s declared `result` → reload → cell intact) +
generic-over-tool-id (an arbitrary `__test__.frobnicate` mints a valid cell) + idempotent-re-pin-replaces
+ different-envelope-appends + shell-vs-headless-path-parity (the SAME cell from `dashboard_pin` and
`call_tool` → `dashboard.pin`) + Slice A view-validator fires through the pin (`view:"heatmap"` rejected)
+ pin coexists with hand-authored cells. UI `PinToDashboard.gateway.test.tsx` **4/4** (real spawned
gateway) — the HEADLINE (pin a reminder.list rich_result via the UI affordance → reload via
`dashboard.get` → render the cell through the real `WidgetView`/`TablePanel` → reminder rows AND row
controls visible) + capability-deny (a session without `mcp:dashboard.pin:call` refused at the host) +
workspace-isolation + fidelity/idempotency. `pnpm test` (unit) **547/547** green; reminders-palette
gateway (uses `ResponseTable`/`RowControls`) **11/11** + DashboardView gateway (uses
`TablePanel`/`WidgetView`) **11/11** — no regression from the shared `RowControls` extraction.
`cargo build --workspace` + `cargo fmt` clean. **Pre-existing red surfaced (NOT this slice's):**
`panel_test` 4 cases fail with `unknown view 'STALE'` — Slice A's `check_view_cells` rejects the
panel-test fixtures' placeholder echoed spec before ref-stripping; `git log` confirms `panel_test.rs`
was last touched at the Slice A commit. Logged at
[`debugging/widgets/panel-test-stale-view-preexisting-slice-a-red.md`](debugging/widgets/panel-test-stale-view-preexisting-slice-a-red.md)
(a Slice A follow-up: fixtures should use a real-but-different view like `gauge` vs `stat` so the
validator passes AND the hydration-overwrite intent is preserved). Scope
[`scope/widgets/pin-to-dashboard-scope.md`](scope/widgets/pin-to-dashboard-scope.md); session
[`sessions/widgets/pin-to-dashboard-session.md`](sessions/widgets/pin-to-dashboard-session.md); public
[`public/frontend/dashboard.md`](public/frontend/dashboard.md) (Slice B section); skill
[`skills/dashboard-widgets/SKILL.md`](skills/dashboard-widgets/SKILL.md) (§ "Pin a tool result").
**Next up (now):** Slice D (channel-origin AI authoring — response → widget → preview →
`dashboard.pin`) and Slice E (extension capability introspection).

**Just shipped (2026-07-04): Dashboards viewer mode — editing is admin-only (branch `master`).**
The Dashboards surface (`ui/src/features/dashboard/`) now has two role-decided postures: an **admin**
(workspace-admin, `isAdmin(caps)`) gets the full authoring surface (roster + create/rename/delete,
drag/resize, per-cell edit/delete, add-panel, variable editor); a **viewer** (any member without an
admin cap) reads the live grid with **no authoring surface at all** — the roster is gone, the grid is
static, no edit/delete/add. **The fix:** `DashboardView` gated `canEdit` on `mcp:dashboard.save:call`,
which is *member-level* (every member holds it → everyone was an editor); it now gates on `isAdmin`
(the `ADMIN_SECTION_CAPS` workspace-admin signal) and threads that one boolean to `DashboardRoster`
(rendered only when true) + `Grid.editable`. **No new server cap, no server change** — the gate is
defense-in-depth; the gateway still re-checks `dashboard.save`/`.delete` per verb. **Tests (real
gateway, rule 9):** `DashboardView.gateway.test.tsx` **11/11** — new VIEWER (no authoring surface),
ADMIN (full surface), and the mandatory VIEWER-DENY (a viewer token rejected on save/delete
server-side); `DashboardRoster.test.tsx` **5/5**. Scope
`scope/frontend/dashboard-viewer-mode-scope.md`; session
`sessions/frontend/dashboard-viewer-mode-session.md`; public `public/frontend/dashboard.md` (viewer-mode
section). Pre-existing out-of-scope tsc reds untouched (accordion, flows, transformDebug — none in
touched files).

**Previously shipped (2026-07-04): Data Studio v3 — one stacked query/preview view (branch `master`).**
v2's explore-tab + builder-tab split is collapsed into ONE **stacked builder tab**: picking a source (or
opening a Library panel, or New panel) opens a single tab with the live **preview on top** and the
**Query/option surface on the bottom** — the user sees the data and shapes the chart together; opening an
existing chart puts the chart in focus with its source beneath it. **The move:** `BuilderPane` gained a
`layout: "split" | "stacked"` prop (default `split`, backward-compatible for the dashboard-parity tests);
Data Studio mounts it `stacked`. The read-only `explore` tab-kind + `ExplorePane` are **retired** (the
builder's own preview + table-view toggle + viz picker cover data inspection); `openExplore` now seeds a
chart draft and opens a builder directly. The **SQL editor** was surfaced, not rebuilt — `QueryTab`
already renders the Builder⇄Code `SqlQueryEditor` for a Direct-SurrealDB source (raw SQL for federation,
the friendly picker for series/flows); v3 proves it shows in the stacked bottom section. **No new
host/verb/cap/table** — pure UI recomposition; layout persistence (the `layout.*` verbs) unchanged.
**Tests (real infra, rule 9):** UI unit **443/443**; `DataStudio.gateway.test.tsx` **6/6** (pick→stacked
builder→save-as-library round-trip + layout persists + SQL-editor-when-needed + open-existing-from-library
+ member-owned + ws-isolation + panel.save deny); panel-builder gateway parity + `DashboardView.gateway`
removal regression **13/13** (default `split` layout, unchanged). Pre-existing/out-of-scope reds untouched
(missing-WASM github-bridge/proof-panel; `sqlSource` casing; a concurrent `CodeEditor.tsx` edit polluting
`SystemView`). Scope `scope/frontend/data-studio-scope.md` (v3 section); session
`sessions/frontend/data-studio-v3-stacked-view-session.md`; public `public/frontend/data-studio.md`
updated. **Next up:** promote `panel-kit` to a `packages/@nube/panel-kit` workspace lib once the
`@/lib/dashboard` type graph is extracted.

**Also shipped (2026-07-04): rules approval loop — a rule proposes, a human disposes.** A rule body can
now `inbox.request_approval(#{ id, channel, body, route, on_approve })`: it raises a `needs:approval`
item AND stages the `on_approve` effect in a new **`held`** outbox status — the relay skips a held
effect, so it is never delivered until approved. A reviewer `inbox.resolve(id, "approved")`s the item,
and a generic **approval-release reactor** (a node-boot tick beside the flow/agent reactors) releases
the held effect (`held → pending`, the relay then delivers it exactly once), discards it on reject
(`held → discarded`), or leaves it held on defer. Reuses the shipped `Item` + `Resolution` + outbox
trio — no new primitive, no new cap (`request_approval` gates on `outbox.enqueue` + `inbox.record`; the
release is a system transition, so no token can force it). Reactor is **domain-free** (keys on
`(resolution, held-effect-id)`, coding path untouched — rule 10). **Tests (real infra, rule 9):**
`lb-outbox` **9/9** (held never schedulable; release/discard guarded + idempotent); `lb-host`
`approval_release_test` **8/8** (gated release delivers once; reject/defer; cap-deny; no forced release;
ws-isolation; durable re-scan); `lb-rules` `approvals_test` **4/4** (effect-first two writes; opaque
deny, no partial write); `RulesApprovals.gateway.test.tsx` **4/4** (full loop through the real spawned
gateway + reactor tick). Scope `scope/rules/rules-approvals-scope.md` (SHIPPED); session
`sessions/rules/rules-approvals-session.md`; public `public/rules/rules.md` + `skills/rules/SKILL.md`
(§5 propose-and-approve) updated. **Next up (out of scope):** typed item facet if a second tag consumer
appears; enforced reviewer routing (a policy scope).

**React Native mobile app (`docs/scope/app/`, workshop `app/`): SHELL SLICE SHIPPED (2026-07-04);
next up: app-extensions.** The RN host (`app/shell/` — RN 0.86.0 / React 19.2.3 / Re.Pack 5.2.5,
MF2 host with `react`/`react-native`/`@nube/app-sdk` as shared singletons; standalone install, see
`.npmrc`) + the shared client in `app/sdk/` (`invoke` verb→route 1:1 with the web `http.ts`, typed
`InvokeError` 403/401, token-per-workspace `SessionStore` over a keychain/memory `SessionStorage`
seam, streaming-fetch SSE with reconnect + `channel.history` catch-up). Login → workspace switcher →
Channels end to end (REST + live SSE + kill/resume) → cap-gated `ext.list` nav (list only; mount is
the next slice). Tested against the REAL spawned `test_gateway` (`app/sdk$ pnpm test:gateway`,
**17/17**): session/switch, channels+SSE, **deny-per-verb**, **workspace isolation**, ext nav, the
**client-singleton regression** (login-established session stays observable on the one client —
the "stuck on login" fix), and the **restore-liveness regression** (a rehydrated-but-invalid session
is dropped to login, not shown empty). A **browser preview** (`make -C app dev` → react-native-web on
5310 against its own throwaway `test_gateway` on **8087**, off the root node's 8080) renders the real
App.tsx; prefilled ada/acme signs straight through to Channels. The preview node is in-memory, so a
restart re-keys tokens — the shell's **validated restore** (`client.restore()` probes the node and
drops a dead session) now falls to the login screen instead of a confusing empty channel list
(`debugging/app/stale-preview-session-shows-empty.md`). Scope:
`app-shell-scope.md` (transport decided there: **REST + SSE via the gateway**, zenoh-ts rejected);
sessions `sessions/app/app-shell-session.md` + `sessions/app/app-preview-login-session.md` +
`sessions/app/app-preview-stale-session-session.md`; public `public/app/app.md`. Remaining asks:
`app-extensions-scope.md` (MF2 remotes, `[app]` manifest block, reference exts `proof-panel-app` +
`channel-chat`), then `app-sdk-scope.md` (extract the full web verb map as the authored source).

**App — Expo bare-modules adoption (2026-07-04): MODULE SYSTEM WIRED + proof-of-life ported; on-device
build deferred.** Adopted Expo's **bare** native-module system (**SDK 57** — the SDK whose
`bundledNativeModules.json` pins `react-native@0.86.0`, the shell's exact RN) **without** giving up
Re.Pack + Module Federation (managed workflow / Metro rejected). `expo`+`expo-secure-store` installed
standalone (repo React-18 workspace + lockfile untouched); native wiring in `android/settings.gradle`
+ `MainApplication.kt` and `ios/Podfile` + `AppDelegate.swift` links expo modules while every
JS-bundling touchpoint stays on Re.Pack (moduleName `LazybonesShell`, bundle root `index`). Did **not**
use `install-expo-modules` (its release maps only to SDK 56/RN 0.85); transcribed the module-linking
subset from `expo prebuild`'s SDK-57 output run in an isolated scratch copy, never letting prebuild own
the tree. Proof-of-life: the session-token store now uses **`expo-secure-store`** behind the unchanged
`SessionStorage` seam (`react-native-keychain` kept dormant until device parity). Runnable checks green:
`app/sdk$ pnpm test:gateway` **17/17** (incl. deny + ws-isolation — the gateway seam is undisturbed),
shell `tsc` clean, web-preview `vite build` resolves the `expo-secure-store` alias. **Deferred (no
device toolchain here):** native Gradle/Cocoapods build, on-device secure-store smoke, EAS Build — the
device slice. Scope `app-expo-scope.md`; session `sessions/app/app-expo-session.md`; public
`public/app/app.md` (Expo section).

**Previously shipped (2026-07-04): Data Studio v2 — the multi-pane data workbench + the extracted headless
`panel-kit` lib.** Data Studio (`/t/$ws/data-studio`) is rebuilt from v1's
single-picker/single-preview/modal into a **dockable multi-pane workbench** on `flexlayout-react` (ISC):
N explore tabs + N panel-builder tabs open side by side; drag to split/tab/dock/float/close/rename; the
whole arrangement (incl. each tab's draft cell) persists **per user** in SurrealDB. **The architectural
move:** the panel-editing logic was lifted out of the dashboard's editor into `ui/src/lib/panel-kit/` — a
**headless** lib (no JSX / `@/components` / `@/features`; only `@/lib/*` + `@nube/genui`; package-shaped
for a later `@nube/panel-kit`): `cellToEditorState`/`editorStateToCell` (the ONE spec (de)serializer),
`usePanelEditor` (the state machine), `defaultCell` (per-view options now injected), the SQL builder model
+ `toSurrealQL`, `draftFromSelection`, `saveDraftAsPanel`, `useGenUiAuthor`. The option-surface views moved
to `features/panel-builder/` (+ a new inline `BuilderPane`, no modal); Data Studio's FlexLayout panes
(`features/data-studio/`) are its own views on that logic — a third consumer can reuse the logic with
different views. **Panel authoring was REMOVED from the dashboard** (deleted `AddPanel`/`EditCellButton`/
the modal `PanelEditor`/the dead `WidgetBuilder`); the dashboard now only PLACES library panels (ref
cells) + renders. **Layout persistence** is a new member-owned verb pair `layout.get`/`layout.set` over
`ui_layout:[ws, user, surface]` (the `nav_pref` pattern generalized; the surface key is opaque, rule 10;
keyed to the token `sub`, bounded 256 KB) — `crates/host/src/layout/`, gateway `GET/PUT /layout/{surface}`,
member caps in `credentials.rs`, UI client `lib/layout/`. **Tests (real infra, rule 9):** UI unit
**437/437**; `DataStudio.gateway.test.tsx` **4/4** (explore→build→save-as-library round-trip + layout
persists + member-owned + ws-isolation + panel.save deny); `DashboardView.gateway.test.tsx` **8/8**
(removal regression + library placement); the moved editor gateway tests re-target `BuilderPane`; Rust
`layout_test.rs` **6/6** + `layout_routes_test.rs` **3/3**, existing nav/panel/gateway/session green.
Pre-existing/out-of-scope reds untouched (missing-WASM fixtures github-bridge/proof-panel; `sqlSource`
casing assertion). Scope `scope/frontend/data-studio-scope.md` (v2, promoted); session
`sessions/frontend/data-studio-v2-workbench-session.md`; public `public/frontend/data-studio.md`.


**Just shipped (2026-07-03): job/flow-run retention + indexed drain scan — the CPU-burn fix (branch
`master`).** A long-lived node pegged a full CPU core re-scanning its own `job` table: the reactors'
drain scan (`lb_jobs::pending`) walked every page and filtered in Rust, and terminal `job` /
`flow_run` / `flow_step_output` rows accumulated forever (`debugging/jobs/node-pegs-cpu-reactor-
rescans-job-table.md`). **Two fixes.** (1) **Indexed drain** — `pending` is now one
`SELECT data FROM job WHERE data.kind=$kind AND data.status IN ['running','suspended']` backed by
`DEFINE INDEX job_kind_status` (`crates/jobs/src/schema.rs`, ensured lazily per-namespace on first
`create`) — O(pending), not O(table); strictly safer than the paged walk on the first-page property.
(2) **Bounded retention** — count-bounded per ws (default 500), delete predicate `status IN (terminal)`
and nothing else so a resumable job/run is never trimmed: `crates/jobs/src/retain.rs` (`retain_terminal`,
`job`) and `crates/host/src/flows/retain_runs.rs` (`retain_runs`, `flow_run` + `flow_step_output` purged
in tandem), swept on the flow reactor tick throttled to every 30th tick (`flows/retention_sweep.rs`).
Config is a compiled caller-owned default (no numeric prefs axis exists). `make purge-store` added for
immediate dev relief. **Tests (real `mem://` store, rule 9):** `crates/jobs/tests/retain_test.rs` **4/4**
(perf@5k terminal + 2 resumable returns only the resumable / index-backed count / never-trim-resumable /
newest-kept / ws-isolation) + `crates/host/tests/flows_retention_test.rs` **2/2** (live-run-safe + step
tandem, ws-isolation); `pending_test` **2/2** still green. `cargo build --workspace` + `cargo fmt` clean;
`cargo test --workspace` green except the pre-existing missing-WASM-fixture suites (github-bridge,
webhook, proof-panel, agent_routed — unrelated). No UI touched. Scope `scope/jobs/job-retention-scope.md`
(open questions resolved); session `sessions/jobs/job-retention-session.md`; public `public/jobs/jobs.md`.

**Just shipped (2026-07-03): library panels — panels as their own reusable + standalone asset (branch
`master`).** A chart is now a first-class asset: a `panel:{id}` record holding the **non-layout half of
a v3 `Cell`** (the spec), cloned from the `dashboard` asset (slug id, owner, `private|team|workspace`
visibility, S4 `share` edge, tombstone, cap-gated verbs). (1) **Backend** `rust/crates/host/src/panel/`:
the six verbs `panel.get|list|save|delete|share|usage` (each own file + cap `mcp:panel.<verb>:call`),
plus the two host-side ref seams — `hydrate_cells` (`dashboard.get` expands each `panelRef` ref cell →
resolved v3 cell under the VIEWER's gates; dangling/unreadable → an honest `panel_missing` placeholder,
never a leaked spec) and `validate_and_strip_refs` (`dashboard.save` validates each ref resolves
in-workspace — loud `BadInput` — and strips the echoed spec, so the ref is authoritative). Additive
`panel_ref`/`panel_vars`/`panel_missing` on `Cell` (inline + ref cells coexist). Delete-safety refuses a
delete-in-use with the usage list unless `force`. (2) **Gateway** `routes/panel.rs` (6 routes) + the caps
in the dev-login set. (3) **UI** `lib/panel/` (types + api + the `Panel↔Cell` bridge), `panel_*` in
`http.ts`, editor affordances (`LibraryPanelBar`: Save-as-library / "used on N dashboards" banner /
Save-to-library / Unlink; `AddLibraryPanel` picker), the `panel_missing` placeholder in `WidgetHost`, and
the standalone page `features/panel/PanelPage.tsx` at `/t/$ws/panel/$id` (reuses `WidgetHost`/
`usePanelData`/the viz bridge — no parallel renderer — cap-gated on `panel.get`). **A panel is a LENS
over data access, never a grant** — sharing shares the DEFINITION; its `sources[]` re-check under the
viewer's caps at render. Scope `scope/frontend/dashboard/library-panels-scope.md` (open questions: None);
session `sessions/frontend/library-panels-session.md`; public `public/frontend/dashboard.md` → "Library
panels"; skill `docs/skills/panels/SKILL.md`. **Tests:** backend `panel_test.rs` **9/9** (the "sharing
never widens data access" + cross-ws `panel_ref` no-hydrate headlines, per-verb deny, ws-isolation,
coexistence, propagation, delete-safety); UI `PanelPage.gateway.test.tsx` **8/8** real gateway; `pnpm
test` **430**; dashboard + nav gateway suites green; `cargo build --workspace`/`fmt` clean. Nav-builder
skill also written this session at `docs/skills/nav/SKILL.md`.

**Just shipped (2026-07-03): the nav builder — user-/team-authored navigation over pages (branch
`master`).** A workspace-scoped `nav` **asset** (cloned from the `dashboard` pattern) whose ordered
`items[]` link to core surfaces, dashboards, extension pages, or dynamic tag-groups — the menu is a
**lens over existing access, never a grant path**. Built end to end: a new `rust/crates/host/src/nav/`
module (the `nav`/`nav_pref`/`workspace_nav_default` records + the verbs), the full MCP surface
(`nav.get/list/save/delete/share/set_default/resolve` + `nav.pref.get/set`, each its own cap, wired
store → `call_nav_tool` → gateway `/navs`·`/nav/resolve`·`/nav/default`·`/nav/pref` → `http.ts` →
`ui/src/lib/nav`), the composite **`nav.resolve`** (pick: personal→team→ws-default→built-in `SURFACES`
fallback; tag-expand via `tags.find`; **cap-strip** every unreachable item), and the UI (NavRail renders
`nav.resolve` with a `SURFACES` fallback, route gates untouched; a **Nav builder tab** under the access
console picking from the three real sources). The nav grants nothing — the server re-checks every verb on
click. Tests (rule 9, real store/node/gateway): **11 Rust** (`nav_test.rs` — CRUD, per-verb deny,
ws-isolation, gate-3 non-member deny, the **"nav never widens"** headline, precedence, tag-group
dynamism, member-owned pref) + **8 UI** (`NavRail.test.tsx` 4 + `NavAdmin.gateway.test.tsx` 4 real-
gateway). `cargo build --workspace` + `cargo fmt` clean; `pnpm test` **430**. Scope
`scope/nav/nav-builder-scope.md` (open questions resolved); public `public/nav/nav.md`; session
`sessions/nav/nav-builder-session.md`. **Deferred (named):** `skills/nav/SKILL.md`, dashboard deep-board
links, extension-authored navs. **Pre-existing unrelated fails (clean-tree):** `SystemView.gateway`,
`sqlSource.gateway`, `agent_routed_test`.

**Previously shipped (2026-07-03): extension widgets over any source — frames-in + `echarts-panel` reference
(branch `master`).** An extension `[[widget]]` is now a **first-class view over the v3 panel model**: a
`[[widget]]` declaring `data = true` opts into frames-in, so an `ext:<id>/<widget>` cell carries the same
`sources[]` + `fieldConfig` + `transformations[]` as a built-in `timeseries`, and the **shell** resolves
them through the shipped `viz.query` path under the **viewer's** grant and hands the tile **resolved
frames** (`ctx.data`) — the tile renders, never fetches, needs no read caps. (1) **Manifest**: `data`
bool through `ext-loader`/`assets`/`ui_decl` → client `ExtUi.data` → source-picker `SourceEntry.data`.
(2) **ctx v3** (additive, all THREE contract mirrors moved together — host `federationWidget.ts`, the new
devkit `src_contract.ts.tmpl`, the extension copy): `v:3`, `data: WidgetFrame[]`, `fieldConfig`, and a
`{ update?, teardown? }` return so a data/vars/range tick re-renders **in place** (the hard-won ExtWidget
StrictMode per-run-slot lifecycle preserved; a v2 tile is byte-identical). (3) **`useVizFrames`** shares
`useVizQuery`'s bridge/interpolation/`vizQueryKey` → one round-trip per spec, no per-tile duplicate
stream; resilient to a missing cache provider (a v2 tile mounts standalone). (4) **Editor**: a `data`
widget KEEPS its `sources[]` + shows Query/Field tabs (the widget is the view, the source its binding); a
bare v2 widget clears targets (unchanged). (5) **`echarts-panel`** (`rust/extensions/echarts-panel`, cloned
from proof-panel): a `data = true` "Chart" widget rendering `ctx.data` with Apache ECharts, driven by the
Field-tab options, `{ update }` for live re-render. Security unchanged (per-target deny → honest empty
frame, workspace-walled; zero new ext caps). ONE render path: the same cell mounts through `WidgetView`
from a dashboard AND a channel `rich_result`. Scope
`scope/frontend/dashboard/ext-widget-source-binding-scope.md` (open questions resolved); session
`sessions/frontend/ext-widget-frames-in-session.md`; public `public/frontend/dashboard.md` → "frames-in";
debug `debugging/frontend/ext-widget-standalone-mount-throws-no-dashboard-cache-provider.md`. Tests:
`cargo build --workspace` clean, `lb-host`/`lb-ext-loader`/echarts backend green; UI `pnpm test` **426**;
new `builder/framesIn.gateway.test.tsx` **8/8** real-gateway (deny · ws-isolation · v2-compat ·
frames-resolution · data-flag projection · dashboard+channel parity); echarts `framesToOption` **7/7**.
**Deferred:** Part C (`@nube/widget` package extraction) + the `useSceneDocs` `scene:` cleanup rider (own
slices); a dedicated `series.watch`-into-`update` live streaming test. **Blocked (env, not code):**
`make publish-ext EXT=echarts-panel` packs+signs the wasm but the running node's dev-login 403/401'd
(`missing bearer credential`) — likely a stale node (`make kill && make dev`); the palette entry is proven
by the gateway data-flag test.

**Previously shipped (2026-07-03): dashboard read cache & call de-duplication (branch `master`).**
One `@tanstack/react-query` cache, scoped to the dashboard visit (`features/dashboard/cache/`,
`DashboardCacheProvider` mounted by `DashboardView` + channel `ResponseView`), collapses the burst of
redundant reads the surface fired. `viz.query` is keyed on the **canonical resolved spec** (not whole-panel
JSON) → the editor's probe/preview/plot share one round-trip and a title/layout edit no longer refetches;
the source-picker bundle + `datasource.list` are one shared fetch per ws (pure `loadSourcePicker` extracted
in `@nube/source-picker`); N cells on one flow → one `flows.node_state` (client-side slicing); `series.read`
backfill cached, live SSE tail left outside the cache (state vs motion). Keys are ws-prefixed + canonicalised.
Real-gateway `queryCache.gateway.test.tsx` asserts the de-dup counts (viz.query=1 across 3 cells,
node_state=1 across 2), workspace-isolation, and the deny path (instrumented on the `invoke` seam). SSE
subscriber-sharing + a `<SourcePicker>` component consolidation are noted follow-ups. Scope
`scope/frontend/dashboard-query-cache-scope.md` (open questions resolved); session
`sessions/frontend/dashboard-query-cache-session.md`; public `public/frontend/dashboard.md` → "Read cache".
Tests: UI `pnpm test` **426** (+ the new gateway file); `test:gateway` dashboard/channel suites green (two
unrelated pre-existing flakes: SystemView bus-peer-count, sqlSource e2e). **Next up:** SSE subscriber-sharing.

**Just shipped (2026-07-03): viz panel-editor parity — Phase 3.5, all 7 steps (branch `master`).**
Closed the gap between the shipped viz spine and a *usable* editor: a user can now build every
editor-supported panel end to end **without ever seeing JSON, a free-typed property id, or a field name
they must remember and retype** (the phase exit gate, MET). (1) **Primitives + FieldNamePicker** — a
searchable `Combobox`/`Checkbox`/`ColorSwatchPicker` and a field picker fed by the live preview's REAL
`viz.query` result fields; every "no shadcn primitive yet" suppression burned down. (2) **Option
registry** (`editor/options/`) — one `OptionDef` per option, the Field tab renders entirely from it via
one `Control`; value-mappings/color-scheme/data-links finally have editors; searchable unit picker;
thresholds mode toggle. (3) **Typed editors for all 11 transform ids** — headline **Organize fields** is
now a row list (reorder/hide/rename) over the real result fields, not a JSON textarea; filterByValue
condition rows, groupBy per-field rows, calculateField operands. (4) **Overrides on the registry** —
matcher pickers + "add property" over the registry + typed control per property + multi-property;
aligned `byRegex`→`byRegexp` to the backend. (5) **Per-viz options to parity** — table
width/align/cell-type/filter/footer, timeseries stacking/threshold-display, stat/gauge/bargauge/pie value
options. (6) **Multi-target queries** — A/B/C rows (add/dup/delete/hide/reorder) + query-options row +
preview table-view toggle. (7) **Per-step transform debug** — the one additive backend flag
(`lb-viz::transform_stepwise` + `viz.query` `panel.debug`/`stopAt` → `steps[]`, same cap, no new verb)
surfaced as a Transform-tab "Show per-step result". The registry-driven round-trip test iterates the
WHOLE registry (a new option can't dodge it) and usability gates are tests (author a mapping/organize/
override through the UI, no JSON). Scope `scope/frontend/dashboard/viz/editor-parity-scope.md`; session
`sessions/frontend/dashboard-editor-parity-build-session.md`; debug
`debugging/frontend/panel-editor-tab-label-stale-after-navmenu.md`. Tests: UI `pnpm test` **422**,
`cargo test -p lb-viz` **53** (+4 stepwise), `cargo build --workspace` clean; the editor-parity gateway
tests (fieldNamePicker/valueMapping/organize/overrides/queryTargets/transformDebug) green. **Next up:**
backend clamp for the new `queryOptions` (maxDataPoints/minInterval/relativeTime — authored + on the wire
now, resolver honoring is the follow-up); BarChart per-viz options; then Phase 4 (import/export).

**Just shipped (2026-07-05): agent-personas #1 (persona-model) — the run's *focus* as data (branch
`master`).** A persona `{ identity, granted_tools, grounding_skills, extends, policy_preset?, runtimes? }`
is a workspace-selected focus that **narrows** a run (advertised tools + pinned skills + identity),
never widening the wall (`persona ∩ agent ∩ caller`, every dispatch re-checked). Two tiers, one shape —
the `agent.def` catalog pattern, fourth reuse: built-ins seeded read-only into reserved `_lb_personas`
from `personas.toml`; custom personas are workspace CRUD. (1) **Record + 5 CRUD verbs**
(`agent.persona.{list,get,create,update,delete}`, one file each under `agent/personas/`) + `resolve`
(extends-unioned effective persona) + `agent.policy.get` (the policy pane's read). (2) **Selection:**
additive `agent.config.active_persona` + a per-invoke `persona` override threaded through **every** front
door (channel payload → `ChannelAgentJob` → worker; routed `AgentInvokeRequest`/`invoke_remote`/`serve`;
`POST /agent/invoke`). (3) **Application (`apply.rs`, the ONE seam both runtimes share):** narrow the
`RunContext.tools` menu (glob = trailing-`*` prefix; = the external ACP bridge's advertised set),
fold identity + pinned-skill bodies into the goal (**fail-closed** — an ungranted pin fails the run at
start, before model spend), filter the advertised catalog to the pinned set (new
`render_catalog_filtered` + `RunContext.persona_catalog`), enforce the #4 runtime restriction.
**Design call (caught by a failing test):** run-assembly persona resolution is a **raw namespace-walled
read**, NOT gated on the picker cap — a persona read can only narrow, so gating guards nothing while
breaking the common member case (the persona analog of "menu is a hint, wall is the law"). Both-runtimes
**swap + narrowing tests green** (in-house via a recording model; external via a scripted `AgentRuntime`
capturing its `RunContext`), plus caps-deny/ws-isolation/precedence/extends-cycle — **19 host tests
green**, existing agent suite + `node --features external-agent` no-regression. Docs:
[`public/agent-personas/agent-personas.md`](public/agent-personas/agent-personas.md) ·
[`sessions/agent-personas/persona-model-session.md`](sessions/agent-personas/persona-model-session.md) ·
`skills/agent/SKILL.md` §7. **Settings UI SHIPPED** (persona pane + Allow/Ask/Deny policy pane over
`agent.policy.get`/`set` + read-only effective-tools view; 6 gateway tests green). **#2 grounding corpus
SHIPPED (2026-07-05):** the `lb-assets` build script now scans the **whole** `docs/skills/` tree
dynamically **plus** the `docs/testing/**` e2e runbooks (seeding as `core.e2e-*`), with a **hard anti-rot
build failure** if a skills dir lacks its `SKILL.md`; 3 new grounding skills authored + embedded
(`core.mcp`, `core.acp`, `core.extension-authoring`) → **corpus 24→34**. The **grounding exit-gate proof**
is green (a persona-grounded run is fed its pinned `core.e2e-backend` runbook body, menu stays focused,
no repo/fs tool). Session:
[`sessions/agent-personas/persona-grounding-session.md`](sessions/agent-personas/persona-grounding-session.md).
**#3 built-in catalog SHIPPED (2026-07-05):** the seven built-in personas as `personas.toml` data
(data-analyst, flow-author, widget-builder, rules-author [extends flow+data], workspace-admin,
channels-operator, system-manager [extends all six]) — verb-lists cross-verified against the live
inventory (155 verbs, zero missing) + skill corpus (34, zero broken pins); destructive verbs excluded
from every persona; **8 catalog tests green** incl. the **confusion demo** (reachable palette 11→1 under
data-analyst) + ws-isolation + caps-deny + `extends` composition. **Recorded finding (not coded around):**
the menu a persona narrows is the palette-descriptor catalog (`host_descriptors()` ∩ caps) + extension
tools, NOT the full ~175-verb surface — the persona lists are the forward-looking allow-list; on a bare
node identity+grounding carry most of the confusion cure (see `persona-catalog-scope.md` finding).
Session: [`sessions/agent-personas/persona-catalog-session.md`](sessions/agent-personas/persona-catalog-session.md).
**#4 extension-builder SHIPPED (2026-07-05) — the topic is COMPLETE (all 4 sub-scopes, all 5 umbrella
gate bullets met).** `builtin.extension-builder` ("100% coding, but never on its own"): the devkit
surface (admin-tier) + `core.extension-authoring` pins + a **safety posture** — a `policy_preset`
Ask-floor on node-mutating verbs (a real run proposing `ext.publish` **durably suspends** for a human
`agent.decide`) and a `runtimes:["default"]` in-house-only restriction (external pairing → named error).
The one code addition: `clamp_to_preset` (the floor is a CLAMP over the evaluated effect, NOT a merged
rule — an appended Ask can't beat a blanket Allow under Deny>Allow>Ask; loosening needs an explicit
per-tool rule) + `check_runtime`, threaded via `RunContext.persona_preset`. **10 coding tests green**
(caps-deny, Ask-floor, floor-not-loosened-by-blanket-Allow, runtime restriction, ws-iso, adversarial
devkit input, + a real-scaffold e2e + the "never on its own" suspend-e2e). Two bugs caught by tests
(the clamp-vs-merge floor + a TOML sub-table binding drop of `runtimes`). Session:
[`sessions/agent-personas/persona-coding-session.md`](sessions/agent-personas/persona-coding-session.md).
Public: [`public/agent-personas/agent-personas.md`](public/agent-personas/agent-personas.md).

**#5 persona-session SHIPPED (2026-07-05) — the post-ship correction of #1's selection model.** Live
use showed #1's single `agent.config.active_persona` workspace-wide toggle was wrong twice over:
two members (or one member's two tabs) can't hold different focuses, and hand-picking a focus
workspace-wide is backwards when the dock already knows where the user is. This slice replaces the
toggle with three ideas, **zero new verbs**: (a) the workspace **enables a roster**
(`agent.config.enabled_personas: Option<Vec<String>>`; None = all enabled — the curation layer);
(b) exactly **one** persona per run, **suggested client-side** from the page surface via new
`Persona.surfaces: Vec<String>` (matched over the enabled roster by the dock — rule 10, no core
branch), with a sticky per-tab **pin** in `sessionStorage`; (c) defaults re-homed to a new nullable
`Prefs.agent_persona` axis (member → ws-default fold). Run assembly (`apply.rs`/`resolve_effective`)
untouched; one-shot boot migration copies any legacy `active_persona` into the ws-default axis
(decode-only thereafter). Backend: **9 host tests green** (precedence table, roster semantics,
disabled-named-error, ws-isolation, independence, migration, surfaces-as-data, capability-deny);
UI: **8 PersonaSettings + 6 DockPersonaChip gateway tests green** (chip == sent payload for context
match + pin; pin survives remount; pin in tab A never changes tab B; disabled absent from picker;
explicit-disabled invoke error; second member's own server fold). Decisions: empty roster = cleared
= all enabled (disabling-all unsupported); `""` clears a prefs axis; multi-match = id-sorted first;
list computes `enabled` server-side. Session:
[`sessions/agent-personas/persona-session-session.md`](sessions/agent-personas/persona-session-session.md).


**Just shipped (2026-07-03): active-agent wiring — the active pick is the ONE implicit agent
everywhere (branch `master`).** A workspace picks one agent and no surface asks again; the pick's
`model_endpoint` is now consumed **per workspace**, and the missing primitive under it — a real
OpenAI-compatible provider adapter — is live. (1) **Adapter:** `openai_compat.rs` (`Provider`, OpenAI
chat-completions) with an honest-failure contract; `node/src/agent.rs::adapter_for` maps
`zaicoding`/`openai`/`openai-compat` to it (both the node fallback AND the per-ws builder route through
it). (2) **Per-workspace model:** `resolve_active_definition` (promoted from `agent.def.test`) +
`resolve_workspace_model` — active def → endpoint → key (sealed-ws-secret → env, host-mediated) → a
model built by a host-owned **`ModelBuilder` seam** the binary installs (rule 1: host never
build-deps the role crate — a **deviation from the scope's "host builds it directly"**), memoized in a
`DashMap<(ws, endpoint-hash)>` on the `Node`, **invalidated on `agent.config.set`**. Additive
`active_definition` field on `agent.config`. (3) **Rules** ride it (`resolve_rule_model` →
`resolve_workspace_model`; unconfigured workspace keeps the honest `DisabledModel`). (4) **In-house
loop** rides it via a per-run `RunContext.model_override` at run start (node env stays the fallback
tier). (5) **Channels** (already shipped in `72b0651` — verified: `RuntimeArg` "Active — <label>" omits
`runtime`; `agent.runtimes.workspace_default`) + **widget transport** (`POST /agent/invoke` →
`invoke_via_runtime(runtime=None)`, `http.ts`, `desktop.rs` — also already shipped, verified). UI
`pick()` now writes `active_definition`. **Tests (rule 9, real store/caps/loop/rules; scripted provider
HTTP the only fake):** new `agent_active_model_test` (6 — picked-endpoint→model · **ws-ISOLATION** ·
cache invalidation on re-pick · loop rides the per-ws model · sealed-secret→env **key precedence** ·
`active_definition` LWW) + `node` adapter unit (2) + shipped `agent_invoke_route_test` (happy/**DENY**
/ws-iso) + `rules_ai_wiring_test`/`agent_in_house_wiring_test`/config/runtimes suites green. Build
(default + `--features external-agent`) + fmt clean. **Close-out (2026-07-03, WASM built):** full
`cargo test --workspace` green (0 failed); `pnpm test` 424/424; agent gateway specs 15/15 + ProofPanel
13/13. One regression the full run surfaced — the per-run `model_override` shadowed a runtime's
*registered* model for a workspace with no pick (`agent_routed_test`) — **fixed** (`dispatch.rs`: override
only with a configured model) + debug entry
[`debugging/agent/model-override-shadows-registered-runtime-model.md`](debugging/agent/model-override-shadows-registered-runtime-model.md).
See
[`sessions/agent/active-agent-wiring-session.md`](sessions/agent/active-agent-wiring-session.md) ·
[`public/agent/agent.md`](public/agent/agent.md) ("The active pick is the ONE implicit agent
everywhere") · [`skills/agent/SKILL.md`](skills/agent/SKILL.md) §6 · scope open-questions all resolved.

**Earlier on 2026-07-03: default agent wiring — the in-house `"default"` agent finished (branch
`master`).** The always-registered in-house loop now (1) binds a **real model** from node config
(`LB_AGENT_MODEL_*`, the `ModelEndpoint` shape — provider/model/api-key-env NAME/base-url) built as
`AiGateway<Provider>` and installed via `install_runtimes`; **no model → the honest `UnconfiguredModel`**
(config-only, symmetric). (2) The **load-bearing fix**: the loop's proposed tool calls
(`agent/step.rs::run_calls`) now route through the ONE host MCP bridge **`call_tool`** (the same entry
`POST /mcp/call` uses) instead of registry-only `lb_mcp::call` — so the loop reaches **host-native**
verbs (`agent.memory.*`/`assets.*`/`series.*`/…) AND extension tools under the identical
`authorize_tool` + caps wall (`agent ∩ caller`); `skill.activate` stays the loop-internal built-in.
This same dispatch serves the **external** agent (both fronts platform-capable, one path). Threaded
`&Node → &Arc<Node>` along the agent path (the trait `AgentRuntime::run` + `AcpRuntime` + test stubs)
so `call_tool`'s `Arc` reaches the loop; every call site already held an `Arc<Node>`. (3) The loop's
`AllowedTool` menu is populated from the caller's **reachable `tools.catalog`** (new
`agent/menu.rs::reachable_tools`), replacing the channel worker's empty `&[]`. (4) **Boot**: a new thin
`node/src/agent.rs` mount (like `control_engine.rs`) builds the registry (in-house default + external
entries when the feature is on) and calls `serve_agent`, mounted after the gateway key install —
closing the serve-wiring TODO. **Provider adapter status (stated plainly):** no real `Provider` exists
yet (only `MockProvider`; real adapters are ai-gateway-deferred) — the deliverable is the real wiring
seam + config + the **unconfigured→configured swap**, proven for real against `AiGateway<MockProvider>`;
a real adapter drops into `build_in_house_model` with no other change. **Tests (rule 9, real
store/bus/caps/gateway/loop; MockProvider the only fake):** `agent_in_house_wiring_test` (8: the
headline — the loop EXECUTES a host-native `agent.memory.set` through `call_tool`, was `NotFound` ·
**capability-DENY** (intersection lacks the cap → denied, fed back, nothing persists) · **workspace-
ISOLATION** (ws-B can't reach ws-A memory) · unconfigured→configured swap · menu = reachable catalog ·
**external-agent parity** + its deny · **offline/sync** resume re-drives a host-native call cleanly).
Full agent suite green (16 test bins) — no double-gate/deny/skill.activate regression. `cargo build
--workspace` (default + `--features external-agent`) + `cargo fmt` clean. See
[`public/agent/agent.md`](public/agent/agent.md) ("The finished in-house default") ·
[`sessions/agent/default-agent-wiring-session.md`](sessions/agent/default-agent-wiring-session.md) ·
[`skills/agent/SKILL.md`](skills/agent/SKILL.md). **Done (2026-07-03, active-agent-wiring):** the real
`Provider` adapter (`openai_compat`) landed → the in-house agent answers with a real LLM (see the
active-agent-wiring entry above).

**Just shipped (2026-07-03): catalog "Test" button + DB-sealed per-workspace model key.** Two gated
additions to the shipped agent catalog, reusing shipped seams: `agent.def.test {id?}`
(`crates/host/src/agent/defs/test.rs`, gated `mcp:agent.def.test:call`) — a **context-proving
diagnostic** that assembles the caller's real run context (system prompt + `reachable_tools` +
`render_catalog`) and runs ONE turn, returning `{ answer, runtime, model, context: {tools, skills},
provider_configured, ok }` so an admin sees the agent *was given* its MCP/ACP/skill context (the
context line is the proof pre-adapter; `provider_configured` is honest — the test node runs
`UnconfiguredModel`); and a names-only **`api_key_secret`** endpoint field resolved secret → env by the
ONE shared `resolve_endpoint_key` (`crates/host/src/agent/resolve_key.rs`), with the UI's write-only
"Model key" field sealing the value through the shipped `secret.set` (the record carries only the
path). **Tests (rule 9):** `agent_def_test_test` (10 — deny · context-proof · inherits-the-wall ·
names-only · resolution precedence · ws-isolation of the key · built-in read-only · bounded ·
`provider_configured` honest) + `AgentCatalogTestAndKey.gateway.test.tsx` (3, real spawned gateway).
Build (default + `--features external-agent`) + fmt clean. See
[`sessions/agent/agent-catalog-test-and-secrets-session.md`](sessions/agent/agent-catalog-test-and-secrets-session.md)
· [`public/agent/agent.md`](public/agent/agent.md) · [`skills/agent/SKILL.md`](skills/agent/SKILL.md).

---

**Just shipped (2026-07-03): GenUI — AI-authored dashboard widgets over one generative-UI layer
(branch `ce-node-wiring-v2`).** A dashboard widget the workspace agent designs from a natural-language
prompt, rendered live from a persisted, versioned IR — **no model in the render path**. **New package**
`@nube/genui` (standard `packages/*` layout): a versioned, **A2UI-*shaped* IR** (flat id-referenced
component map, JSON-Pointer `{$bind}` data model, typed patch messages) with pure ops
(`resolveBindings`/`applyPatch`/`validate`/`migrate`) + a **`defineCatalog`** whose one source
generates the render fns, the prompt signature block, AND the A2UI-style catalog JSON. **Two strata on
two entries** so a viewer never bundles the parser: render (`@nube/genui`, ~24 KB, parser-free) vs
authoring (`@nube/genui/authoring` — the ONE place the single new external dep `@openuidev/lang-core`
loads, for the OpenUI-Lang→IR adapter + streaming + the normalize sloppiness pass). **Parse once,
persist the IR**: accept runs parse→normalize→validate→size-check ONCE (≤8 KB), the typed IR persists,
raw Lang never renders. **A2UI patterns adopted, Google's packages rejected** (no A2UI adapter in v1).
The **`view:"genui"`** dashboard tenant: `GenUiView` mounts `<GenUiSurface>` (data via the shipped
`usePanelData` per v3 `sources[]` target keyed `/data/{refId}`; actions over the `cellTools` leash,
host-re-checked); the builder's "AI widget" tab drives `agent.invoke`(skill `core.genui-widget`)→run
stream→live preview→accept→`dashboard.save`. **Trust tier amended to in-process** (flagged + approved):
the shipped `WidgetIframe` sandbox can't host React (no import map, CSP `connect-src 'none'`, eval'd
engines — the `ext-widget-iframe-tier-cannot-resolve-bare-react` wall); genui widgets are
admin-authored (the `dashboard.save` cap is the trust gate), the IR is trusted DATA satisfying the 5
promotion-checklist items (CI-tested), so it renders in-process (the promotion end-state). **One host
change** (Decision 6): a validation branch inside `dashboard.save` for `view:"genui"` cells (IR `v`
known, ≤8 KB, every component in the embedded generated `genui_catalog.json`) — no new verb/cap/table,
so headless MCP authors get the same loud rejection. A **generated skill** (`skill:core.genui-widget`,
auto-embedded/seeded) whose catalog block + the host's catalog JSON are produced by `pnpm --filter
@nube/genui gen:skill`, CI freshness-gated. **Tests (rule 9):** package unit ×42 (Lang↔IR round-trips +
streaming, op purity + migration goldens, normalize, accept rejections, catalog-compat gate, prompt/JSON
goldens, the promotion checklist, gen:skill freshness); host ×8 (accept/reject matrix + **capability-
DENY** + **workspace-ISOLATION**); UI unit (data helpers, empty-source v3 trap); gateway integration ×4
(real node: save→reload→**render-without-adapter**, save-time rejection, empty-source v3 round-trip,
save-cap deny). One bug fixed + regression (`debugging/genui/genui-probe-setstate-in-render.md`). Node
binary builds (skill embeds); `cargo fmt` clean. Deferred (named triggers): A2UI JSONL adapter, the
channel `view:"genui"` tenant, IR patch-line refine, the design-time sampling policy knob. See
[`public/genui/genui.md`](public/genui/genui.md) · [`sessions/genui/genui-widget-session.md`](sessions/genui/genui-widget-session.md).

**Just shipped (2026-07-03): agent memory — durable, access-walled (branch `ce-node-wiring-v2`).**
The workspace agent's **learned** knowledge (skills are the *taught* half), in the MEMORY.md shape:
many small fact records behind capability-checked MCP verbs, read/written under the derived principal
(`caller ∩ agent`). **New module** `crates/host/src/agent/memory/` (one verb per file): a SCHEMAFULL
`agent_memory` table keyed `{ws, scope, slug}` (composite id `[scope, slug]` → idempotent offline
upsert, LWW); two scopes `workspace` + `member:{user}` where **the member scope is derived from the
authenticated principal, never an argument** — a run under U resolves `workspace + member:U`,
structurally never `member:V` (the member wall). **Four verbs** `agent.memory.list|get|set|delete`,
one `mcp:agent.memory.<verb>:call` cap each, PLUS a distinct **workspace-scope write gate**
(`store:agent_memory/workspace:write` — a member always curates their own member memory; writing
SHARED memory needs the extra cap). **Derived index** (list-computed, never stored) injected at
session start into **both runtimes** AFTER the persona + skill catalog, framed as *recalled
background, not instructions*, under an **on-behalf-of** principal (the caller's sub so member scope =
the human, agent's intersected caps so it never widens — the `substrate.rs` contract; a naive derived
`agent:session` sub would miss the caller's own memory, caught by the injection test). **Bounds** (desc
≤ 120, body ≤ 8 KB) + a **best-effort secret lint** (PEM/`AKIA…`/`sk-…`/GitHub/`password:` refused).
Injection capped at the 100 most-recently-updated (older stay stored/listable — evict from injection
only). In-house gets the verbs by default; external profiles opt in to `set`. Model-proposed
`set`/`delete` mid-loop is a **named deferral** (the channel worker surfaces no tool list + the loop
dispatches via `lb_mcp::call`, which doesn't reach host-native `agent.*` — the shared in-house-tool-
surfacing gap). **Tests (rule 9):** `host/agent_memory_test` (8: per-verb deny · ws-write gate distinct
· workspace isolation · **member wall (bob never sees member:ada)** · idempotent upsert · bounds +
secret lint · MCP roundtrip + per-verb MCP deny · **real run injects the index after set / loses it
after delete**). `cargo fmt` clean. Session
[`agent-memory`](sessions/agent-memory/agent-memory-session.md); public
[`agent-memory`](public/agent-memory/agent-memory.md); skill
[`agent-memory`](skills/agent-memory/SKILL.md). **Next up:** surface `agent.memory.*` as
model-proposable in-house tools; vector/semantic recall (v1 non-goal).

---

**Just shipped (2026-07-03): core skills — the two-tier skill catalog (branch `ce-node-wiring-v2`).**
The developer-authored **core skill tier** alongside the shipped S4 user tier, both behind the SAME
grant gate and the SAME `load_skill`. **Embed + seed:** a `lb-assets` build script embeds the
**whole** `docs/skills/*/SKILL.md` corpus (dynamically scanned — every dir with a SKILL.md, no
allow-list; **plus** the `docs/testing/**` e2e runbooks as `core.e2e-*`, agent-personas #2) into the
node binary at build time — parsing/stripping frontmatter, flagging repo-relative links — and `node`
boot seeds immutable `skill:core.<name>@<node-version>` records into a reserved system namespace
(`_lb_skills`, the `_lb_identity` precedent; one constant + one resolver file, boot seeder is the only
writer). A skills subdir missing its `SKILL.md` now **fails the build** (the anti-rot gate,
persona-grounding #2). Idempotent re-seed; a node upgrade seeds new versions, keeps old for rollback.
**Read-only to users:** `put_skill`/`deprecate_skill` reject any
`core.*` id regardless of caps (a non-opaque `Reserved`→`BadInput`, checked before the caps gate).
**New verb** `assets.deprecate_skill` (soft delete via a `skill_meta:{id}` flag — hidden from
list/latest, pinned load still resolves, a new version un-hides). **`list_skills`** gains `{tier,
description, latest, granted}` rows (the one agent catalog); wired `list_skills`/`deprecate_skill`/
`revoke_skill` into MCP dispatch. **Default grant set** (`core.lb-cli`/`core.query`/`core.store-read`)
applied at workspace creation (node config `LB_DEFAULT_CORE_SKILLS`), revocable like any grant — NO
grant bypass for core. **Catalog injection both runtimes:** the in-house loop keeps its once-per-run
inject; the external ACP runtime folds the granted catalog into the goal (its only channel), under the
derived principal (`caller ∩ agent`); persona unified onto the same `load_skill` loader. **One real fix
to the shipped path:** the caps grammar splits a resource on `/` AND `.`, so a dotted core id
under-matched `store:skill/*` — the dev-login + grants now use `store:skill/**`
([debug](debugging/auth/skill-star-cap-misses-dotted-core-id.md)). **Tests (rule 9, real store/loop/
gateway):** `assets/core_skill_seed_test` (3) + `host/core_skills_test` (11: core.* put/deprecate
rejected even for admin · ungranted-core deny · empty-catalog-without-read-cap · tier rows · deprecate
hide/pin/un-hide · default grants at creation · ws isolation · **real-run catalog injection tracks
grant→grow/revoke→shrink**) + `host/core_skills_mcp_test` (4: per-verb MCP deny + tier rows over the
bridge). Also fixed a pre-existing red (`flows_nodes_test` `BUILTINS` missing `flipflop`). `cargo fmt`
clean. Session [`core-skills`](sessions/skills/core-skills-session.md); public
[`skills`](public/skills/skills.md); skill [`skills`](skills/skills/SKILL.md). **Next up:** durable
**agent-memory** (the sibling "make the agent smarter" scope) — the same enforcement thesis.

---

**Just shipped (2026-07-02): control-engine v1 — node integration + a generic auth fix.** The shipped
CE extension is now **installed, cap-reachable, and driven end to end against a real gateway** (not just
green in tests). Three parts: (1) **boot-install** — `rust/node/src/control_engine.rs` (mirroring
`federation.rs`) installs + supervises the CE sidecar env-gated by `LB_CONTROL_ENGINE_BASE`, approves its
`net:tcp` connect, and seeds one appliance; `make dev` builds+wires it **opt-in** via `CE=1` /
`CE_BASE=host:port` (OFF by default — needs a running ce-studio). (2) **Extension tools reach users via
GRANTS, not a login hardcode** — the real fix for the page's `out_of_scope`/`denied`: `install_native`
grants each `[ui]`/`[[widget]]` scope tool (∩ granted) to `role:workspace-admin` (new
`crates/host/src/authz/grant_ui.rs`), `resolve_subject_caps` folds a role subject's direct grants, and
login merges `resolve_caps` into the token — so installing ANY extension makes its page reachable by
admins with **zero login edits**, durable + revocable (`authz-grants-scope.md` realized end to end). (3)
**A native-callback boot-ordering fix** — native roles now mount AFTER `Gateway::new_live` installs the
shared signing key, so a sidecar's `LB_EXT_TOKEN` verifies on callback (was a silent 401; `federation`
benefits too). Proof: dev token carries the CE caps resolved from the store, `appliance.add`/`list`
succeed through the real callback (200, was 401/403). Session
[`ce-v1-node-integration`](sessions/control-engine/ce-v1-node-integration-session.md); debugging
[auth](debugging/auth/ext-page-caps-require-grant-not-login-hardcode.md) ·
[callback-key](debugging/extensions/sidecar-callback-401-key-minted-before-gateway.md). Branch
`ce-node-wiring`. **Next up:** promote the grant-on-install behavior to `public/auth-caps/` once a second
extension exercises it; wire the same `grant_ui_scope_to_admin` call into the wasm `install` path.

---

**Shipped (2026-07-02): control-engine v1 — slice S7 (`BridgeTransport` + federated wiresheet page,
branch `ce-v1`).** The LB-authored UI half: a federated remote under `rust/extensions/control-engine/ui/`
mounts the vendored `@nube/ce-wiresheet` `CeEditor` wired to a `BridgeTransport implements EngineTransport`.
The vendored package is UNTOUCHED — the transport is injected. **Request half:** a table maps the
wiresheet's `/api/v0` REST paths → `control-engine.*` tools (tree/schema/add-node/patch/set+clear-override/
add-edge/remove-node/call-action), translating each body to the verb's arg shape (keyed node `{uid,kind}`,
appliance always injected); an unmapped path throws a LOUD error naming it (never a silent 404 — the signal
a follow-up verb like `set-layout`/undo/group is owed). **Stream half:** `openStream` arms
`control-engine.watch {appliance}` → `bridge.watch('series.watch', {series})`; each S6 frame (`frames.ts`)
decodes into the editor's `DecodedFrame`/`TopologyMsg` (the `>2^53`-as-string → bigint rule mirrored). No
`bridge.watch` (Tauri/tests) → static canvas, no throw. **Page:** appliance picker (`appliance.list`) +
empty-state `appliance.add` flow; the `[ui] scope` lists exactly the verbs the canvas emits, so a read-only
grant yields a read-only canvas (bridge narrowing + host re-check). **v1 gaps (absent-not-broken):**
presence hidden, per-actor undo engine-shared, drag-position not persisted (no `set-layout` yet).
**`@nube/ce-wiresheet` resolution:** the ext UI is standalone (own lockfile, `--ignore-workspace`, like
proof-panel) and resolves the vendored package by vite ALIAS to its built `dist/` (react external in both
builds → one React); `build.sh` builds the vendored dist first, then the remote. **Seam gap (no vendored
edit):** the vendored `index.ts` doesn't re-export the `DecodedFrame`/`TopologyMsg`/… types or the wire-tag
constants — recovered via `Parameters<StreamHandlers[...]>` + fixed protocol literals; a clean re-export is
a tracked upstream S1 follow-up. **Tests:** 27 co-located vitest UNIT tests green (frame decode vectors ·
full request-map coverage + loud-unmapped · arg translation · openStream cov→onFrame + no-watch degrade ·
mount picker + empty state) + `vite build` (`dist/remoteEntry.js`) + the vendored `build:lib` + `tsc`
clean. The live end-to-end path is proven MANUALLY against a live `ce-studio` (the `test:gateway` harness
has no SSE/native-sidecar transport — thecrew/proof-panel punt live SSE to Playwright), not in the harness.
Session [`ce-v1-s7`](sessions/control-engine/ce-v1-s7-session.md). **Next up:** S8 (e2e hardening + ship) —
and the deferred verbs the unmapped-path errors name (`set-layout`, undo/redo, group/copy).

---

**Shipped earlier (2026-07-02): control-engine v1 — slice S6 (`control-engine.watch` live COV feed, branch
`ce-v1`).** `control-engine.watch {appliance, scope?}` → `{series, subject}` surfaces CE's change-of-value
stream as a workspace-scoped live feed. The sidecar re-encodes each already-decoded `rubix-ce` `CovEvent`
to a **plumbing-agnostic JSON frame** (`{kind:"cov", ts, values:[{uid,v}], status?}` /
`{kind:"topology", ts, msg}`; integers past `2^53` → strings for JS-bigint safety) and pumps it onto a
deterministic series (`ce-cov:{appliance}:{scope-hash}`) via the host `ingest.write` callback — the shipped
`series` motion + gateway `GET /series/{series}/stream` SSE is the live read S7 opens. Shipped the
**zero-core-change fallback** per the slice's sequencing decision (the generic extension-watch primitive is
not in core), behind the same tool name + frame contract so S7 is plumbing-agnostic (swap tracked as a
named follow-up). **Lifecycle:** arm-on-first / disarm-on-last per series (in-memory pump refcount, not
durable state); `appliance.remove` force-disarms a live watch; the pump reconnects on a CE WS drop with
bounded backoff (a gap, not a dead stream). **One generic core fix** (CE-ignorant): the MCP `ingest.write`
verb now publishes live motion after its durable write, matching the `POST /ingest` HTTP route — it was
durable-only, so a sidecar-written sample never reached the SSE. **Tests (real infra):** frame + series
units (9) · watch lifecycle units (3: arm/disarm via the instrumented `ce_fake`, appliance.remove disarm,
WS-drop reconnect) · integration (3: **exit gate** arm→frame→motion on the real series subject · deny ·
ws-B-cannot-watch-ws-A isolation). Sanity-grep clean; `cargo fmt` clean. Real-engine COV tier left opt-in
(ce-studio not run here). Exit gate **MET**. Session
[`ce-v1-s6`](sessions/control-engine/ce-v1-s6-session.md). **Next up:** S7 wiresheet bridge over the frame.

---

**Shipped earlier (2026-07-02): control-engine v1 — slice S5 (graph WRITE verbs, branch `ce-v1`).**
The seven v1 command verbs, each a thin caps-gated map onto ONE `ControlEngine` trait method, working
local AND routed: `control-engine.add-node` · `.patch` · `.set-override` · `.clear-override` · `.add-edge`
· `.remove-node` · `.call-action` (one file per verb under `src/tools/`). Node identity on the wire is
the **keyed** form (`{uid, kind?, path?}`) — a write MUST address a concrete node (new `NodeKeyArg` +
`require_node_key` in `args.rs`; no root fallback, unlike reads). `remove-node` returns the soft-deleted
UIDs (CE's 24h-undo handle S8's `restore` consumes). **Caps:** each verb has its own gate
`mcp:control-engine.<verb>:call`, added to the manifest `[[tools]]` AND `request` (so the grant carries
them); **no new store/net/secret cap** — the writes reach CE over the already-granted `net:tcp` socket.
**Self-check first (defense-in-depth):** each write verb calls `host.require("control-engine.<verb>")?`
before resolve/parse/trait-call (the inbound `native.call` carries no caller identity), alongside the
host-side `authorize_tool` on the routed boundary. `serve.rs` gained a minimal write-dispatch arm
(`is_write_verb` → `HostCtx::grant_only_from_env` → `dispatch_write`) — reads stay as-is (S6 edits
serve.rs in parallel). **Open question resolved:** the optional `{session?, actor?}` attribution is
**deferred** — the pinned `ce-client-rust` (`51ab97e`) exposes no per-call header/session/actor hook, so
the "LB principal → CE actor" mapping is a later follow-up (no client API invented). **Tests (real infra,
fake-backed CI gate):** CE units (8: per-verb self-check→deny/counter + missing-node validation) + host
`control_engine` (deny + happy write matrix across all 7 verbs) + host `control_engine_appliance_routing`
(**routed `ce.patch` via the appliance record** + **offline write fails loud, nothing queued**) +
opt-in `#[ignore]`d real-engine write flow (add-node/patch/remove-node/tree **proven against live
ce-studio `:7979`**; add-edge + call-action best-effort due to documented `ce-client-rust`↔engine decode
quirks, NOT S5 mapping faults). `cargo fmt` + sanity-grep **clean** (no CE strings in core src). Exit
gate **MET**. Session [`ce-v1-s5`](sessions/control-engine/ce-v1-s5-session.md). **Next up:** S6
(`control-engine.watch` COV live feed) / S7 (the `@nube/ce-wiresheet` federated page over `BridgeTransport`).

---

**Shipped earlier (2026-07-02): control-engine v1 — slice S4 (appliance registry + routed hop, branch
`ce-v1`).** S4 makes `appliance` a real, workspace-walled concept and proves the symmetric-nodes claim
with two real in-process nodes. **Three layers:** (1) a **generic core** change — native Tier-2 sidecars
are now first-class in the ONE MCP routing registry (a `LocalDispatch` trait in `lb_runtime`, a host
`SidecarDispatch` adapter, `install_native` registers the native ext), so `resolve`/`dispatch`/
`serve_call` treat wasm and native uniformly and a native ext is reachable over the **cross-node routed
hop** with no per-tier branch and no CE strings in core. This corrects the kickoff's "no core change"
premise: routing was generic, but *serving* (`serve_call`) was wasm-only, so a native ext was unreachable
cross-node (Open Question #1, resolved as a generic router fix). (2) the **generic `store.write`/
`store.delete`** host-native MCP verbs, gated per-table by the `store:<table>:<action>` grammar (the write
half of the direct-store contract; the CE registry is the first caller, core stays `ce_appliance`-ignorant).
(3) the **CE `ce_appliance` registry**: the record model + `store.*`-callback CRUD (via the adopted
`lb-sidecar-client` `HostCtx`, the ROS idiom with a per-verb cap self-check), the `control-engine.appliance.
add|list|remove` verbs, and `resolve.rs` (selector → CE base; empty → local, known → its base, unknown/
other-ws → **not-found** — the isolation wall; no-gateway dev tier → literal base). **Tests (real infra,
real seed):** generic `store_mutate` (4) + `native_routing` (1, routed native hop) + CE
`appliance_registry` (6: CRUD · resolve · deny-per-verb · **ws-A invisible to ws-B** · stateless) + host
`control_engine_appliance_routing` (1: **two-node routed `ce.tree` via the appliance record** + **offline
fail-loud, nothing queued**) + CE units (3). Regressions green (`cross_node_routing`, `control_engine`
S3 + hot-restart). Real-engine tier **green against live ce-studio `:7979`** (a no-gateway regression
found + fixed, [debug](debugging/extensions/ce-tree-fails-without-gateway-real-engine-tier.md)).
`cargo fmt` + sanity-grep clean. Exit gate **MET**. Session
[`ce-v1-s4`](sessions/control-engine/ce-v1-s4-session.md) ·
[core routing](sessions/control-engine/native-routing-registry-session.md). **Next up:** S5 write verbs.

---

**Shipped earlier (2026-07-02): control-engine v1 — slices S1 + S3 (branch `ce-v1`).** The CE
bridge's first two slices, built in parallel. **S1 (the `EngineTransport` seam)** was cut
**upstream** in `NubeIO/ce-wiresheet` (branch `lb-transport`, local — not pushed): a pure
refactor that lifts the editor's hardwired transport (`rest.ts` `fetch` + `ws.ts` socket)
behind one `EngineTransport`/`EngineStream` interface, with `DirectTransport` reproducing
today's direct-to-CE behavior verbatim and `CeEditor` gaining an optional `transport?` prop —
so the S7 LB MCP/Zenoh bridge becomes an *injection*, not a fork. Exit gate met: a
`MockTransport` vitest renders a tree + applies a frame with **zero `fetch`/`WebSocket`
globals touched**; `pnpm typecheck` + **145** tests + both vite builds green. **S3 (the sidecar
+ local read verbs)** stood up `rust/extensions/control-engine/` (native Tier-2, mirroring
`federation`): the stdio control loop, `Arc<dyn ControlEngine>` per appliance via the pinned
`rubix-ce` git dep, and `control-engine.tree` + `control-engine.schema` serving **verbatim**
engine DTOs behind the caps gate (the tool NAME is the gate, no CE knowledge in core — sanity
grep clean). The ONE sanctioned fake (`ce_fake`, behind the `ce-fake` feature, armed by
`LB_CE_FAKE=1`) lets the host test drive the **real** supervisor + gate + stdio ABI; the
opt-in real-engine tier ran **green against a live ce-studio engine on `:7979`**. Tests: crate
unit **6** (dispatch deny-before-call + verbatim DTOs), host `control_engine_test` (capability-
deny opaque · happy tree/schema · hot-restart `restart_count==1`), `cargo build --workspace`
green. Workspace-isolation for the verbs is deferred to **S4** (needs the appliance registry —
noted, not faked). Open questions resolved in both slice docs (DTO=verbatim; port=7979). Slices
[S1](../rust/extensions/control-engine/docs/slice-1-wiresheet-transport-seam.md) /
[S3](../rust/extensions/control-engine/docs/slice-3-sidecar-local-mode.md); session
[`ce-v1-s1-s3`](sessions/control-engine/ce-v1-s1-s3-session.md). **Next up:** S4 (appliance
registry + registry-routed `call_tool`), then S5 write verbs (which adopt the `ui-ext`
native→host callback transport, the same mechanism the ROS driver uses).

**Just shipped (2026-07-02): the workspace default runtime is honored on a run (agent-config follow-up).**
`agent.config.set` persisted a workspace default, but `agent.invoke` / the channel `/agent` path ignored
it — an omitted `runtime` fell back to the compiled-in `default`. Closed with **one resolution seam**
(`rust/crates/host/src/agent/resolve_default.rs::resolve_effective_runtime`, precedence **explicit arg →
workspace `agent.config.default_runtime` → registry default**) wired into `invoke_via_runtime` — the ONE
place runtime selection happens, so BOTH entrypoints (`agent.invoke` via `serve`, the channel worker)
resolve identically with no second copy. **Registry drift is fail-open** (a stored id the node no longer
offers → registry default + `warn!`, never an errored run; a store-read failure → treated as unset).
**No widening:** resolution runs AFTER the `mcp:agent.invoke:call` gate and is pure selection (every tool
still re-checked under `agent ∩ caller`; reading the config to dispatch needs no `agent.config.get`
grant). The **`model_endpoint` override** at invoke time is a decided, **named-deferred** follow-up
(runtimes are built at boot with a fixed endpoint; honoring the stored endpoint threads the stable
`AgentRuntime`/`RunContext` seam — its own slice). Tests (rule 9, real infra, seeded via the real write
path): host `agent_default_runtime_test` **5** (explicit-wins, absent→stored-default via a registered
stub runtime, stored-but-unavailable→registry-default fallback, workspace-isolation, gate-still-denies);
UI `AgentDefaultRuntime.gateway` **1** (admin sets default in Settings → an omitted-runtime `kind:"agent"`
run resolves it and settles to a durable answer). Live: feature-on node boot lists
`open-interpreter-default`, `agent.runtimes`/`agent.config.set/get` round-trip over `lb`, and the real
`interpreter` subprocess answered `"PONG"` through the seam (role-crate smoke, provided Z.AI key). Scope
[`agent-config`](scope/external-agent/agent-config-scope.md) (open Q resolved); session
[`invoke-default-runtime`](sessions/external-agent/invoke-default-runtime-session.md); public
[`external-agent`](public/external-agent/external-agent.md) ("Honoring the stored default on a run");
skill [`external-agent`](skills/external-agent/SKILL.md) (§4/§5 now shipped). One pre-existing UI flake
noted (`CommandPalette.agent.gateway` runtime-dropdown step; exonerated — fails identically with the seam
bypassed). **Next up:** the `model_endpoint` invoke-time override; wire `serve_agent`/a callable
`agent.invoke` from the node binary (the serve-wiring TODO) so the live channel run is drivable over the
gateway; full `AgentProfile` authoring when the external-agent feature ships in anger.

**Shipped (2026-07-02): the Settings surface — user preferences editor + per-workspace agent config.**
A dedicated **Settings** nav surface (always visible — prefs are member-level) with two tabs. **Preferences**
is the `lb-prefs` client half the prefs scope long deferred: an editor over **all eight axes** (language,
timezone, date/time style, first-day-of-week, number format, unit system, and the closed dimension→unit
overrides) via `prefs.set` (own record) + an admin "Workspace defaults" scope switch via `prefs.set_default`;
it reads `prefs.get` (only set axes) with `prefs.resolve` as the ghost fallback, and the unit-override picker
is generated from the SAME `dimensions.generated.ts` the server enforces. **Agent** needed NEW backend:
`agent.runtimes` only *listed* what a node offers — nothing persisted a workspace's *choice*. Added
`workspace_agent_config:[ws]` (SCHEMAFULL, composite-id, names-only `model_endpoint`) + two verbs
(`agent.config.get` member, `agent.config.set` admin — the latter validates the chosen runtime against the
node's registry, `BadInput` on an unknown id), mirroring `prefs.set_default`; gateway `GET|PUT /agent/config`.
The Agent tab is a runtime dropdown (backed by `agent.runtimes`) + endpoint fields, editable for an admin,
read-only for a member (server is the wall). Tests (rule 9, real infra): host `agent_config_test` **6**
(round-trip + names-only, per-verb deny, workspace-isolation, unknown-runtime reject, idempotent replay); UI
`SettingsView.gateway` **3** (prefs round-trip + hydrate, agent round-trip, cap-gate + server-deny). Scope
[`agent-config`](scope/external-agent/agent-config-scope.md); session
[`agent-config-settings`](sessions/external-agent/agent-config-settings-session.md); public
[`external-agent`](public/external-agent/external-agent.md) (agent-config) + [`prefs`](public/prefs/prefs.md)
(settings UI). **Next up:** honor the stored default in `agent.invoke` when `runtime` is omitted; the
pre-auth bootstrap-locale path; full `AgentProfile` authoring when the external-agent feature ships in anger.

**Just shipped (2026-07-01): channel rich responses (the descriptor-driven render contract) + reminders
as its first tenant — the channel is now a GENERIC front-end for the MCP tool surface.** The channel
became a second mount surface for the shipped dashboard widget contract: a command/tool/agent answers
with a `render` block (`kind:"rich_result"`, v-stamped) in its channel `Item` body, and the channel
mounts it through the **shipped** `WidgetView`/`views/*` — no new renderer, trust router, or bridge.
**The headline (a mid-build design correction):** the first pass leaked tool-specific knowledge into the
frontend (the palette reshaped `reminder.create`'s args + hardcoded `reminder.list`'s render). That
breaks rule 7, so we corrected to **100% backend-driven**: the frontend names exactly **one** tool
(`tools.catalog`) — for every command it lists it, renders its `input_schema` widgets **by string**
(`x-lb.widget`), and posts its **declared** response render, with **zero** `tool.name` branches. The
descriptor carries both halves: `input_schema` (the form) + a new **`ToolDescriptor.result`** field (the
`x-lb-render` response envelope). Verbs accept the **flat form** (e.g. `reminder.create` builds the
nested `Action` server-side); the UI posts collected fields verbatim. The widget/view vocabulary is
**OPEN — UI built-ins ∪ extension-contributed `ext:<id>/<widget>`** (the shipped `WidgetView`/`ExtWidget`
federation, install-gated), unknown→text fallback. **Reminders proves it with zero UI reminder
knowledge:** `reminder.create`'s descriptor declares the cron+select form; `reminder.list`'s descriptor
carries the interactive-table `result` with pause/run-now/delete row controls (a per-row control binds
the row's fields via the shipped vars engine — `${id}` row field + `{{value}}` interaction — passing the
row object as the control's `VarScope.values`); the generic palette renders and posts both. Added the new
gated idempotent verb **`reminder.fire`** (run-now), reusing the shipped internal fire path. The
`query_result → rich_result` migration is additive (old `QueryCard` path kept + a no-regression test).
**Three bugs surfaced by the real-gateway loop:** the `ts` unit (ms→s, fixed); the write verbs
hard-requiring `ts` while the generic controls send none (fixed — default to the host clock, matching
`create`; regression test); and a **pre-existing** one — reminder firing re-resolves the creator's caps
from the durable grant store, so a **dev-login/token-only** reminder won't fire (run-now *and* the
scheduled reactor) — documented as a named follow-up (security-semantics-sensitive, not this slice; the
control is correct and works the instant the fire path is fixed). Rust: whole `lb-host` suite green +
`reminder_fire_test` 9 (deny/isolation/idempotency/flat-form/catalog/ts-default). UI: `pnpm test` green
(open widget registry, cron round-trip, generic dispatch, ResponseView mount/degrade) + real-gateway
reminders loop (create/list-render/pause/delete/deny/isolation/token-never-crosses; run-now asserts the
documented deny). Scopes [`channels-rich-responses`](scope/channels/channels-rich-responses-scope.md) +
[`reminders-rich-responses`](scope/reminders/reminders-rich-responses-scope.md); sessions
[`channels-rich-responses`](sessions/channels/channels-rich-responses-session.md) +
[`reminders-rich-responses`](sessions/reminders/reminders-rich-responses-session.md). **Next up:** the
fire re-resolve fix (persist member caps durably on login), then the named follow-ups — make the legacy
`agent.invoke`/`federation.query` palette branches descriptor-declared routes too (finish the
tool-agnostic palette), A2UI/JSON-render as an additional sandboxed view, pin-a-response-to-a-dashboard,
and the live-updating `/reminders` card.

**Just shipped (2026-07-02): Widget Kit Phase 1 — a declarative per-field presentation vocabulary + the
`lib/widgets/` extraction + a shared table column-model.** A `reminder.list` table rendered raw record keys
as headers (`maxRuns`/`principalSub`/`nextAttemptTs`) and dumped nested `action` as a JSON blob, and the
input widgets were trapped in feature folders. Phase 1 fixes both, **backend-driven and generic** (no
tool-specific UI): (A) a field author declares `label`/`description`/`hide`/`order` **once** — on `x-lb`
for a FORM field, on the shipped `fieldConfig` (`displayName`==label; **added `hide`**/`order`) for a
RESPONSE field — and BOTH the palette arg rail and BOTH table renderers resolve them through the **one**
`resolveFieldPresentation` + `humanize` (`maxRuns`→"Max Runs"), so a header and a form label can't drift.
(B) The input widgets + registry + `CronBuilder` + the presentation/table core moved into `ui/src/lib/widgets/`
(registry = the public API), a **behavior-preserving move + re-export shim** — the palette/dashboard suites
stayed green with no assertion changes. (C) One shared `table/columns.ts` (resolved headers, `hide`,
`order`, nested-value rendering) that both `TablePanel` (read-only) and `ResponseTable` (row-controlled)
consume; ResponseTable's only extra stays the per-row control column. **The motivating fix, green over the
real gateway:** `reminder.list`'s descriptor declares a `fieldConfig` (Max Runs / Next fire / Action,
`principalSub`+`ts` hidden); the render envelope (`RichResultPayload`, TS+Rust) gained an optional
`fieldConfig` (additive data on the existing envelope — **no new verb/cap/table/WIT**), `buildCell` copies
it onto the cell, and the `/reminders` table now reads author labels and hides the noise. **`hide` is
presentation, NOT security** (a hidden field still crossed the bridge under the viewer's grant — proven by
a test; the deny is opaque with or without `hide`). Tests (rule 9, real gateway): Rust `lb-host` 83 lib +
descriptor/payload assertions; UI `pnpm test` 313; real-gateway reminders palette 11 (incl. the presentation
regression mounted off the LIVE catalog + capability-deny + workspace-isolation). Scope
[`widget-kit`](scope/frontend/widget-kit-scope.md), session
[`widget-kit`](sessions/frontend/widget-kit-session.md), public
[`widget-kit`](public/frontend/widget-kit.md). **Next up:** Phase 2 — move the dashboard view
renderers/controls into the library, the mount-context input channel (`ctx.mode`/`value`/`onValue`) +
`defineWidget`, and ext-authored input widgets.

**Just shipped (2026-07-01): the in-channel agent wired into the `/` command palette — the run-lifecycle
#5 read surface.** The rendered channel composer is the `CommandPalette` (not the old, unrendered
`MessageComposer`), so the in-channel agent was orphaned — `/agent hey` showed "No commands match". Made
the agent a **first-class palette command** (`agent.invoke`, gated by `mcp:agent.invoke:call` via the
catalog's per-tool `authorize_tool` — the descriptor NAME is the gate, zero special-casing) whose
`runtime` arg is a real **dropdown** backed by a new read verb **`agent.runtimes`** (`{ default, runtimes }`,
minimal shape, gated by the distinct read cap `mcp:agent.runtimes:call`); submit routes to
`onSendAgent → postAgent` (the `kind:"agent"` payload path + the live `AgentCard`), never a raw
`agent.invoke` tool call. Deleted the orphaned `parseAgentCommand` + `MessageComposer`. Tests (rule 9,
real backends): host `agent_runtimes_test` 5 (read-surface + capability-deny + workspace-isolation +
catalog-integration) + `tools_catalog_test` 4; UI `RuntimeArg` 3 + `pnpm test` 272; real-gateway
`CommandPalette.agent.gateway.test` 2 (post `kind:"agent"` → real drain → `AgentCard` answer). Scope
[`agent-runtimes-scope.md`](scope/external-agent/agent-runtimes-scope.md), session
[`agent-runtimes-picker-session.md`](sessions/channels/agent-runtimes-picker-session.md). **Next up:** the
remaining #5 — ACP `session/cancel` (user-driven stop) + the foreign-loop resume contract.

**Also shipped (2026-07-01): the operator CLI `lb` (v1 slice) — the terminal twin of the shell.**
Executed `scope/cli/operator-cli-scope.md`'s v1 slice boundary. New crate `rust/role/cli/` (`lb-cli`
lib + `lb` binary): a fourth client of the node gateway holding **no authority of its own**, denied
exactly when the server denies. A `Transport` trait with two impls — **Remote** (reqwest over
`POST /mcp/call`) and **Local** (embedded `Node::boot()` + a minted `dev_claims`-shaped `Principal`,
fully offline) — both ending at `lb_host::call_tool`; mode is a `--local` flag, never an `if cloud`
branch. Commands: `lb login`/`whoami` (dev-login token, per-workspace, `0600`, never logged); the
universal `lb call <tool> '{json}'`; one typed `lb inbox list`; `lb devkit sign` + `lb ext publish`
(the `make publish-ext` / `lb-pack` fold over the same `lb-devkit` signing — `lb-pack` kept as a shim,
Makefile green); `lb local …`. **Zero new MCP verbs/caps/tables/enforcement** — a pure client; `-w` is
a **credential selector** (unstored ws → loud error), never a ws override. Every command prints a
`ws/user/role/mode` header (stderr) + shaped body (stdout); a deny renders `DENIED  mcp:<tool>:call`
and exits non-zero. One host touch: `lb_auth::claims_unverified` (a client-side header read, not an
authz path). New deps: `clap`/`tabled`/`dirs`. Tested in-process against a **real** gateway on a real
socket, seeded via the real write path (no mocks): `cargo test -p lb-cli` **38 green** — capability-deny
+ workspace-isolation + offline, remote AND local; `devkit sign` → `verify_artifact` round-trip; config
`0600` + "no command emits the token". Verified live end to end (login → call → sign → publish, remote
and local). Docs: [`public/cli/cli.md`](public/cli/cli.md), [`skills/lb-cli/SKILL.md`](skills/lb-cli/SKILL.md),
session [`sessions/cli/operator-cli-session.md`](sessions/cli/operator-cli-session.md). **Next up:** the
full typed verb set (`ws`/`members`/`channels`/`outbox`/`system`/`agent`/`store`/`tags`) + `watch` SSE
streams — thin additions on the same one client path (v1 non-goals).

**Just shipped (2026-07-01): the shared X/Y plot builder (dashboard + channel query charts).** One
chart model (`ui/src/lib/charts/`: `PlotSpec` + field-kind inference + `buildPlot` + `suggestPlot`,
16 unit tests) and one 10× Recharts renderer/builder (`ui/src/features/charts/`: `PlotChart` with real
titled X/Y axes + gridlines + themed tooltip + legend + reduced-motion draw-in; `PlotBuilder` — run a
query, see the typed fields, pick chart type + X/Y/series with a live preview), reused by BOTH surfaces.
Dashboard panels (`timeseries`/`barchart`/`piechart`) render through it when a `plot` spec is set
(additive; persisted in `Cell.options.plot` via `dashboard.save`, new **Plot** editor tab). Channel
query results get a **Customize** builder; the viewer's choice persists **per-user** via new host verbs
`channel.chart_pref.get`/`.set` (record `channel_chart_pref:[ws, cid__item__user]`, channel-`sub` gated,
member cap added) — the worker-authored result stays immutable. `pnpm test` 258 green; `cargo test -p
lb-host --test channel_chart_pref_test` 3 green (round-trip + per-user + capability-deny + workspace
isolation). Docs: [`public/frontend/dashboard.md`](public/frontend/dashboard.md) → "X/Y plot builder".

**Also shipped (2026-07-01): shadcn migration Waves 0–2 + a reusable page-shell (Flows shape,
everywhere).** Executed `scope/frontend/shadcn-migration-scope.md` over `ui-standards-scope.md`.
**Wave 0** generated the three missing primitives token-bound like `sidebar.tsx` —
`ui/src/components/ui/{alert,dialog,switch}.tsx` (`alert` replaces `.state-alert`; `dialog` on the
already-present `@radix-ui/react-dialog` with real focus-trap; `switch` a hand-authored `role="switch"`
button, no new dep). **Wave 1** migrated the shared `confirm/ConfirmDestructive` (→`Dialog`+`Switch`+
destructive tokens; used by admin ×4 + extensions) and the six channel views. **The alignment fix
(the headline):** every full-screen surface was drifting because only Flows put `AppPageHeader` at the
**full width** with the rail *below* it; the others had a full-height rail beside a body-only header.
Extracted that shape into **reusable layout components** — `components/app/{page,rail,empty-state}.tsx`
(`AppPage`/`AppRail`/`AppEmptyState`) — and adopted them in **Flows** (reference), **Dashboard** (Wave 2:
restructured + header controls → `Button`/`Input`/`Select`/`Badge`), **Rules**, and **Channels** (the
channel rail moved *into* `ChannelView` via `AppPage`, so the `#channel` header spans full width;
`RoutedShell` dropped its channels-only aside; `ChannelsRoute` passes `onSelectChannel`/`onSwitchWorkspace`).
Per the user, **deleted the "add new workspace" control** on the channel rail (`WorkspaceSwitcher` lost
its create `<form>`, migrated to `Select`+`Alert`); restored the workspace tooltip on `AppPageHeader`'s
badge (`title="Workspace {ws}"`). Removed the migrated paths from `ui/eslint.config.js` `LEGACY_VIEWS`
(now **error**-guarded): **`LEGACY_VIEWS` 28 → 17**, lint **150 → 91 warnings, 0 errors**. Presentation
only — **no MCP tool / record / bus subject / capability change**. Verified live against a real gateway
`node` (Dashboard/Flows/Channels/Rules render the identical aligned shape; screenshots this session).
Tests: `pnpm test` 268/268; migrated-view gateway suites green in isolation (`ChannelView` 5/5 incl. a
360px responsive smoke, `RulesView` 7/7, `DashboardView` 6/6, all `flows`). The residual `App.gateway`
red is the **pre-existing `signInReal` "not a member" seeding flake** (varies run-to-run, present at
HEAD, unrelated). **Next up:** Wave 3 (admin/ingest tables + `switch`) and Wave 4/5 (remaining single
surfaces → flip `lint` to `--max-warnings 0`). Scope `scope/frontend/shadcn-migration-scope.md`;
session `sessions/frontend/shadcn-migration-channels-rules-session.md`.

**Just shipped (2026-07-01): the flows data & JSON node pack — 20 built-in nodes.** Executed
`scope/flows/data-nodes-scope.md` end to end: **Tier A** pure transforms (`change`/`select`/`merge`/
`map`/`flatten`/`sort`/`range`/`aggregate`/`template` + `csv`/`xml`/`yaml`/`base64`), **Tier B** durable
state (`filter` RBE, `unique`, `batch` — one additive capped `flow_node_buffer` record), **Tier C**
engine-extending (`switch` edge-gating, `split`/`join` array-carry, `delay` durable park). The registry
went 8 → 28 descriptors. Pure transform logic lives in `crates/flows/src/ops/` (unit-tested in-crate);
`builtins.rs` → `builtins/` and `execute_node.rs` → `execute_node/` (FILE-LAYOUT, all files < 400 lines).
Three new spine Decisions (**14** switch edge-gating, **15** split/join array-carry, **16** delay
parks-on-resume) and **all five scope open questions resolved** — zero left. New parse crates
(`csv`/`quick-xml`/`serde_yaml`/`base64`) added to `key-stack.md`. Frontier engine gained a `Skipped`
gating outcome + `run_store::{ready_one_dependent,skip_gated,park_step}` + `flows.resume` clearing a
suspended status. **No new MCP verb, no new capability.** Green: `cargo build/test --workspace`,
`cargo fmt`; unit 78 + Tier A 15 + Tier B/C 11, mandatory capability-deny + workspace-isolation both
proven. One bug found+fixed (`debugging/flows/switch-else-branch-fires-unconditionally.md`). Scope
`scope/flows/data-nodes-scope.md`; session `sessions/flows/data-nodes-session.md`; public
`public/flows/flows.md` (data-pack section).

**Just shipped (2026-07-01): `chains` engine RETIRED — `flows` is the one DAG engine.** Executed
`scope/flows/chains-retirement-scope.md` (`flows-scope.md` Decision 6, taken to its clean-cut end
state — delete, no alias, no stub). Deleted outright: the host `chains` module (8 files), the
`lb_rules::workflow` DAG model, the gateway `/chains` routes, the six `mcp:chains.*` + two
`store:chain:*` grants, and the React chain feature (`features/chains` + `lib/chains`, the Chains nav
entry/route/surface). `flows` is a proven strict superset (same binding grammar + triggers +
one-job-per-node topology + frontier driver + CAS run-store, plus richer nodes and a live-SSE canvas),
so nothing is lost — to "chain rules into a DAG" you author a flow of `Rhai`/`Tool` nodes. Superset
proven before any chain test was deleted (2 gap flow tests added first: partialFailure-under-Halt +
`MAX_FLOW_NODES` save-reject). **Regression guard (the headline):** `chains_retired_test.rs` (each
retired verb → unknown-verb `NotFound`, gone not merely ungranted) + `chains_retired_routes_test.rs`
(each `/chains…` route 404, `/flows` still 200). Removed one straggler: `max_chain_steps` chains-only
dead code. `rule-chains-scope.md` kept as `rubix-cube` lineage; `rules-workbench-scope.md` Phase-2 DAG
canvas banner'd as superseded by Flows. Green: `cargo build/test --workspace`, `cargo fmt`, `pnpm test`
(242), `pnpm test:gateway`; prove-absence grep clean (no live `chains`/`lb_rules::workflow` outside
`rubix-cube` + `docs/` lineage + the two regression tests). Scope
`scope/flows/chains-retirement-scope.md`; session `sessions/flows/chains-retirement-session.md`; public
`public/flows/flows.md` (chains-removed note).

**Earlier on 2026-07-01: flow timestamp display + a flow-read "binding broken" fix.** A flow node's
canonical epoch-**seconds** `ts` now renders as the viewer's wall-clock via `format.datetime` (the
`lb-prefs` host tool) wired into the ONE viz field-config bridge (`fieldconfig/format.ts` — the
long-planned "when lb-prefs ships, this becomes the real dispatch" swap), unit declared as `dateUnit:"s"`
so the seconds→ms ×1000 is never magnitude-guessed. Backend: `prefs.get`/`resolve`/`set` are now
member-level (a member must resolve their OWN prefs to render; still forced to the caller's `sub`, deny
test unchanged). Also fixed the live "binding broken — re-pick" on saved flow read cells (an empty v2
`source:{tool:""}` placeholder shadowed the real v3 `sources[]` in `WidgetView`). Scope
`scope/prefs/flow-ts-display-scope.md`; session `sessions/prefs/flow-ts-display-session.md`; debug
`debugging/flows/flow-read-binding-broken-empty-source.md`. Tests: +3 real-gateway (two-viewer render +
ws-isolation) +8 unit; regression suites green (UI 232, flow-binding gateway 16, Rust prefs 8).

**Flows — the visual node-graph engine (backend spine + Wave 3 surfaces): SHIPPED** (2026-06-30), over the
shipped rules + jobs + outbox + extension plane. A `flow:{ws}:{id}` typed node graph authored
on a React Flow canvas and run as a durable `lb-jobs` session — **flows are not a new engine**,
they generalise the `rubix-cube` rule-DAG (Decision 6: one engine — `chains` since **retired**, see
above). The
Decisions 1–13 are held verbatim. Wave 3 adds the **editor canvas** (Slice E) + the **dashboard↔flow
binding** (Slice F) — pure clients of the shipped `flows.*` / `flows.nodes` gateway verbs (no new host
work, no new caps).
- **`lb-flows`** — a pure crate (serde + serde_json + thiserror + jsonschema only): the
  `NodeDescriptor` keystone contract, the additive `[[node]]` manifest block (parse + validate: the
  bound `tool` must exist + the inline JSON-Schema 2020-12 config must compile), the five built-in
  descriptors, the merged `flows.nodes` registry (built-ins ∪ installed-ext nodes — a read-time union
  over `install` records, derived not stored), the typed `Flow` graph model + DAG math (Kahn, the
  chain binding grammar verbatim), and the canonical `coalesce` enum.
- **the run engine** — a `flow_run` coordinator (pins `flow_version`, Decision 1) + one `flow-step`
  `lb-jobs` job per node, driven by the `chains` frontier driver ported verbatim (Decision 8): CAS
  exactly-once (`Enqueued→Running`, the cross-node owner), `FailurePolicy = Halt|Continue`,
  suspend/resume/cancel, `flows.patch_run` (config-only to an UNEXECUTED node, validated against the
  PINNED schema — Decision 12), `ResumePointDrift`, subflow-parks-on-child (Decision 11), the full
  `flows.*` run surface incl. `flows.runs.list` (reattach).
- **extension nodes** — descriptor-aware dispatch under `caller ∩ install-grant` (the shipped
  `build_call_context` — two-direction deny, no widening); the source shape (host-owned
  `flow:{ws}:{flow}:{node}` series + arm/disarm, Decision 2); the worked `mqtt/extension.toml`.
- **triggers/lifecycle** — the five kinds; `flows.enable`; the two passes — `react_to_flows_cron`
  (clock-scan, deterministic firing id, fire-once-then-skip) + `reconcile_flows` (single-owner
  election, arm/disarm, guarded teardown — Decision 13); `flows.inject` (Decision 9 retain-vs-fire);
  placement matched as data.
- **Composition, never widening:** `flows.run` plus every node-tool's own gate under `caller ∩ grant`
  — a node calling a tool the caller lacks is denied at that node, recorded `Err`, run continues.
- **Tests (all green, real `mem://` store + real jobs/caps/outbox/install records — no mocks):**
  lb-flows **26** + ext-loader **16** + host **30** (flows_nodes 5 · flows_run 12 · flows_ext 5 ·
  flows_triggers 8) — incl. capability-deny per verb, the no-widening run gate, workspace-isolation
  on every record, resume/cron-replay exactly-once, ResumePointDrift, subflow-parks-on-child. Sessions:
  `sessions/flows/{node-descriptor,flow-run,extension-triggers}-session.md`; public:
  `public/flows/flows.md`. **Deferred:** the mqtt native sidecar binary · subflow
  reactor-driven park · cross-node owner failover (Decision 10) · host-side `flows.save` journaling
  (canvas undo is client-side until it lands).

**Flows — runtime control (Wave 3, observable + interruptible runtime): SHIPPED** (2026-06-30), over
the flows backend spine. Driven by a live-node reproduction of four user reports ("start but not
stop", "no live values", "export can't see connections", "node config posts the whole flow"). The
runtime already ran headless (cron/boot/event reconcilers share `coordinator::drive`) but drove
**synchronously to terminal inside the call** — so a run was over before any observer could watch or
interrupt it. This slice makes the existing runtime **observable + interruptible**:
- **the manual run is a background job** — `flows.run` seeds synchronously, `tokio::spawn`s the drive,
  returns `run_id` at once (the cron/boot/inject reactors keep the synchronous path).
- **`cancel`/`suspend` bite mid-run** — the driver re-reads the durable run status between frontier
  batches and halts on `cancelled`/`suspended` (Stop actually stops; a deterministic test proves a
  pre-cancelled run drives no node).
- **a live SSE watch** — `flows.watch` + `GET /flows/runs/{run}/stream` streams a snapshot then
  `node-settled`/`run-finished` deltas over a workspace-walled `flow:{ws}:{run}` Zenoh subject (a
  near-verbatim copy of the `run_events` watch trio); the canvas folds it (SSE-primary, poll-fallback).
- **per-node config CRUD** — `flows.node.get`/`flows.node.update` on the saved flow (schema-validated,
  version-bumped) so a node tweak isn't a whole-`Flow` post; a **"Save node"** button in the canvas.
- **Tests (green, real `mem://` + bus/jobs/caps — no mocks):** host **9** new (deny + ws-isolation per
  node verb · watch deny + ws-isolation + snapshot-then-delta · async run returns-before-terminal ·
  mid-run cancel stops, deterministic) keeping the flows suites at 49; frontend **6** real-gateway
  (node.update round-trip + version, schema reject, deny, ws-isolation, async run settles, export
  `needs` round-trip) + **4** unit (`flowGraph` export round-trip). Session:
  `sessions/flows/flow-runtime-control-session.md`; debug:
  `debugging/flows/async-run-not-send-recursion.md`; scope:
  `scope/flows/flow-runtime-control-scope.md`; public updated. **Deferred (unchanged):** cross-node
  owner failover for a backgrounded run (restart `flows.resume` re-drives) · per-node step-level
  token streaming.

**Flows — the Node-RED message envelope (`payload`/`topic`, auto-wire): SHIPPED** (2026-07-01), over the
flows runtime. A flow message is now a JSON **envelope** — a primary `payload` slot (always on output) +
optional `topic` (routing/name) + free metadata — that **flows down a wire automatically on connect** and
carries `topic` through a chain (Node-RED ergonomics over our own durable per-node engine, no shared
mutable `msg`). A **clean breaking change** (flows is in dev): `${steps.x.output}` and bare node outputs
are removed.
- **auto-wire (D3)** — single upstream + no `with.payload` → inputs = the upstream's full recorded
  envelope (no binding typed); a **join** (≥2 upstreams) with no `with.payload` is rejected at save by the
  new `UnboundJoin` lint (auto-wire never silently picks one upstream).
- **carry-forward (D4)** — a node's recorded envelope = `{ ...carry, ...emitted }` (carry = inputs minus
  `payload`, so `topic` propagates); a join carries nothing.
- **binding grammar (D5)** — `${steps.x}` = whole envelope; `${steps.x.<dot.path>}` = a field path into it
  (`payload`/`topic`/`findings`/`payload.items.1`, missing→null); `.output`/`.findings` are no longer
  special. Whole-reference only, no interpolation.
- **per-builtin envelopes (D6)** + **explicit `counter` mode (D7)** — `tick` (default, +step every firing
  regardless of payload) vs `throughput` (+payload size); the implicit-throughput trap is gone. Built-in
  descriptor ports renamed to `payload`/`topic`/`findings`. Sink destination = `msg.topic ?? config.name`.
- **canvas (D10)** — `flow_node_state` stores the whole envelope; `flowGraph` shows its `payload` badge
  (falls back to the whole envelope when there's no `payload`).
- **Tests (green, real `mem://`/caps/jobs — no mocks):** `lb-flows` (binding field-paths/whole-envelope,
  the join lint, no-stray-port); host `flows_run_test` +6 (auto-wire 3-node no-`with`, join-lint
  rejection, topic carry-forward, `rhai return msg`, counter tick-vs-throughput) keeping the flows suites
  green, sink reads `payload` + `msg.topic` routes; `ui flowGraph.test` (envelope→payload badge +
  fallback). The mandatory capability-deny + workspace-isolation tests carry over (assertion shapes moved
  to `payload`). Session: `sessions/flows/flow-message-envelope-session.md`; scope:
  `scope/flows/flow-message-envelope-scope.md`; public updated. **Built on by:** the flow⇄dashboard
  binding UX below (picker `payload`/`topic` ports, read views default to `payload`).

**Flows — the flow⇄dashboard binding UX (pick a node + port; switch / slider / JSON, both ways):
SHIPPED** (2026-07-01), over the shipped `flows.inject` write + read-out (no new transport). Makes the
bidirectional binding **authorable in clicks** and carries **structured JSON both ways**.
- **port-aware inject** — `flows.inject {…, port?}` upserts the per-port `flow_input:{flow}:{node}:{port}`
  (node-level unchanged); same `mcp:flows.inject:call` cap + per-call ws + grant recheck (no widening).
  Threaded through the host dispatch + the gateway `POST /flows/{id}/inject` route.
- **binding precedence** — `resolve_node_bindings` overlays retained `flow_input` so a run's input is
  **per-port retained > node-level retained > static `with` / auto-wire** (explicit + tested, both
  branches); the injected value is the node's `payload`.
- **read-back** — `flows.node_state {id}` folds each node's retained `flow_input` (`input` node-level +
  `inputs` per-port) into its entry, so a control seeds its CURRENT state from its OWN input, not its
  output. One read drives canvas + dashboard; no new verb.
- **frontend** — a **Flows source-picker group** (flow→node→port → Action/Source, friendly labels, no
  tool name); switch/slider/**JSON** controls driving a port + seeding current value on mount
  (`useFlowNodeValue`); a new **JSON/object read view** (`jsonview`) pretty-printing a node's `payload`.
- **Tests (green, real spawned gateway / `mem://` — no `*.fake.ts`):** host `flows_triggers_test` +8
  (port upsert, precedence by a real run, object round-trip, node_state read-back, inject deny node- AND
  port-keyed, ws-isolation); `ui` unit `flowsPicker.test.ts` (6); `ui` gateway
  `FlowDashboardBinding.gateway.test.ts` +7 (picker offer, slider port-inject + precedence, current-value
  mount read, switch boolean, JSON validate/reject, JSON read advance, ws read-isolation). Session:
  `sessions/flows/flow-dashboard-binding-ux-session.md`; scope `scope/flows/flow-dashboard-binding-ux-scope.md`;
  public `flows.md` + `frontend/dashboard.md` updated. **Deferred:** a `flows.node.watch` SSE (sub-second
  liveness) + JSON-path sub-field output selection.

**Flows — PLC-grade reliability (unique run ids + conflict-safe writes): SHIPPED** (2026-06-30), over
the flows runtime. Driven by a live `:8080` reproduction: 8 concurrent `POST /flows/chain4/run`
returned **one shared run id** + a wall of `Invalid revision` / `read or write conflict` store
errors. Root cause (one bug, three symptoms): the run id was **constant for the node's whole uptime**
— `gw.now` froze at gateway construction and the manual id was `"{flow_id}-run-{now}"`, so every run
re-drove the same terminal record (churn + flickering Stop/Resume + "no values") and overlapping runs
raced the run-store's monotonic `rev` RMW. Fixes:
- **unique run id** — `flows.run` mints a ULID (`new_ulid`) when no `run_id` is supplied; a
  caller-supplied id is still honored (resume/subflow/retry); `default_run_id` kept for inject/cron.
- **live gateway clock** — `Gateway::now` is an accessor (live `SystemTime` in prod, injected
  `fixed_now` for tests); the 35 `gw.now` sites became `gw.now()`, fixed-clock test seam intact.
- **conflict-safe store write** — store-level `lb_store::write_locked` (per-`(ws,table,id)` async lock
  + bounded retry-on-conflict, the `capped_insert` shape) backs `run_store` + `lb-jobs`; `create_run`
  seeds create-if-absent (idempotent under a racing `start`).
- **Live verified:** the same 8 concurrent runs now return **8 distinct ULIDs, zero store errors**,
  each settling `success`. **Tests (real, no mocks):** `store::write_locked_test` (concurrent
  same-record writes → coherent rev) · `host::flows_plc_reliability_test` (concurrent-same-id-settles-
  once [mandatory regression] · unique-id · cap-deny · ws-isolation); flows suites + store + jobs +
  UI unit (186) regression-clean. Session: `sessions/flows/flow-plc-reliability-session.md`; debug:
  `debugging/flows/{frozen-gw-now-collides-run-ids,run-store-rev-conflict-under-concurrency}.md`;
  scope/public updated. **Plus reactive cron firing (item 5), shipped same session** after a live
  "cron trigger never fires" report: `react_to_flows_cron`/`reconcile_flows` had **no production
  driver** (only tests called them) → added `spawn_flow_reactors` (a detached per-node tick over the
  configured ws, live clock) wired into node boot; `flows.save` now **derives** the canonical
  `flow.cron` from a `mode:cron` trigger node's `config.cron` (the canvas wrote the node config, the
  reactor scanned the flow field — disconnected); and the node binary switched to `Gateway::new_live`
  (it was still building the gateway with the fixed-clock seam). **Live-verified:** a `* * * * *`
  trigger fires every minute headless, each run settling `success` with real values. Debug:
  `debugging/flows/cron-trigger-never-fires-no-reactor-driver.md`; test
  `flows_triggers_test::cron_trigger_node_derives_flow_cron_and_fires_a_run`.

**Flows — the persistent runtime view (Node-RED / PLC steady state): SHIPPED** (2026-06-30), over the
reactive-cron slice. Driven by a live report: opening an armed flow showed a **frozen last-run "DONE"
snapshot** + a contradictory "54 runs / no runs yet" banner ("the count isn't going up"). Deep-review
root cause (design): the canvas's only runtime view was one **finite `flows.runs.get` snapshot**, but
the spec's persistent per-node state — **Decision 5: `flow_node_state` last-value, updated in place
each scan** (the Node-RED "each wire shows its current value"), already WRITTEN by `record_outcome` —
had **no read verb** and was never painted. Fixes: **`flows.node_state {id}`** verb
(`crates/host/src/flows/node_state.rs`, gated + ws-walled; `GET /flows/{id}/node_state`) returns every
node's `{node, value, rev}` + armed fields; the **canvas paints node_state as the base steady-state**
with the run snapshot overlaid (`nodeStateValues`; `values = {...base, ...overlay}`), refreshed on the
armed tick; an **honest armed banner** ("next fire / last fired / N runs"); **deterministic
multi-trigger** cron derivation (conflicting specs reject, identical collapse); `runs.list` gains `ts`
+ newest-first. **Live-verified:** `node_state` rev advanced 59→60 on a cron firing (updated in
place). **Tests:** host `node_state_*` + `multi_trigger_*` (3) · frontend `nodeStateValues` +
`armedState` (9) · flows backend (38) + UI unit (195) + flows gateway e2e (24) green. Session:
`sessions/flows/flow-persistent-runtime-session.md`; debug:
`debugging/flows/canvas-shows-finite-run-not-persistent-node-state.md`; scope/public updated.

**Flows — N independent triggers per flow + per-wire subgraph runs + a real counter: SHIPPED**
(2026-06-30), over the persistent-runtime slice. Driven by the user: saving `chain4` with **two cron
triggers** was rejected ("a flow has one schedule") — "I never said max one trigger; the whole design is
wrong, it needs to be like Node-RED, unlimited nodes — what about a webhook or MQTT-sub?". Root cause
(design, two bugs): a flow's reactive state (`flow.cron`/`flow.next_attempt_ts`) was **hoisted to the
flow** (one schedule/cursor), and `create_run` enqueued **every** indegree-0 node (one run fired the
whole graph). First evaluated reusing edgelinkd/reflow/phlow/dora as a library on a Pi — chose to **keep
+ evolve our engine** (it has the durable/resumable/capability/ws run-store the in-memory candidates
lack) and borrow their ideas. Fixes: **per-node trigger cursors** `flow_trigger_state:{flow}:{node}`
(`trigger_store.rs`; the reactor scans **trigger nodes**, N independent — no single-schedule rejection,
only malformed-cron rejected); **per-wire subgraph runs** (`Flow::reachable_from`/`indegrees_within` +
`create_run{entry}` + recorded `entry_node`, `finalize` scoped to the run's node set; wired to
cron/inject/boot/`flows.run{entry}`); a real **`counter`** builtin backed by durable **node memory**
`flow_node_memory:{flow}:{node}` + a new **atomic `lb_store::increment`** (server-side accumulate, per-key
serialized — a retry can't double-add), so the count GOES UP per firing and survives restart (the
original ask). **Tests (real store/jobs/caps — no mocks):** `store::increment_test` (64 concurrent
firings → unique totals 1..=64, ws-walled) · `host::flows_multi_trigger_test` (multi-cron independence,
subgraph isolation + `entryNode`, counter 1→2→3, cap-deny, ws-iso) · `lb-flows` model helpers · full
host (64) + store + jobs + flows green · `cargo fmt`/`build --workspace` clean · UI `pnpm test` 195.
Scope: `scope/flows/flow-multi-trigger-reactive-scope.md`; session:
`sessions/flows/flow-multi-trigger-reactive-session.md`; debug:
`debugging/flows/flow-level-cron-rejects-multiple-triggers.md`; public updated. **Follow-up (not bugs):**
UI per-trigger armed chips · per-node enable/disable · orphan-cursor sweep on trigger removal · native
http-in/webhook source node (its own scope).

**Flows — headless Stop/Deploy + armed-after-restart + Count/Counter clarity: SHIPPED** (2026-06-30),
over the multi-trigger slice. Three live reports against the canvas: (1) "can't see the Stop button",
(2) "show whether the flow is running after a server restart + reload", (3) "the count still isn't going
up". Root causes: the armed banner read the **dormant** `flow.cron`/`next_attempt_ts` (no writer since
triggers moved to per-node cursors) so a cron flow showed **idle after reload**, and the only Stop (per-
run Suspend/Resume/Cancel) renders only for a non-terminal **live run** — which a finite-firing headless
flow never has, so a cron/source flow had **no Stop at all**. The "count" report was **not a bug**: the
flow used `count` (pure per-firing transform) where it needed `counter` (durable accumulator). Fixes
(UI; the `flows.enable`/`flows.node_state` backend was already complete): `deriveArmedState(flow, runs,
nodeState?)` now reads the AUTHORITATIVE `flows.node_state` (`enabled` + soonest cron/nextAttemptTs from
the per-trigger cursors) with graph-derived "scheduled" (holds when disabled); a durable **Deploy/Stop**
toggle in `FlowArmedBanner` bound to `flows.enable` (survives restart). Plus a backend clarity fix: the
two near-identical palette titles became **"Count (input size)"** / **"Counter (running total)"** (type
ids unchanged; needs node rebuild to surface). **Live-verified on `:8080`/`acme`/`chain4`:** converting
`count`→`counter` made `count-5` climb headless **3→4→5** at successive minute boundaries (trigger-4 on
its own clock) and `a` **20→24** (its `[1,2,3,4]` throughput binding) — both with zero manual runs; a
throwaway `counter(step:1)` ticked 1→2→3. **Tests:** `armedState.test.ts` rewritten to the node_state
model (incl. armed-after-restart) + new `FlowArmedBanner.test.tsx` (Stop/Deploy fires `onToggle`); UI
`pnpm test` **203** green; `cargo build -p lb-flows` clean, `lb-flows` 29. Session:
`sessions/flows/flow-runtime-stop-deploy-and-counter-clarity-session.md`; debug:
`debugging/flows/armed-banner-reads-dormant-cron-no-stop-for-headless.md`. **Also this session — the
`sink` node could never write:** its `series` target sent `{series,value,ts}` to `ingest.write` (whose
`Sample` needs `producer`/`seq`/`payload`) → "missing field `producer`", and its `inbox` target sent
`{channel,body}` to `inbox.record` (needs `id`) → "missing arg: id"; no sink-execution test existed.
Fixed `dispatch_sink` to build valid bodies + added `host::flows_sink_test` (3, real ingest/inbox,
fail-before verified). Debug: `debugging/flows/sink-node-request-shapes-dont-match-target-verbs.md`.
**Follow-up:** surface a node
`description` in the palette (the deeper Count/Counter fix; titles are the stopgap) · per-trigger armed
chip on each trigger node (data in `node_state.nodes[].armed`).

**Saved-PRQL-query surface (`query.*`): SHIPPED** (2026-06-30), layered over the rules + federation
plane. A workspace authors a query once in **PRQL** (or `lang:"raw"` for dialect-native text), saves
it as an editable `query:{ws}:{id}` record, and runs it against **any** source through one MCP family.
PRQL is the authoring layer only — no new engine, SurrealDB stays the one datastore (rule 2).
- **`lb-prql`** — a pure, zero-I/O crate wrapping the pinned `prqlc` (0.13): `compile(prql, dialect)
  -> sql` per dialect (Generic/Postgres/MySql/DuckDb), goldens frozen.
- **`query.*`** — `save`/`get`/`list`/`delete`/`run`/`compile` host verbs over `query:{ws}:{id}`,
  each its own capability. `query.compile` is a pure dry-run (own cap, no data access); `query.run`
  compiles for the target's dialect and **dispatches to the engine that already owns the wall** —
  `store.query` (platform) or `federation.query` (datasource). **`query.run` composes, never widens**
  (rule 5): it needs `mcp:query.run:call` **and** the target's underlying cap (`store.query` /
  `federation.query`); holding `query.run` alone is denied. `$var` binds through the engines' real
  param paths (injection-safe); a `lang:"raw"` write is rejected by the read-only gate.
- **rules seam** — `source("query:<name>")` resolves to `query.run` under `caller ∩ grant`, so a saved
  query is a centrally-editable data definition rules reuse by name.
- **Phase 2** (DataFusion-over-`store.query`-reads for full Surreal PRQL) is deferred by decision; the
  relational subset + `raw` escape hatch ship now. UI Queries page deferred (follow-on).

**Rules + chains plane (post-S8 platform capability): SHIPPED** (2026-06-28). The lazybones-native
**rules engine** (`lb-rules`) + **rule chains** + the **federation datasources** extension landed,
ported from the `rubix-cube` engine (MIT/Apache-2.0) and **re-seamed onto our chokepoints**.
- **`lb-rules`** — a sandboxed `rhai` cage (governors + zero I/O surface) + a lazy column-oriented
  `Grid` + the timeseries plan-builders + the `AiMeter` budget + the nsql fence + the DAG model/binding
  resolver, linking only `rhai`+`serde`. The three seams the scope named are abstracted as traits the
  host implements: grid `collect` → `store.query`/`series.*` (platform, SurrealQL) or
  `federation.query` (external); `ai.*` → the AI-gateway; `alert` → inbox + outbox.
- **`rules.*`** — `run`/`save`/`get`/`list`/`delete` host verbs over `rule:{ws}:{id}`, each
  capability-gated, with the per-source `caps::check` inside every collect (`caller ∩ grant`).
- **`chains.*`** — a rule DAG driven over **`lb-jobs` + a SurrealDB run-store** (rubix-cube's trait
  shape, our durable backend): `save` (DAG-validated up front), `run`/`resume`, `get`/`list`/
  `runs.get`. The CAS step-claim makes a restart-`resume` exactly-once.
- **platform additions:** a `caps` `Net` surface + `Connect` action (the `net:*` grammar), and a
  capability-mediated **`lb-secrets`** store (was an S0 placeholder) for the federation DSN.
- **federation extension** — a native (Tier-2) `federation` ext embedding DataFusion as a library,
  `federation.query` + `datasource.*` CRUD + `federation.mirror` (an `lb-jobs` batch into the series
  plane), `net:*` + secret-gated, SELECT-only validated, tested against a real spawned DB.

**Tests (the gate, all green):** 28 `lb-rules` unit (cage / grid / **AI fence+budget** / DAG+binding)
+ 12 host integration (6 rules + 6 chains: capability-deny per verb, mid-run source deny, ws-isolation,
the seed-real-series→rollup+alert→inbox round-trip, AI budget, DAG validation at save, diamond frontier,
Halt subtree-skip, **restart-resume-exactly-once**) + the federation real-DB suite + `lb-secrets`
mediation/deny/isolation. No mocks; the only fake is the model provider behind the AI seam (+ the real
external DB behind the one `Source` trait). Sessions: `sessions/rules/rules-session.md`,
`sessions/datasources/datasources-session.md`; public: `public/rules/rules.md`,
`public/datasources/datasources.md`.

**Rules `ai.*` → real model wired** (2026-07-03): the `rules.run` bridge no longer hardcodes
`DisabledModel` — a rule's `ai.*` reaches the node's real `ModelAccess`, resolved per-workspace from the
agent-catalog pick (`agent.config`). Single-turn, no tools; the nsql fence + budget meter unchanged. An
unconfigured workspace (or provider-less node) still gets the honest `"AI not configured for rules"`
error and runs data-only rules. +8 host integration tests over the real bridge against
`AiGateway<MockProvider>` (AI-wired · not-configured · ws-isolation · fence regression · budget ·
adapter unit), all green. Scope `scope/rules/rules-ai-wiring-scope.md`; session
`sessions/rules/rules-ai-wiring-session.md`. Known gap: token accounting is a length estimate until
`Turn` surfaces the provider's real count (named in `model.rs`).

**S8 — data plane (durable store + generic ingest + tagging): exit gate MET** (2026-06-27). All three
slices shipped on the pinned **SurrealKV** persistent engine — (0) `Store::open` + the capability-spike
matrix + crash-consistency set, (1) `lb-ingest` durable exactly-once buffer (proven across a
kill-mid-commit), (2) `lb-tags` typed graph + spike-gated full-text/vector/counts, with `series.find`
discovery wired on tags. Data survives a node restart; a fleet writes one series without collision;
isolation/deny/offline tests pass on disk. The S9 collaboration-UI work proceeds in parallel. (Earlier
context below.)

**In S7 — platform maturity** (see `STAGES.md`); **both** S7 exit-gate slices have shipped — the
**signed extension registry** and the **native Tier-2 supervisor** — so the **S7 exit gate is fully
MET**. The Rust workspace + a React/Tauri UI exist and build; messaging, a second node + sync,
cross-node routed tool calls, the browser SSE/HTTP path, shared workspace assets, the **AI core**
(central agent + AI-gateway sidecar + durable jobs), the **coding workflow** (issue → triage →
approval-gated job → progress → transactional outbox), the **signed registry** (pull · verify · cache ·
install · offline · rollback) — now over a **real HTTP transport** (`lb-role-registry-host` server +
`HttpSource` client, replacing the in-memory stub) — and the **native Tier-2 supervisor** (a supervised
OS-process sidecar that restarts cleanly with no durable state lost) are all proven end to end.
The native tier's deferred **child→host callback transport** is now **SHIPPED** (2026-07-02): a native
sidecar calls host MCP tools out-of-process over an authenticated `POST /mcp/call` via the generic
**`lb-sidecar-client`** crate — the out-of-process dual of the wasm guest's in-process `host.call-tool`
bridge, denied identically by the host gate. This also made the child's injected `LB_EXT_TOKEN` a
genuine node-signed JWT (the signing key now lives on `Node`, shared by the gateway + the token minter,
replacing the throwaway-key co-trust placeholder). Proven by a real gateway over real HTTP: happy
round-trip, capability-deny, and workspace-isolation. See
[`native-callback-transport`](scope/extensions/native-callback-transport-scope.md). This unblocks the
`ros` driver (poller → `ingest.write`, `point.write` → `outbox.enqueue`) and later `mqtt`/`control-engine`.
The S6 **github-bridge** is now packaged as an installed Tier-1 wasm artifact (the deferral resolved; the
orchestrator stays a host service by design), with a **live HTTP ingress** (**`lb-role-github-webhook`**,
HMAC-verify the `X-Hub-Signature-256` → `ingest_via_bridge`) and a **live HTTP egress** (the outbox's
**`lb-role-github-target`**, delivering `create_pr`/`comment` over GitHub REST) — the relay now hardened
with **backoff + dead-letter**. The ingress and egress now **connect end to end into a live PR**: the
producer emits the structured `{repo,head,base,title,body}` payload the GitHub target maps, and a
durable-scan **resolution reactor** (`react_to_approvals`) auto-starts the coding job the moment its
approval lands `Approved` — closing webhook → triage → approval → JOB → outbox → GitHub with no manual
step. The ingress is now **multi-tenant** (`tenant_router`: `POST /webhook/{tenant}` over a
`TenantRegistry`), one process fronting many workspaces, each authenticated by its own secret with the
workspace wall held at the front door. And the whole loop now **runs as a service**: `lb-role-github-workflow`
ticks the reactor + the outbox relay per workspace (`run_workflow_loop`), mounted into the `node` binary
by config (`node/src/github.rs`) alongside the webhook front door — so a real webhook delivery flows
issue → triage → approval → JOB → PR end to end in a running process — and the set of serviced
workspaces is a **durable directory** (`register_workspace`/`deregister_workspace`) the driver re-reads
each tick, so a workspace is onboarded/retired **without a restart**. No doc-site build and no native
desktop window (webkit toolchain) yet.

**API keys (machine principals) — shipped** (2026-06-29). Long-lived, non-human credentials
(appliance/cli/api/agent) over the **existing** authz model — a key is a non-human
`Subject::Key("{id}")`, authenticated by a peppered bearer secret `lbk_{ws}.{id}.{secret}` (verified
per request, O(1) ws-scoped lookup) and authorized through the one `caps::check` chokepoint. Read-only
vs read-write is just the resolved caps (two built-in roles, `apikey-read`/`apikey-write`); expiry is a
lazy check at authentication; revoke is instant on the revoking node (hash→principal cache busted).
New `lb-apikey` pure crate + host `apikey` service + gateway bearer-auth + the admin "API Keys" tab.
See `sessions/auth-caps/api-keys-session.md`, `public/auth-caps/auth-caps.md`.

**S0 exit gate — MET.** `cargo build --workspace` green; CI runs (FILE-LAYOUT size check +
build wasm guest + test + fmt); the four forever decisions (SDK/WIT, capability grammar +
token, job-queue, extension manifest) are written as scope docs.

**S1 exit gate — MET.** A tool call routed through MCP succeeds *with* the grant and is
refused *without* it; a second workspace cannot see the first's data. Through the real WASM
component. See `sessions/core/s0-s1-spine-session.md`.

**S2 exit gate — MET.** Post a message in the UI and it appears (Vitest `ChannelView`); history
survives independent of the bus / a restart (the store keeps it); an extension version swaps live
(hello v1→v2) with state intact. Mandatory capability-deny, workspace-isolation (bus + store +
inbox), and hot-reload categories. See `sessions/bus/messaging-session.md`.

**S3 exit gate — MET.** A second node joins (config-only `Node::boot_as(role)`); a cross-node tool
call routes over a Zenoh queryable and is capability-checked on the calling node, workspace-first;
channel data syncs edge↔hub with **idempotent offline apply** (§6.8); the browser reaches a node
over **SSE/HTTP** (replacing the S2 in-memory fake) and sees live messages appear. **61 Rust + 8
Vitest + 2 shell tests** pass — incl. capability-deny, workspace-isolation, and the first
offline/sync categories, all now **across two nodes** and the gateway. See
`sessions/sync/multi-node-sync-session.md` and `public/SCOPE.md`.

**S4 exit gate — MET.** A doc private to a user is shared to a team and read by a member while a
**non-member is denied** (gate 3, the membership layer below the workspace wall); the doc linked
into a channel is read by a channel `sub`-grantee; a **skill loads only when the workspace granted
it**; extension install records persist `requested ∩ admin_approved` per workspace. Capability-deny
(non-member / no-grant) and workspace-isolation hold across **store + MCP**. New `lb-assets` crate +
host `assets` service + `assets.*` MCP bridge + UI `DocView`. **83 Rust + 11 Vitest + 2 shell
tests** pass. Content is stored as a record (not `DEFINE BUCKET` — unavailable in our `kv-mem`
build; an S7 config swap). See `sessions/files/shared-assets-session.md` and `public/SCOPE.md`.

**S5 exit gate — MET.** An edge user invokes the central agent over the routed MCP namespace; the
agent calls the gateway for a model turn and a **granted MCP tool** inside its loop (under
`agent ∩ caller` — no widening); a workflow **job survives the edge disconnecting and resumes
idempotently**. New: `lb-jobs` (durable resumable session), `lb-role-ai-gateway` (swappable model
access + replay-safe idempotency cache, mock provider), host `agent` service (the loop + the gates)
+ routed wiring, grant **delegation** (`Principal::derive` + caps gate 2b), `agent.*` MCP bridge, UI
`AgentView`. **105 Rust + 14 Vitest + 2 shell tests** pass — incl. capability-deny (invoke gate +
the in-loop intersection), workspace-isolation across **store + MCP**, and offline/sync (interrupted
session resumes; duplicate invocation does not re-spend). See `sessions/agent/ai-core-session.md`
and `public/SCOPE.md`.

**S6 exit gate — MET.** A GitHub issue → inbox `needs:triage` → the S5 agent triages + drafts a
**shared scope doc** → a `needs:approval` inbox item **genuinely gates** a durable coding job (no job
record before approval; refused with `AwaitingApproval`; a rejected approval starts nothing) →
progress streams to a channel (motion) → every external effect goes through the **transactional
outbox** with at-least-once retry + receiver dedup (never lost, never double-sent). New: `lb-outbox`
(the transactional `Effect` + `enqueue`/`pending`/`mark_*`/relay), `lb_store::write_tx` (the one-tx
seam), `lb_inbox::Resolution` (the approval facet), the host `workflow` service (the orchestrator +
the gate), `workflow.*` MCP bridge, UI `WorkflowView`. **124 Rust + 18 Vitest + 2 shell tests** pass
— incl. capability-deny (each workflow verb), workspace-isolation across **store + MCP**, and
offline/sync (the outbox delivers at-least-once, idempotently). See
`sessions/coding-workflow/coding-workflow-session.md` and `public/SCOPE.md`.

**S7 exit gate — FULLY MET.** *Registry half:* an extension installs from the **signed** registry →
pull · **verify** (Ed25519 over a digest binding manifest+wasm) · cache · install through the existing
S4 flow; runs **offline** once cached; **rolls back** with **no durable state lost**; a tampered/
unsigned/foreign-key artifact is **rejected before caching, even with the grant**. *Native half:* a
**native Tier-2 sidecar is supervised and restarts cleanly** — a killed child (a **real OS process**)
is respawned, resumes answering, and **no durable workspace state is lost** (a channel message posted
before the crash is intact after); install/lifecycle are capability-gated (no spawn without
`mcp:native.install:call`); ws-B can never see or control ws-A's sidecar (store + MCP + the runtime
map); a signed `tier="native"` artifact installs through the registry and a tampered one is rejected.
New (registry): `lb-registry` + the host `registry` service + `registry.*` MCP bridge + UI
`RegistryView`. New (native): `lb-supervisor` (spawn/frame/health/restart behind a `Launcher` seam) +
the `echo-sidecar` reference binary + the `[native]` manifest block + the host `native` service
(supervision stateless: live PID in a runtime `SidecarMap`, durable truth in `Install` + `native_status`
records) + `native.*` MCP bridge + UI `NativeView`. **~163 Rust + 26 Vitest + 2 shell tests** pass —
incl. capability-deny, workspace-isolation across **store + MCP**, offline, rollback/hot-reload,
signing/verification, and the **supervision/restart** category (real process, no durable state lost).
Posture: process-group isolation + scoped identity + bounded restart; OS hardening + a boot reconciler
are noted follow-ups. See `sessions/registry/registry-session.md`,
`sessions/extensions/native-tier-session.md`, and `public/SCOPE.md`.

---

## Slices in flight

One row per vertical slice being built. State: `scoped` → `building` → `tested` → `shipped`.

| Slice | Topic | Stage | State | Scope | Session | Notes |
|---|---|---|---|---|---|---|
| **Email/password login + workspace selection (the Slack front door) — backend (Phase 1)** — a globally-unique case-insensitive `email` + one global argon2id password on the `_lb_identity` identity; admin verbs `identity.set_email`/`identity.set_password` (gated `mcp:identity.manage:call`) + self-service `POST /auth/password`; the three gateway routes `POST /auth/login {email,password}` (verify globally → enumerate effective memberships → 0→403 / 1→full token / N→select-token + roster), `POST /auth/select {workspace}`, `POST /auth/switch {workspace}`. | auth-caps | post-S10 (core-auth-caps) | **tested** (2026-07-16; working tree, not git-committed) — additive, alongside the legacy `/login`; the removal sweep is the next slice | [email-login](scope/auth-caps/email-login-scope.md) | [email-login](sessions/auth-caps/email-login-session.md) | **`lb-authz`:** `Identity.email` (lower-cased) + `fold_email` + a **race-safe unique index** `identity_email:{folded}` via `store::create` (Conflict-on-duplicate, not read-then-write); `identity_by_email` case-insensitive lookup; new **`identity_credential.rs`** global credential record in `_lb_identity`. **`lb-host`:** new **`identity_credential/`** service (`set` admin verb · `change` self-service · `verify` = `global_credential_verify`, **timing-uniform** — argon2 burned against a dummy hash on unknown/absent so email-enumeration has no latency oracle · MCP bridge); `identity/` gains `email` in the view, `set_email`, `by_email`, and **`login_workspaces`** (un-gated enumeration: effective member AND not disabled, `{ws,name}`). **`lb-role-gateway`:** `GlobalCredentialCheck` trait (`GlobalPasswordHash`/`GlobalDevTrustAny`, selected by `LB_DEV_LOGIN` like the per-ws seam); the **select-token** (`ws:""`,`caps:[]`,`constraint:["ws-select"]`,~5-min TTL — powerless everywhere, positively accepted ONLY at `/auth/select`); **`mint_full_session`** = the one role-correct issuance path factored out of `login.rs` (viewer floor ∪ `resolve_caps_live` ∪ nav-reach) so `/login` + all `/auth/*` mint byte-identically; `/auth/login|select|switch|password` + `auth_reply.rs`; per-email failure rate limit (10/15min, `FixedWindowLimiter::peek`); admin REST `/admin/identities` (accepts `email`) + `/{sub}/email` + `/{sub}/password`. **Decisions:** all 3 open questions resolved (dev-login honored · 10/15min per-email · `{ws,name}` roster). **Tests (real gateway+store, argon2 real, no mocks):** `email_login_test` 6 (1/0/N branch · **unknown-email==wrong-password 401 body** · email uniqueness+case-insensitive · self-service rotate) + `email_login_deny_test` 3 (select-token refused everywhere but `/auth/select` · full token refused at `/auth/select` · switch-to-non-member 403) + unit `select_token` 3 + `rate_limit::peek` 1. **Regression green:** `login_hardening`/`identity_routes`/`gateway`/`admin_routes`/`nav_reach`/`viewer_reach` + gateway `--lib` + `lb-authz`/`lb-host --lib` + `cargo build --workspace`. **Deferred:** the `/login` + per-ws `Credential` + `identity.set_credential` **removal sweep** (machines → API keys; port legacy suites) — next slice; email-verify/reset/MFA/OIDC unchanged deferrals; rubix-ai `[patch]` wire (Phase 2) + UI (Phase 3) are downstream. |
| **`@nube/dashboard` — the reusable dashboard grid core (v0.1)** — extract the shell's grid into `packages/dashboard`, the sixth `packages/*` sibling: the pure cell/geometry model + Grafana fieldConfig TYPES (no apply logic) + panel-rows math + `mergeLayout` (row-carries-members), a react-grid-layout host behind a widget REGISTRY (unknown view → honest placeholder; `ext:*` wildcard for federation tiles), an opaque generic `scope`, a package-owned `TimeRange`, and a read-only mobile stack the grid degrades to below 768px. Never fetches/persists/knows a workspace; persistence = the consumer's `onLayout`. RGL/react-resizable CSS shipped scoped under `.lbdg-root` (cannot leak into a host). 24/24 tests (incl. a REAL end-to-end drag through RGL in jsdom), typecheck + build (ESM+CJS+dts+css) green. Tagged `dashboard-v0.1.0` for the git-subdir pin. Shells NOT migrated (later slice). | frontend | S10+ | **shipped** (2026-07-15) | [scope](scope/frontend/dashboard/dashboard-package-scope.md) | [session](sessions/frontend/dashboard-package-session.md) | Grid core only — no fieldConfig apply, wizard, datasources, variables. |
| **Embed-node — `lb-node` lib target + `boot_full(BootConfig)` seam (Phase 2a)** — give the `node` package a LIBRARY target exposing a supported embed API (`BootConfig` + `boot_full`) so a third-party embedder (`NubeIO/rubix-ai`) git-deps `lb-node` and boots a full node in-process; refactor `main.rs` onto it. | node-roles | S10+ | **tested** (2026-07-10; working tree, not git-committed) | [embed-node](scope/node-roles/embed-node-scope.md) | [embed-node](sessions/node-roles/embed-node-session.md) | **Package renamed** `node`→**`lb-node`** (bin stays `node`, lib `lb_node`, `version=0.1.9` kept for the core-skills seeder); all `cargo …-p node` repointed to `-p lb-node` (Makefile `NODE_BIN`, `deploy/common/Dockerfile`). **New lib** (`rust/node/src/`, folder-of-verbs, all files <400 lines): `lib.rs` barrel · `config.rs` (`BootConfig` `#[non_exhaustive]`+`Default` + `from_env()` — the ONE place `LB_*` boot vars are read · `GatewayMode` · `AgentModelConfig`) · `builder.rs` (`boot_full(cfg)->RunningNode` = the one ritual + `RunningNode::serve()` + struct-sourced `open_store`) · `seeds.rs` · `reactors.rs` · `seed_identity.rs` · `hello_demo.rs` (gated). **`main.rs` thin** (17 lines): `boot_full(BootConfig::from_env()).await?; running.serve().await`. **`RunningNode`** hands back `{ node: Arc<Node>, gateway: Option<(Gateway,SocketAddr)>, agent_server: Option<AgentServer> }` (fields `pub` for an additive `shutdown()`). **Config-vs-drift:** `hello_demo`/`seed_user` are config postures (`from_env` = exact binary parity, `Default` = embed-friendly off/skip); store-path selection relocated from `Node::boot`'s internal env read up to `from_env()` (same behaviour). Load-bearing native-role/agent mount-AFTER-gateway-key ordering preserved EXACTLY. **Deferred (named):** de-env the `federation`/`control_engine` role mounts (still read their own `LB_*` env — the core ritual is fully struct-config); `GatewayMode::Listener` + real `RunningNode::shutdown()` (reactor cancel + sidecar token); refactor the OTHER two embedders (`ui/src-tauri/src/full.rs`, `test_gateway.rs`) onto `boot_full` (Phase 2b). **Tests green** (`rust/node/tests/embed_test.rs`, real `mem://` store, no mocks): `embedded_node_denies_a_caller_without_the_cap` (MANDATORY cap-deny), `embedded_node_isolates_workspaces` (MANDATORY ws-isolation), `from_env_defaults_match_the_binary` — all via `boot_full`. `cargo build --workspace` ok · `cargo build -p lb-node --features external-agent` ok · `cargo test -p lb-node` 5/5 · `cargo fmt`. |
| **Extensions out-of-tree — SDK extraction + cutover (slices 1–2)** — make `NubeDev/lb-ext-sdk` (Rust) + `NubeDev/lb-ext-ui-sdk` (TS) the AUTHORITATIVE owners of the extension contract; `lb` becomes a consumer. | extensions | S10+ | **shipped** (2026-07-10; working tree, not git-committed) | [ext-out-of-tree](scope/extensions/ext-out-of-tree-scope.md) | [cutover](sessions/extensions/ext-out-of-tree-cutover-session.md) | **Slice 1 (SDK filled):** `lb-ext-native` now speaks lb's REAL supervisor wire — it had a divergent `Request`/`Response`/`Init` shape no host could read; rewrote `frame.rs`/`wire.rs`/`handshake.rs` as byte-for-byte mirrors of `lb-supervisor::{frame,rpc}` + a `serve(reader,writer,tools)` loop (`init`/`health`/`call`/`shutdown` → a caller `Tools` impl) + `serve_stdio(tools)`. `lb-sdk` gained a `links` build script exporting `DEP_LB_SDK_WIT*`, `WORLD_NAME`, and documented guest usage. Tagged `sdk-v0.2.0`→`sdk-v0.2.1`; 26 tests green, clippy -D clean. **Slice 2 (cutover, behavior-neutral):** workspace `lb-sdk` dep repointed path→git-tag `sdk-v0.2.1`; **`lb/rust/sdk` deleted**; host `runtime` bindgen sourced from the SDK WIT via a new `build.rs` reading the `links` metadata + `include!`ing a generated `bindgen!` from `$OUT_DIR` (no in-repo WIT copy — closes the host-mirror leak the scope names); `ext-loader` unchanged (only `world_major_matches`). Guests (hello, hello-v2, the 5 product exts, core-thing fixture, devkit wasm template) converted to the same seam (`build.rs` reads `DEP_LB_SDK_WIT`→`generate!`; normal `lb-sdk` dep — a build-dep does NOT get `DEP_*`). UI shell (`ext-host/federation.ts`, `dashboard/builder/federationWidget.ts`) imports the page/widget contract from `@nube/ext-ui-sdk` (`ui-v0.4.0`, `link:` interim) — the "three mirrors" collapse to one; `contract-mirrors.guard.test.ts` updated to assert the package is authoritative. **Green:** `make build-wasm` ok · `cargo build --workspace` ok · `cargo test --workspace` ok EXCEPT one **pre-existing** failure (`lb-cli reminder_test`, from the tree's builtin-role-freshness authz WIP — touches zero SDK code) · `pnpm test` 166/168 files (2 pre-existing: radius-scale.guard, DebugValueView). The extraction's own tests pass (contract-mirrors 6, ExtWidget 6, ExtWidgetTheme 1, ext-host federation). **Deferred (named):** slice 3 Artifact v2 + `lb-ext publish` wired · slice 4 exts move to `lb-extensions` + fixtures to `rust/fixtures/ext/` · slice 5 CI conformance · npm publish (`link:` until then) · the zero-boilerplate `lb_sdk::export!` guest macro (fragile cross-crate; `generate!`-from-`DEP_LB_SDK_WIT` shipped instead). |
| **Reports — the report builder + branded PDF exporter (finish)** — close the demo-pass loose ends on the built reports feature: drop the unused TipTap deps, unwind the PanelPicker's cross-feature import, and land the DURABLE fix for the frozen-built-in-role-row footgun that blocked the demo (the throwaway reseed). | reports | post-S8 | **shipped** (2026-07-10; working tree, not git-committed) | [report-builder](scope/reports/report-builder-scope.md) · [builtin-role-freshness](scope/auth-caps/builtin-role-freshness-scope.md) | [reports-finish](sessions/reports/reports-finish-session.md) · [debug](debugging/auth/builtin-role-row-frozen-stale-on-new-caps.md) | The feature (all 5 tracks) was built/green per `HANDOVER-reports.md`; this session finished four things. **A — editor port + deps:** verified the ported textarea editor (`components/markdown-editor/MarkdownEditor.tsx` + `MarkdownBody.tsx`, react-markdown/remark-gfm) is a faithful port of lazybones' shipped editor and reads in the block card + the A4 preview; `a4-sheet.ts` kept (preview geometry); deleted the now-unused TipTap deps (`@tiptap/react`/`starter-kit`/`pm`/`tiptap-markdown`, 0 imports) from `ui/package.json` + refreshed `pnpm-lock.yaml` (−572 lines, all tiptap/pm). **B — demo coupling unwound:** lifted the shared demo cell builders out of `features/admin/setup/*` into `lib/panel/demoCells.ts` + `lib/panel/demoGallery.ts` (one responsibility/file); `dataToInsight.ts` keeps wizard-only artifacts + re-exports; `PanelPicker` now imports from `@/lib/panel` (no cross-feature import); `templateGallery.ts` deleted, its test moved to `lib/panel/demoGallery.test.ts`. **C — durable role-freshness fix:** a workspace seeded before a new built-in cap was added kept the stale `member`/`workspace-admin` role rows forever (idempotent seed writes only when absent) and `resolve_caps` read that stored record — so a new built-in cap never reached already-seeded tokens. Fix: `resolve_caps_with`/`_subject_caps_with`/`_sourced_with` take a `BuiltinRoleCaps` callback and UNION the live `*_role_caps()` on top of the stored record for granted built-in roles (union not replace — an ext's `grant_assign(Subject::Role(name))` still honoured; custom roles untouched); `LiveBuiltinRoleCaps` (host) + `resolve_caps_live`/`resolve_subject_caps_live` are the canonical host entry points every caller (login mint, apikey, reminder, dashboard access_check, access console) uses. The throwaway `rust/node/examples/reseed_roles.rs` + `examples/` dir DELETED. **D — tests:** `crates/authz/tests/builtin_role_freshness_test.rs` (4: stale row → missing without the union, present with it; NoBuiltin=raw; custom role unaffected; union keeps direct role-subject grants) + the `sourced_cap_set_equals_resolve_caps_no_drift` cross-check still green; UI `MarkdownEditor`/`MarkdownBody`/`PanelPicker` unit (15). **Green:** `cargo test -p lb-render --lib` (24) · `cargo test -p lb-host --test report_test` (7) · `cargo test -p lb-role-gateway --test report_routes_test` (4) · `cargo test -p lb-authz` (all) · `npx tsc --noEmit` clean · `npx vitest run src/features/reports src/features/panel src/components/markdown-editor` (106). **E — docs:** session + debugging entry + README row + public `reports.md` promoted + this row + `docs/skills/reports/SKILL.md` + scope "Post-scope demo pass" updated. Rule 10 held (no branch on an ext id). Lesson: two halves of one role-resolution path went stale at different rates (viewer live via the login floor, author/admin frozen via the stored row); make the resolver AUTHORITATIVE for built-in names, don't repair the stored row. |
| **weather — REMOVED from core; rebuilt as an out-of-tree extension** — the host-native `weather.current` tool + built-in dashboard viz were hard-removed from `lb` and rebuilt as a standalone native (Tier-2) extension in `NubeIO/rubix-ai-extensions` (`extensions/weather/`) | weather | post-S8 | **removed** (2026-07-14) | — (scope moved out; was `scope/weather/`) | [ext-out-of-tree](scope/extensions/ext-out-of-tree-scope.md) | **Hard removal, no migration** (per request). Deleted from core: `rust/crates/host/src/weather/` (module + `weather_tool_test`), the `mod weather`/re-exports in `host/src/lib.rs`, the `"weather."` `HOST_NATIVE_PREFIXES` entry + dispatch arm in `tool_call.rs`, `mcp:weather.current:call` from `VIEWER_CAPS` (`builtin_roles.rs`), the `weather.current` `catalog.rs` entry, and the `weather` view from `widget_catalog.json`. UI: deleted `dashboard/views/weather/` (`WeatherPanel`/`wmoCode`/`observedLocal` + its gateway test), `panel-builder/options/defs/weather.ts`, and the **entire geo-search control chain** it was the sole consumer of (`controls/{GeoSearch.tsx,geocode.ts,geoWrite.test.ts}`, the `geo-search` control kind in `types.ts`, `writeGeoPlace`/`GeoPlace` in `binding.ts`, the `Control.tsx`/`OptionSectionCard.tsx` branches); dropped `weather` from the `View` union, `NO_FIELDCONFIG_VIEWS`, `WIZARD_VIEWS`, `SOURCELESS_VIEWS`, the display-override read views, VizGallery/VizPicker cards, and `usePanelData`'s `weatherSource` self-source path; removed the `LB_WEATHER_OPEN_METEO_BASE` weather stub from `test/real-gateway.ts`. **The replacement** (built the same session, in `rubix-ai-extensions`): a native sidecar serving `weather.current {lat,lon}` over stdio via `reqwest`→Open-Meteo behind the `LB_WEATHER_OPEN_METEO_BASE` seam (`net:tls:api.open-meteo.com:443` gated), plus a module-federated `Current Weather` dashboard widget (temp/condition/wind/updated) reading lat/lon from `ctx.options`. **Green:** ext `cargo test` 4/4 incl. a live-socket fetch test through a real local Open-Meteo stub; ext UI `pnpm test` 3/3 + `vite build`; lb `cargo build -p lb-host` clean, lb UI `tsc` clean (weather-free) + affected vitest suites (VizGallery/registry-round-trip/dashboard) green. Rule 10 held both ways — core now has **no idea weather exists** (the ext id is opaque data through the generic native-sidecar + widget seams). |
| **rules cage — `records()` honors its `Array<Map>` contract on the federation path + a chart-ready buildings example** — the documented `category(query(...).records(), ...)` one-liner was a lie on every sqlite/postgres source; collapse the two seam shapes at the boundary + add the chart example the docs promised | rules | S10+ | **shipped** (2026-07-09) | [rules-for-widgets](scope/frontend/dashboard/rules-for-widgets-scope.md) (slice 3 follow-up) | [records-maps-federation-chart-example](sessions/rules/records-maps-federation-chart-example-session.md) · [debug](debugging/rules/records-returns-positionals-on-federation.md) | **The drift:** the catalog advertised `records(grid) -> Array<Map>` and the rules skill doc documented `category(query(...).records(), "name", "value")` as a chart-ready rule's last line — but on the **federation** path (every sqlite/postgres source, including `demo-buildings`) `records()` returned positional **arrays**, so the one-liner errored `every row must be a record`. Two seams feed the cage `Grid` with two row shapes: platform (`store.query`/Surreal) → JSON objects; federation (`extensions/federation/src/query.rs::shape`) → column-aligned arrays. `grid.rs::records()` forwarded whatever the seam returned; the cage unit tests ran against `RecordingData::platform(...)` (object fixtures) so they masked the drift; the host render path (`viz::frame::result_to_rows`) was already correct via a separate columnar-zip arm. **The fix (one normalizer at the seam boundary):** `grid.rs` gained `row_to_map(row, columns)` (object→pass through; array→zip with `columns` into a map keyed by SELECT aliases; scalar→single-cell map) and `records()` routes every row through it — so the `chart` family, `emit` data, and plain `r.col` access work uniformly on every source kind. The three buildings examples + their regression-test assertions moved off the broken positional shape (`r[0]`/`r[1]` → `r.building`/`r.kwh_per_m2`; `rows[0][0]` → `rows[0].get("building")`). **New chart-ready example** `buildings-intensity-chart` in `buildings_examples.json` — same proven intensity query, last line `category(rows, "building", "kwh_per_m2")`; bind a panel to `{tool:"rules.run", args:{rule_id:"buildings-intensity-chart"}}` and it renders. `RecordingData::federation(...)` test helper seeds federation's real positional wire shape so the contract is unit-testable without the sidecar. **Tests green:** `cargo test -p lb-rules` (24, incl. 2 new — `records_returns_named_maps_from_federation_positional_rows` + `category_runs_on_federation_records`), `rules_buildings_examples_test` (121s, real spawned federation sidecar + real `buildings.db`: chart body → 8 rows trimmed to label+value, Riverside 4.68 kWh/m²), + `query_test`/`rules_test`/`rules_ai_wiring_test`/`federation_sqlite_test` (no blast-radius breakage). Lesson: a catalog signature is a contract not a description of behavior, and two shapes at a seam need exactly one normalizer at the boundary — and a "hard-won fact" recorded against a green test can be a fact about a bug (the test asserting the broken shape was the drift's hiding place). |
| **Widget catalog + save-validation (widget-platform Slice A)** — backend-owned `widget_catalog.json` served over a new `dashboard.catalog` MCP verb (built-in view palette + per-view config schema + ext tiles + genui components) + host-side `dashboard.save` rejection of unknown `view` kinds (closes G4: the AI hallucinating views the save then accepts). | widgets / frontend | S9+ | **shipped** | [slice scope](scope/frontend/dashboard/widget-catalog-scope.md) · [umbrella](scope/widgets/widget-platform-scope.md) | [widget-catalog](sessions/widgets/widget-catalog-session.md) | **Shipped (2026-07-04):** `dashboard.catalog` verb (`host/src/dashboard/catalog.rs`, `&Arc<Node>` branch before the generic `dashboard.`) merges built-ins (`widget_catalog.json`) + ext `[[widget]]` tiles (generic `ext.list`, opaque ids) + genui names; `views.rs` validator rejects unknown `view` on `dashboard.save` (store-only, view-NAME only; `ext:<id>/<widget>` accepted structurally); `mcp:dashboard.catalog:call` added to `member_caps()`; dead TS `View` ids trimmed to match the 17-case render switch. Green: host `widget_catalog_test` 8/8 (deny+plain-member, ws-iso with a real widget install, save-validation over shell + headless `POST /mcp/call`, round-trip), `views.rs`/`credentials.rs` units, TS catalog↔renderer consistency guard; `pnpm test` 536. Follow-ups: option-key validation, version stamping/migration, `ext:` install-resolve `warnings[]`, the RN app renderer. |
| **`@nube/source-picker` — reusable source picker (db / datasources / Zenoh / flows / ext widgets)** — extract the dashboard's "pick a value/source" machinery into a transport-agnostic package so surfaces OUTSIDE the dashboard reuse ONE picker (first new consumer: the `thecrew` graphics-canvas extension). | frontend | S10+ | **shipped** (2026-07-02) | [scope](scope/frontend/dashboard/source-picker-package-scope.md) · [dashboard index](scope/frontend/dashboard/README.md) | [session](sessions/frontend/source-picker-package-session.md) | **Package built + dashboard migrated (parity) + thecrew consumer wired LIVE.** `packages/source-picker` mirrors `@nube/panel` (pure, props-driven, React peer dep, ESM+CJS+dts+scoped CSS): 3 layers — MODEL (`buildSourceEntries`, all groups, pure), LOADER hook (`useSourcePicker(loaders, ws)` over an INJECTED `SourceLoaders` seam; deny-tolerant; reads loaders via a ref so an unmemoized object can't infinite-loop — caught via an OOM), UI (`<SourcePicker>` grouped `<select>`). Imports NO `@/` / no transport — one picker works from the shell (gateway/Tauri) AND an extension (bridge). Package 16/16. **Dashboard refactor (zero behavior change):** kept every consumer's import path via two shims — `builder/sourcePicker.ts` re-exports the package model (+ keeps the positional `buildSourceEntries` sig), `builder/useSourcePicker.ts` is the shell adapter (`SourceLoaders` from `@/lib/*`); `ExtWidget` imports `widgetIdOf` from the package. Parity: dashboard unit 129/129, gateway `widgetBuilder` 17/17 + `panelEditor` 6/6 + `flowsPanelEditor` 4/4, `tsc` + shell build clean. **thecrew consumer (the reuse payoff, LIVE):** `file:` dep (builds `--ignore-workspace`); `bridge/source-loaders.ts` implements `listSeries` over `bridge.call("series.list")`; manifest gains `mcp:series.list:call` (shipped verb, consumer request); a loaders context feeds the `PropertyRail` bind picker — which replaced the old `source.channels()` closed loop with the reusable `<SourcePicker>` that DISCOVERS every workspace series. `bind` stays `{channel}` (scope decision; widening filed as `scene-source-binding`). thecrew unit 69/69 (+3 `PropertyRail.test.tsx` via real `mountPage`); live e2e `thecrew-bind-picker.spec.ts` 1/1 (pick a discovered series → bind channel updates) + page/widget e2e still 2/2. (Pre-existing, NOT this work: `DashboardView.gateway` "tab Field" fails on the clean tree.) Zero core additions. |
| **thecrew → the graphics-canvas extension (phases 1–2: viewer + editor)** — lift the proven playground into a publishable extension: zero-tool wasm stub (the publish path requires component bytes; no UI-only tier), federated `[ui]` graphics page + `[[widget]]` scene cell on one remote, scenes as workspace docs via `assets.put_doc/get_doc/list_docs`, live values via `series.latest` backfill + `series.watch` SSE through the bridge (the simulator fake is deleted, per rule 9) | frontend | S10+ | **shipped** (2026-07-02) | [thecrew-extension](../rust/extensions/thecrew/docs/thecrew-extension-scope.md) · parent: [graphics-canvas](scope/frontend/graphics-canvas-scope.md) · [public](public/frontend/graphics-canvas.md) | [thecrew (playground build)](sessions/frontend/thecrew-session.md) · [scoping](sessions/frontend/thecrew-extension-scoping-session.md) · [lift build](sessions/frontend/thecrew-extension-session.md) | **Phases 1–2 lifted & green.** Shipped: `extension.toml`+`Cargo.toml`(excluded from host ws)+zero-tool wasm stub `src/lib.rs`+`build.sh`; federation remote (`vite` lib build → one `remoteEntry.js`, React externalised, three.js bundled) exporting `mountPage`+`mountWidget`; `bridge/scene-io.ts` (load/save/list via `assets.*`) + `bridge/bridge-source.ts` (one-multiplexer ValueSource: `series.latest` backfill + `series.watch`/poll). Simulator **deleted** (rule 9); default source is inert. Tests: thecrew `pnpm test` 50/50 (lifted 31 + new seams) + `TheCrew.gateway.test.tsx` 6/6 (round-trip, backfill, cap-deny ×2, workspace-isolation, widget no-access) on the real spawned gateway; `cargo build --workspace`+`fmt`+ext `cargo test` green. **Findings** (surfaced, not built): (1) `put_doc` still last-writer-wins — interim client read-before-write conflict + honest "changed underneath you" prompt (generic `document-store/` revision ask stands); (2) `assets.list_docs` returns only `{id,title}` (NO tags) — scene picker filters on an `scene:` id-prefix convention (parent OQ3 resolved to prefix) + still tags `scene`; (3) the member/dev cap set lacks `mcp:assets.put_doc:call` — a real save needs the install grant to carry it (positive gateway tests mint it); (4) `series.watch` SSE has no gateway-vitest transport — backfill proven live, watch proven via the widget stub (Playwright deferred, matching proof-panel). Debug: standalone `pnpm install` walked to the repo workspace → `--ignore-workspace` in build.sh. **Session 2 (LIVE — tested→shipped):** published + installed into a running node via `make publish-ext EXT=thecrew` (HTTP 204; `Graphics` `[ui]` slot + `Scene` `[[widget]]` in `ext.list`; `remoteEntry.js` 200 at the manifest path) and driven in a real browser. **Finding 3 fixed** (the real blocker): the page bridge authorizes against the LOGGED-IN user's caps, not the install grant — `member_caps` lacked the exact `assets.{get_doc,put_doc,list_docs}` + `series.watch` verb caps (only `store:doc/*`/`mcp:*.write` wildcards) → live save/load 403'd; added those EXISTING caps to `credentials.rs` (grant-config, zero core additions; gateway tests 13/0, deny tests still deny). `make seed-thecrew`/`seed-demo.sh` seeds the AHU-1 scene doc + `ahu1.*` series + a read-only dashboard through the real verbs (parent OQ4). **Two live-only bugs fixed** (green in unit+gateway, broke live): (5) three.js in a Vite LIB build leaks `process.env.NODE_ENV` → `process is not defined` → `define` in vite.config; (6) shell `pickMount` wants the page export named `mount`, thecrew exported `mountPage` → export `mount`+alias. Live e2e **2/2** (`thecrew.spec.ts`: nav slot → page mounts → AHU-1 loads → **SF-1 live 1800** in the rail → drag→save→reload; `thecrew-widget.spec.ts`: seeded `ext:thecrew/scene` dashboard cell mounts read-only, canvas renders, NO save bar) + screenshots in `docs/shots/`. **New findings (STOP-and-surfaced):** (7) the reworked dashboard PanelEditor source picker DROPPED the "Extension widgets" `PickerGroup` — a packaged `[[widget]]` can't be added via the live builder UI (core dashboard change; seeded the cell directly instead); (8) `ExtWidget.tsx` passes `options:{}`/`binding:{}` — a cell's `options.sceneId` never reaches the widget (interim: thecrew reads `ctx.vars.sceneId`); (9) SurrealKV `Invalid revision` on `list_docs` after repeated writes (pre-existing engine bug — demo runs on in-mem). **Session 3 (findings 5–9 + blank-cell fit closed):** (7) restored the "Extension widgets" group in `QueryTab` — selecting a tile sets `view:ext:<id>/<widget>` + clears the target (re-wires the existing picker/serializer, zero core additions); (8) `WidgetView`→`ExtWidget` now forward `cell.options`/`cell.binding` to `ctx` (the `ctx.vars` workaround retired) + a Scene picker over `assets.list_docs` (`useSceneDocs`) sets `options.sceneId`; (5) shared `federation-remote.preset.ts` carrying the `define`+React-externals so a new bundling ext doesn't rediscover them; (6) `mount.test.tsx` asserts the remoteEntry exports `mount`+`mountWidget`; **blank widget cell fixed** — `canvas/fit-bounds.ts`+`FitCamera.tsx` auto-fit the ortho camera to the cell every frame (drei re-asserts declarative props, so a one-shot effect stayed blank; [debug](debugging/frontend/scene-widget-cell-renders-blank-fixed-camera-fit.md)). Verified LIVE: `thecrew-widget.spec.ts` now **drives the restored palette** (Add panel → pick "thecrew · Scene" → pick scene → Save) and the AHU-1 scene renders framed in the cell (e2e 2/2; screenshots refreshed). thecrew unit 60/60, dashboard-core unit +7 (QueryTab/ExtWidget), gateway `TheCrew` 6/6 + `panelEditor` 6/6. **Session 4 (Phase 3 — AI drawing):** shipped [`docs/skills/graphics-canvas/SKILL.md`](skills/graphics-canvas/SKILL.md) (scene schema + generated shape catalog + the `bind→series` contract + read-modify-save loop + a worked "draw AHU-1" curl run) and **teaching-error validation** — `scene/catalog.ts` (the catalog from the live symbol registry, one source of truth) + `validate.ts::teachingReport` flags unknown types and returns the catalog so an AI self-corrects. Channel/dashboard embed (`{view:"ext:thecrew/scene",options:{sceneId}}`) verified shipped (rides `ResponseView.buildCell`→`WidgetView`→`ExtWidget`; locked with a test). **STOP-and-surfaced (core-blocked):** the in-page draw-with-AI RAIL — `agent.invoke` is not an MCP tool / has no gateway route, and the page bridge is `/mcp/call`-only, so the rail needs a NEW agent-invoke surface (the AI-drawing LOOP itself is zero-core via `assets.*`; the agent already runs it server-side from channels). thecrew unit **66/66**, live SKILL recipe verified against the real node. NOT built (phases 4–5): symbol packs, 3D; + the rail once the invoke surface exists. |
| **External-agent runtime seam (#1) + the real `exec --json` driver behind it** — a host-owned `AgentRuntime` trait beside `ModelAccess`, a runtime registry keyed by profile id, the `external-agent` cargo feature (OFF by default), and the shipped `lb-external-agent` driver reachable through the seam with Open Interpreter as the default and VT Code/Codex swappable by profile id (no code change) | external-agent | S10 | **shipped (#1)** | [runtime-seam](scope/external-agent/runtime-seam-scope.md) · [external-agent](scope/external-agent/external-agent-scope.md) | [runtime-seam-integration](sessions/external-agent/runtime-seam-integration-session.md) | **`lb-host`**: `AgentRuntime` (object-safe) + `RunContext` + `ErasedModel`/`ModelHandle` (erase `ModelAccess` so the in-house loop is a registry trait object AND a `ModelAccess` consumer — no second loop); `InHouseRuntime` = the always-registered `default` calling `run_session` verbatim; `RuntimeRegistry` (absent→default, known→entry, **named-unknown→error**); `invoke_via_runtime` (one selection point, same invoke gate for every runtime). **`lb-role-external-agent`** (new role crate, feature-gated): `AcpRuntime` (one type; per-agent difference = `AgentProfile` data) wrapping `lb_external_agent::drive`, forwarding `RunEvent`s; `profiles::resolve_builtin` (open-interpreter/vtcode/codex; OI+Codex share the codex shim, differ only by binary); `scratch::ScratchDir` (per-run per-ws cwd seal — the run-lifecycle #5 filesystem seal, path-traversal-safe). **`node`**: optional dep + `external-agent` feature (`cargo tree` proves the ACP/external deps ABSENT from the OFF build, present ON) + `register_external_runtimes` hook. Selection wired into the routed `agent.invoke` (`AgentInvokeRequest.runtime`, `serve_agent` carries the registry, `invoke_remote` gains `runtime`). **#3 wall / #4 model-routing / #5 durable-job+resume+supervision are named, linked TODO seams — not faked.** Tests (rule 9): host seam gate (5 — resolution + default-unaffected + capability-deny), role swap/registry/scratch-isolation (9), opt-in real-subprocess smoke (`EXTAGENT_SMOKE=1`). Next: move the default onto the official ACP SDK (`interpreter acp`, verified reachable) — additive, seam unchanged. |
| **Telemetry console — the in-store capped sink + in-browser viewer** — the consumer half of observability: a FIFO-capped SurrealDB telemetry ring, a gated workspace-walled MCP read surface, and an in-browser console with filters + a live tail + an audit lane | observability | S10 | **shipped** | [telemetry-console](scope/observability/telemetry-console-scope.md) · [observability](scope/observability/observability-scope.md) | [telemetry-console](sessions/observability/telemetry-console-session.md) | New reusable store primitive **`lb_store::capped_insert`** (one verb file beside `write_tx`/`scan`): insert + FIFO-trim to newest `cap` per a caller-chosen key (per-source OR global) in **one SurrealDB transaction**, ULID insert-seq (no clock/counter). **Correct under concurrency = single-tx + per-`(ns,table,cap_key)` in-process lock + bounded retry-on-conflict** (the lock stops snapshot-interleaving over-eviction; the retry handles `kv-mem`'s retryable conflict the fire-and-forget Layer would otherwise drop → under-count) — the 100-insert-at-5×-cap test now lands EXACTLY `cap`, deterministically ([debug](debugging/observability/capped-insert-overgrows-cap-under-concurrency.md)). **`lb-telemetry`**: `SurrealCappedLayer` (a `tracing-subscriber` Layer, peer to stderr/OTLP, config-selected by `LB_TELEMETRY_SINK` in `node/main.rs` — no `if cloud`) that filters to `target==lb.telemetry` (so SurrealDB's own logs / the sink's own write queries don't pollute the ring), writes the redacted schema (`params_digest` only — a planted secret reaches ZERO rows), and mirrors onto the ws-walled tail subject; `Secret<T>` + `params_digest` redaction; `SinkConfig`. **Host read surface** (`crates/host/src/telemetry/`, gated new `telemetry:read`, HARD-filtered to caller's `ws` server-side): `telemetry.query` (filter source/actor/level≥/outcome/trace_id/text/time, seq-paged) · `telemetry.trace` (one correlated trace) · `telemetry.tail` (live feed; rides SSE, opaque `Denied` before the bridge for an ungranted caller, `NotFound` for a granted bridge call) · node-admin `telemetry.purge`. NO `telemetry.write`. **Gateway** `routes/telemetry_stream.rs` (`GET /telemetry/stream?token=`, snapshot-then-live, 403 before body; modelled on `run_stream`); `member_caps` grants `telemetry.read` (NOT `audit.query` → the dev session exercises the degraded audit lane). **UI** `lib/telemetry/` (query/trace/purge over `mcp_call` + SSE tail) + `features/telemetry/` (folder-of-verbs: `useTelemetry` snapshot+live-fold+trace-pivot, `filterUrl` shareable codec, `TelemetryFilterBar`, `TelemetryList` click-trace→timeline, `AuditLane` labelled-degraded, `TelemetryView`); Telescope nav surface gated on `telemetryRead`. Audit lane reads a SEPARATE store, never merged into the ring; audit unshipped → honest labelled empty state, no fake rows. Tests (real `mem://`+bus+gateway, no mocks): `lb-store` capped **6** (FIFO · per-source-vs-global · **concurrency exact-cap** · zero-cap-clamp · body round-trip) · `lb-telemetry` redact/secret/layer · host `telemetry_test` **7** (deny-per-verb opaque · ws-isolation · planted-secret→zero-rows · filter-narrow · trace-correlate · tail Denied-vs-NotFound) · UI `filterUrl` **6** (codec round-trip + min-severity + invalid-drop) · UI `TelemetryView.gateway` **4** (filters narrow · cap-deny · ws-isolation · **live SSE row** over real gateway). `cargo build --workspace`/`fmt` clean; `pnpm test` 186/186. **Test-infra fix:** the telemetry test harness made deterministic (leaked harness runtime + shared `Node` + capture-then-await the real Layer write — no global-subscriber/spawn races). **Deferred (named):** OTLP peer sink + cross-Zenoh-hop trace propagation + metrics (the emit half, still scoped) · amortized trim cadence · cross-node console reads · the cross-tenant operator console (a higher capability). Pre-existing shared-gateway flakes (SystemView/ProofPanel/App, vary run-to-run, the many-peers class) are unrelated to this slice. |
| **Flows — Wave 3 surfaces (React Flow canvas + dashboard↔flow binding)** — the typed-node editor generalising the chain canvas + the Cooler-Control round-trip | flows | post-S8 | **shipped** | [flows-canvas](scope/flows/flows-canvas-scope.md) · [dashboard-binding](scope/flows/dashboard-binding-scope.md) | [flows-canvas](sessions/flows/flows-canvas-session.md) · [dashboard-binding](sessions/flows/dashboard-binding-session.md) | Pure **client** of the shipped `flows.*`/`flows.nodes` verbs — **no new host work, no new caps, no new tables**. **Gateway** (`role/gateway/src/routes/flows.rs`, mirrors `chains.rs`): one route per `flows.*` verb, each re-checking `mcp:flows.<verb>:call` server-side via `lb_host::call_tool` (ws+principal from the token); an invalid DAG / schema-invalid node config → `400` inline. Dev `member_caps` gained the `mcp:flows.*` set (member-level); UI `CAP.flowsList` gates the nav. **UI client** (`lib/flows/`): `flows.types.ts` + `flows.api.ts` (1:1 verbs) + the `flows_*` http command mapping. **Canvas** (`features/flows/`, one component/hook per file): `flowGraph.ts` (canvas⇄record 1:1 + colour/executed-node math), `SchemaForm.tsx` (JSON-Schema 2020-12 → shadcn primitives, `ajv` validation — **no per-node hand-coded form**, fails loud on an unsupported schema), `Palette.tsx` (grouped by category incl. ext `[[node]]`), `FlowNodeView.tsx`, `NodeConfigPanel.tsx` (Save/Patch gated on ajv validity + the executed-node-lock), `useFlowRun.ts` (bounded `runs.get` poll + `runs.list{status:"active"}` reattach on open), `FlowCanvas.tsx` (edit/save-new-version/run/suspend/resume/cancel/`patch_run`/import/export/undo + the v-pinned banner), `FlowRail`/`useFlows`/`FlowsView`; the `flows` core surface registered (NavRail+routing+`allowed`+`surface`). **Dashboard binding** (Slice F): `flows.inject` is one more granted action tool a control calls through the shipped `WidgetBridge` (`/flows/{id}/inject`); a flow-node series (`flow:{ws}:{flow}:{node}`) is one more `series.watch`/`series.read` source — **no new dashboard mechanism**. **Tests (real, no mocks):** `SchemaForm.test.tsx` **8** (ajv accept/reject + render) · `FlowsCanvas.gateway.test.tsx` **13** (palette real built-ins+ext node, save round-trip, invalid-DAG inline reject, schema-invalid reject, run→runs.get colours, import/export round-trip, **undo restores node+edges atomically**, ws-isolation, **cap-deny**, inject-retain no-run, patch_run, delete idempotent) · `FlowDashboardBinding.gateway.test.tsx` **5** (slider→inject→retain `fired_run:false`, next run reads it, **viewer deny at bridge+host**, ws-isolation, flow-node series read-out) · Rust `flows_routes_test` **7** (nodes built-ins, CRUD, cycle→400, run snapshot, save deny, ws-iso opaque 403, inject-retain). `cargo build/test/fmt --workspace` green · `pnpm test` 176 · `pnpm lint` 0 errors. **Deferred:** `flows.watch` SSE (removes the poll) · host-side `flows.save` journaling (canvas undo is client-side until it lands) · mqtt native sidecar · subflow reactor-driven park · cross-node owner failover (Decision 10). |
| **Flows — node-graph engine (backend spine, Waves 1–2)** — node descriptor + merged `flows.nodes` registry · durable run engine over `lb-jobs` (frontier + CAS + version-pin + patch_run + subflow) · extension backend nodes (caller∩install-grant, source arm/disarm, mqtt reference) · triggers/lifecycle (5 kinds, `react_to_flows_cron` + `reconcile_flows`, placement, guarded teardown) | flows | post-S8 | **shipped** | [flows](scope/flows/README.md) | [node-descriptor](sessions/flows/node-descriptor-session.md) · [flow-run](sessions/flows/flow-run-session.md) · [ext+triggers](sessions/flows/extension-triggers-session.md) | New **`lb-flows`** pure crate (the `NodeDescriptor` keystone contract + `[[node]]` manifest block parse/validate + five built-ins + merged `flows.nodes` registry derived from `install` records + JSON-Schema 2020-12 config gate + typed `Flow` graph model + DAG math + the chain binding grammar + the canonical `coalesce` enum). **Run engine** (host `flows/`, one verb/file): `save`/`get`/`list`/`delete` (DAG + every node config validated; version bumped on edit — Decision 1), `run`/`resume`/`suspend`/`cancel`, `patch_run` (config-only to an UNEXECUTED node, validated against the PINNED schema — Decision 12), `runs.get`/`runs.list` (reattach), `enable`/`inject` (Decision 9 retain-vs-fire). A run = `flow_run` coordinator (pins `flow_version`) + one `flow-step` `lb-jobs` job per node, the `chains` frontier driver ported verbatim (Decision 8 — CAS exactly-once `Enqueued→Running`, `Halt`/`Continue`, concurrent branches as independent jobs), subflow parks on a pinned child run (Decision 11), `ResumePointDrift`. **Extension nodes**: descriptor-aware dispatch under `caller ∩ install-grant` (the shipped `build_call_context` — two-direction deny, no widening); source shape host-owns `flow:{ws}:{flow}:{node}` series + arm/disarm (Decision 2); worked `mqtt/extension.toml`. **Triggers**: `react_to_flows_cron` (deterministic firing id, fire-once-then-skip) + `reconcile_flows` (single-owner election by placement-as-data, arm/disarm, guarded teardown — Decision 13). Manifest addition `[[node]]` (additive, the §11.2 gate) in `lb-ext-loader`; `Install.nodes` propagated at install (wasm+native). **Composition, never widening:** `flows.run` + every node-tool's own gate under `caller ∩ grant`. Tests (real `mem://` store + jobs/caps/outbox/install records, no mocks): lb-flows **26** · ext-loader **16** · host **30** (flows_nodes 5 · flows_run 12 · flows_ext 5 · flows_triggers 8) — capability-deny per verb + the no-widening run gate + workspace-isolation + resume/cron-replay exactly-once + ResumePointDrift + subflow-parks-on-child. **Deferred (Wave 3 + named follow-ups):** React Flow canvas + dashboard↔flow binding · mqtt native sidecar binary · `flows.watch` SSE (canvas polls `runs.get` until it lands) · subflow reactor-driven park · cross-node owner failover (Decision 10) · the formal `chains.*`→flows alias. |
| **Global identity / many-workspaces (the Slack model)** — one global identity per person in `_lb_identity`, a per-workspace `membership` roster, login resolves identity→memberships→token | auth-caps | post-S10 (core-auth-caps) | **shipped** | [global-identity](scope/auth-caps/global-identity-scope.md) | [global-identity](sessions/auth-caps/global-identity-session.md) | New `lb_authz` raw: `identity.rs` (`_lb_identity` namespace, `identity_create/get/list`) + `membership.rs` (per-ws `membership:{sub}`=`{sub,joined_ts}`, tombstone-on-leave, `membership_add_raw/remove_raw/get/list/is_member/has_any`). New host services (one verb/file): **`identity/`** `create/get/list/workspaces` (cap `mcp:identity.manage:call`); **`membership/`** `add/remove/list` + un-gated `membership_login_resolve` seam (cap `mcp:members.manage:call`). `membership.add` writes the row AND system-grants `role:member` (raw `grant_assign`, not the gated host verb — a join is not a widening); `membership.remove` tombstones AND **composes** the shipped `revoke_subject` + `token_revoke_mark` (clean exit — live token refused next verify); `membership.list` returns the **effective** roster (membership ∪ legacy `user:*` rows — lazy migration #10). `create_workspace` gains the first-member bootstrap (auto-membership + `role:workspace-admin`). **Login** resolves identity→memberships→the existing token: effective member mints; an empty ws bootstraps the requester (decision #3, the dev-login realization); a non-member of a ws that has members is refused "not a member of any workspace" (decision #4). Identity lazy-created on first touch. `Subject::User(sub)` grant store UNCHANGED (#6 — `sub` stays `user:ada`). Gateway routes `/admin/identities*` + `/admin/members*`; `http.ts` cases; `member_caps()`+`admin-caps.ts` carry the two new caps. UI: Access console **People tab re-points** `user_list`→`membership.list` (decision #9); workspace switcher resolves through `identity.workspaces`. Tests (real store/gateway, no mocks): host `identity_membership_test` **7** (deny-per-verb · ws-isolation store+MCP · one-identity-in-N-ws · login/zero-memberships · leave-clean-exit · legacy-migration no-access-change · removed-tombstone-replays-idempotent) + gateway `identity_routes_test` **5** (forged-denied · create+add+list+workspaces · login bootstrap+refuse · clean-exit live-token-refused) + UI `Membership.gateway.test` **4** + re-pointed PeopleAdmin/DocView green. `cargo build --workspace` green; `cargo test -p lb-authz -p lb-host -p lb-role-gateway` green; `pnpm test` 168/168; `pnpm lint` 0 errors; `tsc`+`build` green. `pnpm test:gateway`: slice tests green/stable; pre-existing SystemView bus-peer-count flake (fails w/ and w/o this change, in isolation) + intermittent timing flakes in the shared serial gateway remain — not introduced here. **Non-goals deferred:** org/tenant tier · OIDC/SSO `cred_ref` · multi-hub identity sync · `bus.watch` membership motion. |
| **Saved PRQL queries (`query.*`)** — author once in PRQL, save as an editable `query:{ws}:{id}` record, run against any source | query | post-S8 | **shipped** | [prql-query](scope/query/prql-query-scope.md) | [prql-query](sessions/query/prql-query-session.md) | Phase 1 only. New pure crate **`lb-prql`** (`crates/prql/`, wraps pinned `prqlc` 0.13, zero I/O, `compile(prql,dialect)->sql`; 11 goldens). New host service **`query/`** (sibling to `federation/`+`rules/`, one verb/file): `save`/`get`/`list`/`delete`/`run`/`compile` over `query:{ws}:{id}`, each its own cap. `query.compile` is a pure dry-run (own cap, no data); `query.run` compiles for the target's dialect and **dispatches to the engine that already owns the wall** — `store.query` (platform, `Generic` SQL through the read-only parse-allowlist) or `federation.query` (datasource, dialect from `datasource.kind`). **`query.run` composes, never widens** (rule 5): needs `mcp:query.run:call` AND the target's underlying cap (`store.query`/`federation.query`) — checked before compile/resolution; holding `query.run` alone is denied. `lang:"raw"` is the SurrealQL/SQL escape hatch; `$var` binds through the engines' real param paths (injection-safe; datasource params = typed error in v1 — no sidecar bind path yet). Rules seam: `source("query:<name>")` → `query.run` under `caller ∩ grant`. Descriptors (`save`/`run`/`compile`) registered with `x-lb` widgets. Tests (real stack, no mocks): lb-prql 11 goldens + **host `query_test` 10** (cap-deny per verb · the two HEADLINE no-widening denies · ws-isolation · compile + malformed-PRQL · read-only gate rejects a `raw` write · injection-safe param binding + missing/extra typed errors · save→get→edit→save→run on real SurrealDB rows · rule `source("query:<name>")` · datasource round-trip vs a REAL spawned Postgres reusing the federation rig). `cargo build/test --workspace` green + fmt. **Deferred:** Phase 2 (DataFusion-over-`store.query`-reads for full Surreal PRQL) · datasource param binding (sidecar follow-up) · UI Queries page · folders/tags · dashboard/channel binding. |
| **Channels `/` + `@` command palette** — one reusable catalog-driven, capability-filtered `CommandPalette` (the SQL `/query` is its first tenant) | channels | post-S8 | **shipped** | [command-palette](scope/channels/channels-command-palette-scope.md) | [command-palette](sessions/channels/channels-command-palette-session.md) | Host **`tools.catalog`** verb (`crates/host/src/tools/`: `catalog.rs`/`descriptor.rs`/`tool.rs`) gated `mcp:tools.catalog:call`, reached via `POST /mcp/call` (`tools.` dispatch arm) AND `GET /mcp/catalog`; returns, for the calling principal, only the tools it is authorized for (runs the SAME `authorize_tool` gate the call would — no catalog↔call drift), each as `{name,title,group,input_schema?}`. Registry widened `Vec<String>`→`Vec<ToolDescriptor>` (additive — manifest `input_schema` field, host-native via per-verb `descriptor()`; absent schema → free-text arg, old extensions still appear). **UI** (`ui/src/features/channel/palette/`, one responsibility per file): `useCatalog` (fetched ONCE on mount + cached → `/` opens 0ms, revalidate on focus/reconnect/grant), `useMentions` (`@` entity listers over existing verbs, SWR), pure `parsePalette` (keystroke→structured `{tool,args}` / kind-tagged Item; `/`=command, `@`=mention, modes reclassify, fuzzy best pre-selected — host NEVER parses chat text), `argWidgets/EntityPicker` (auto-opens for an `x-lb` entity arg) + `argWidgets/SqlArg` (schema-aware SQL autocomplete from the `useDatasourceQuery` discovery SELECTs, cached per source), `CommandPalette` (RENDER-only `role="listbox"`, keyboard `/ @ ↑ ↓ ⏎ ⌫ Esc Tab`, whole-chip delete), wired into the composer (submit emits the structured Item via `channel.api.ts`, never raw `/`-text). Shared TS types mirror the Rust shapes exactly (`ToolDescriptor`/`ToolsCatalog`; `parsePayload` union). Tests (real gateway, no mocks): host `tools_catalog_test` 4 · gateway `gateway_routes_test` catalog HTTP 200/403 + cap-filtering · UI `parsePalette.test.ts` 11 + `CommandPalette.gateway.test.tsx` 6 (catalog ONE fetch + 0ms open · capability-filtered two seeded principals, no `/query` for the no-cap one, no existence leak · keyboard round-trip emits structured payload). `cargo test -p lb-host -p lb-role-gateway` + `pnpm test` 167 + `pnpm test:gateway` 175 green; UI was already built — this session audited+verified it. |
| **Channels in-channel SQL query → auto-plotted charts** — post a `kind:"query"` Item → a host worker runs `federation.query`, persists a `kind:"query_result"` Item with columns/rows + an auto-picked chart, streamed over SSE | channels | post-S8 | **shipped** | [query-charts](scope/channels/channels-query-charts-scope.md) | [query-charts](sessions/channels/channels-query-charts-session.md) | The palette's first tenant. Kind-tagged payloads (`channel/payload.rs`: `query`/`query_result`/`query_error` inside the existing `body`, no Item migration; untagged stays chat) + pure host-side chart picker (`channel/chart.rs`: temporal x→line, categorical x+numeric→bar, single-numeric-many-rows→histogram, else `chart:null`) + the **inline** query worker (`channel/query_worker.rs`, runs in `channel::post` on a `kind:"query"` item — one post→one execution, idempotent, re-entrancy-guarded; runs `federation.query` UNDER THE POSTER'S principal so a member without the datasource grant is denied; caps 500 rows/256 KB with a `truncated` flag; posts the result/error under `system:query-worker`; deny + missing-source collapse to opaque "query not permitted"). Two grants checked in order (channel `pub`, then the datasource grant). UI renders query→a chip, query_result→chart-first with a ⊞ table toggle (chart:null→table-only, "showing first N rows" when truncated), query_error→inline error. Tests (real store/bus/gateway + the REAL sqlite federation sidecar, seeded via the real write path, no mocks): host `chart`/`payload`/`query_worker` units (incl. the new `keyed_rows` regression) · gateway `gateway_routes_test` query_error round-trip (deny path) · **new `gateway_query_test.rs`** — the **happy-path** round-trip (post a `kind:"query"` → `query_result` with `columns:["day","signups"]`/4 rows/a non-null **line** chart in history AND streaming over SSE, against a real seeded sqlite source) + the **workspace-isolation** query path (a ws-A source name from ws-B → opaque `query_error`, no ws-A data leaks). `cargo build --workspace` + `cargo test -p lb-host -p lb-role-gateway` + `cargo fmt` clean; `pnpm test`/`pnpm test:gateway` green. **Bug fixed this session:** every `query_result` came back `chart:null` — `federation.query` returns column-aligned ARRAY rows but the chart picker keys by column name; fixed with `query_worker::keyed_rows` (zip arrays→objects for the picker, persist arrays), regression unit + the end-to-end gateway test ([debug](debugging/channels/query-result-chart-always-null.md)). |
| **Channels in-channel agent → ask an agent, get an answer in the channel** (v1) — post a `kind:"agent"` Item → a host worker drives an agent **run** through the `AgentRuntime` seam and posts a `kind:"agent_result"`/`agent_error` back; the **external** Open Interpreter agent is driven **for real against Z.AI GLM-4.6** | channels | post-S10 | **shipped (v1)** | [channels-agent](scope/channels/channels-agent-scope.md) | [channels-agent](sessions/channels/channels-agent-session.md) | Sibling of the query worker. **Kind-tagged payloads** (`channel/payload.rs`: `agent`/`agent_result`/`agent_error` inside `body`, additive; mirrored in `ui/.../payload.types.ts` + `encodeAgent`). **Inline agent worker** (`channel/agent_worker.rs`, runs in `channel::post` on a `kind:"agent"` item — re-entrancy-guarded; drives `invoke_via_runtime` UNDER THE POSTER'S principal with `agent_caps = poster.caps()`; posts result/error under `system:agent-worker` as `a:<job>`; caps the answer 256 KB; missing `mcp:agent.invoke:call` AND unknown runtime collapse to opaque "agent not permitted"). **Runtime registry now on the `Node` spine** (`boot.rs`: `runtimes: Mutex<Arc<RuntimeRegistry>>`, `runtimes()`/`install_runtimes()`; default-only over a new `UnconfiguredModel` — the honest "no in-house model wired" state; `boot_on_bus` replaces 3 test struct-literals). **`node` binary** `external_agent::install` (feature-ON installs default + external `AcpRuntime` entries, OFF is a no-op). **External-agent fix:** the codex wrapper now emits Z.AI `model_providers` `-c` overrides incl. the load-bearing `wire_api=chat` (codex defaults to `/responses`, 404 on Z.AI); `ModelEndpoint.base_url` added; default provider id `zaicoding` (not the throttled built-in `zai`). **UI:** `AgentCard` (running→answer→opaque error), `MessageItem` routing + `MessageList` settled-run hiding, ~~`/agent [@runtime] <goal>` composer command~~ (**superseded** — the agent is now a first-class `agent.invoke` `/` palette command with a runtime dropdown; the `/agent`-string path was orphaned on the unrendered `MessageComposer` and is DELETED — see the `agent-runtimes-picker` row below). **LIVE RUN-FEED:** the external driver now streams each `RunEvent` **per-line** (`drive` gained an optional sink; `AcpRuntime::run` forwards them onto the ws-walled run subject via a detached publisher task) instead of a burst at the end; UI `run.stream.ts` (`openRunStream` → `GET /runs/{job}/stream`) + `useRunFeed` fold events into the card so you watch tool-calls (spinner→✓/✗), reasoning, and text **live** while the run is pending; `postAgent` is non-blocking (fire-and-forget — request/feed/answer all arrive over SSE); a failed run surfaces the terminal reason (e.g. `429`) in `agent_error` via `finish_message` instead of an empty string. Tests (rule 9 — real store/bus/loop/channel; only the model-provider HTTP ever stubbed): host `payload`/`agent_worker`/`codex` units · `channel_agent_worker_test` (5: happy-path, opaque capability-deny, opaque unknown-runtime, re-entrancy, ws-isolation) over the REAL in-house loop · **opt-in live e2e** `role/external-agent/channel_smoke_test` (`EXTAGENT_SMOKE=1`): channel post → worker → seam → Open Interpreter → **Z.AI GLM-4.6** → `agent_result` = "PONG" · UI `payload.test.ts` + ~~`parseAgentCommand.test.ts` (5)~~ (removed with the orphaned path) + `useRunFeed.test.ts` (6). `cargo fmt` clean; affected Rust suites (44 binaries) + feature-OFF/ON node builds + UI channel suite (42) green. **Follow-ups (named, not faked):** ~~background execution so the run detaches from the POST connection & survives tab-close (#5)~~ **→ SHIPPED, see the next row**; supervision (wall-time ceiling + kill/reap of a hung run) is the remaining #5 half; external-agent #3 wall + #4 model-routing before any *production* external run; stream the in-house loop's per-token deltas too. |
| **Channels in-channel agent → background/detached execution** (run-lifecycle #5, the biggest-value half) — the in-channel agent run is no longer inline in `channel::post`; it is a **durable enqueue job drained by a background reactor** off the POST connection, so it survives tab-close AND a node restart, and is idempotent | channels | post-S10 | **shipped** | [channels-agent](scope/channels/channels-agent-scope.md) · [run-lifecycle #5](scope/external-agent/run-lifecycle-scope.md) | [channels-agent-background](sessions/channels/channels-agent-background-session.md) | **Option A** (durable job + reactor), mirroring `spawn_flow_reactors` — chosen over a bare `tokio::spawn` (Option B) because #5 wants durable+resumable, not just detached. **`lb-jobs`**: new `pending(store, ws, kind)` drain-scan (`crates/jobs/src/pending.rs`) — lists ws-namespaced jobs of a kind that are still `is_resumable()` (a terminal job is drained, never re-driven); bounded to one `MAX_SCAN_LIMIT` page. **`channel/agent_job.rs`**: `ChannelAgentJob{cid,goal,runtime?,run_job,poster_sub,poster_caps,ts}` — the durable enqueue payload; carries the poster's sub+caps so the reactor reconstructs the poster via `Principal::routed` (same co-trust rebuild as the routed-agent hub — in-process, ws-scoped, never widens); ids `q:<run_job>` (enqueue, distinct from the `<run_job>` agent-session run job) + `a:<run_job>` (answer/idempotency key). **`channel/agent_worker.rs`**: `run_if_agent` now ENQUEUES (writes the durable job, returns at once) instead of driving; `drive_queued_run` is the drained drive (same opaque/honest split, 256 KB cap, `invoke_via_runtime` under the reconstructed poster, posts under `system:agent-worker`) and **short-circuits idempotently** when `a:<run_job>` already exists (no re-run/re-spend/double-post). **`agent_reactor.rs`** (twin of `spawn_flow_reactors`): `spawn_agent_reactors` (production — spawns a drive per pending job so a long run never stalls the tick; two no-double-drive guards: in-process `in_flight` set + the durable `a:<run_job>` check) and `drain_channel_agent_runs` (synchronous flush for tests / immediate drain), over one shared `scan_drivable` (retires a malformed record as `Failed`). Wired at boot in `node/src/main.rs` beside `spawn_flow_reactors` (2 s tick, configured ws). Tests (rule 9 — real store/bus/loop/channel/**job-queue**, only the model HTTP stubbed): **lb-jobs `pending_test` 2** (kind+status filter · ws-isolation) · **lb-host `agent_job` units 3** · **`channel_agent_worker_test` 6** over the REAL in-house loop + REAL reactor drain: **post returns before the run completes** (result absent right after `post`, appears only after the drain), happy-path, **idempotency (double-drain → one result)**, opaque capability-deny, opaque unknown-runtime, re-entrancy, **ws-isolation (a ws-B drain never drives the ws-A run)**. `cargo test -p lb-jobs -p lb-host -j2` + `cargo build -p node` + `cargo fmt` green (the untracked WIP `role/cli` breaks `--workspace` resolution — touched crates build in isolation). **Remaining #5 half:** supervision (wall-time/iteration ceiling + kill/reap of a hung run → `agent_error`, not a stuck card) → **SHIPPED, see the next row**. |
| **Channels in-channel agent → wall-time supervision** (run-lifecycle #5, the remaining half) — a detached run is now **bounded**: a hung/looping run is reaped at a wall-clock ceiling and posts an honest `agent_error` instead of a card that spins forever | channels | post-S10 | **shipped** | [channels-agent](scope/channels/channels-agent-scope.md) · [run-lifecycle #5](scope/external-agent/run-lifecycle-scope.md) | [channels-agent-supervision](sessions/channels/channels-agent-supervision-session.md) | The ceiling lives at the **one drive seam** (`channel::agent_worker::drive_run`), wrapping the single `invoke_via_runtime` future in `tokio::time::timeout(ceiling, run)` — **not** inside the `AgentRuntime` trait — so it bounds every runtime uniformly (in-house *and* every external `AcpRuntime`) without each impl re-implementing a ceiling; the in-house loop already self-bounds via `MAX_STEPS`, this adds the wall-clock bound the external subprocess lacked. **Drop is the reaper:** on timeout the run future is dropped, which for `AcpRuntime` closes the ACP session + subprocess stdio → the child is reaped, no zombie pinning the job (an explicit ACP `session/cancel` for a *user-requested* stop remains open — this reaps a *hung* run). **Fail-closed:** the ceiling is host authority and overrides the run's eventual word → a run that blows the budget is `agent_error`, never a late success (the untrusted-agent posture #5 requires). `RUN_WALL_CEILING` = fixed 15 min node default (per-workspace policy deferred); `TIMEOUT_MESSAGE` is honest + distinct from the opaque deny (a timeout must not masquerade as an authorization signal). Seams: `drive_queued_run`/`drive_run` take `ceiling: Duration`; the production tick + no-arg `drain_channel_agent_runs` pass `RUN_WALL_CEILING`; new **`drain_channel_agent_runs_with_ceiling`** lets a test reap at a tiny wall-time. Tests (rule 9 — real store/bus/loop/channel/job-queue, only the model HTTP stubbed): `channel_agent_worker_test` now **7** — new `a_run_that_exceeds_the_supervision_ceiling_is_reaped_with_an_agent_error` (a `HungRuntime` whose `run` sleeps 3600 s, reaped by a 50 ms ceiling → `agent_error` = the timeout message, asserted **not** "agent not permitted"; a re-drain posts no second error — job retired terminal + `a:<job>` idempotency guard). Real short ceiling over a real run, not a paused virtual clock (the run does real embedded-SurrealDB I/O). `cargo test -p lb-host --test channel_agent_worker_test` (7) + `--lib channel::` (27) + `cargo build -p node` + `cargo fmt` green. **Remaining #5:** ACP `session/cancel` (user-driven stop), an iteration/token ceiling for the external subprocess distinct from wall-time, per-workspace ceiling policy, then resume (the `agent.runtimes` read surface + composer picker → **SHIPPED, see the next row**). |
| **Channels in-channel agent → `agent.runtimes` read verb + composer runtime picker** (run-lifecycle #5, the read surface) — the in-channel agent is now a **first-class `/` palette command** with a real **runtime dropdown** (not a typed `@id`); the orphaned `/agent`-on-`MessageComposer` path is removed | channels | post-S10 | **shipped** | [agent-runtimes](scope/external-agent/agent-runtimes-scope.md) · [run-lifecycle #5](scope/external-agent/run-lifecycle-scope.md) | [agent-runtimes-picker](sessions/channels/agent-runtimes-picker-session.md) | **The bug:** the rendered composer is the `CommandPalette` (via `ChannelView`), **not** the old `MessageComposer` — the palette's `/` menu listed only `tools.catalog` tools and its submit never built the `kind:"agent"` payload, so `/agent hey` showed "No commands match" and `postAgent`/`parseAgentCommand` were orphaned. **Three locked decisions:** (1) a **descriptor, not a special-cased string** — the agent is a real `tools.catalog` descriptor fuzzy-matched like `federation.query`; (2) the **catalog gates on the descriptor NAME `agent.invoke`** — `tools.catalog` keeps a tool only if `authorize_tool(principal, ws, <name>)` passes, so the run's existing `mcp:agent.invoke:call` gate decides visibility with **zero special-casing** (absent, not greyed — no new cap, no `if` in the catalog); (3) the runtime arg is a **dropdown backed by a new read verb** `agent.runtimes` — **minimal shape** `{ default, runtimes:[sorted ids] }` (health/version deferred). **Rust (host):** new `agent/runtimes.rs` `list_runtimes` (gate `mcp:agent.runtimes:call` → opaque `Denied`; registry-derived — no store read, so no cross-ws data structurally; `RuntimeRegistry::default_id()` added), the `"agent.runtimes"` dispatch arm in `agent/tool.rs`, `agent/descriptor.rs` `invoke_descriptor()` named `agent.invoke` (schema `{goal, runtime:{x-lb:{widget:"runtime"}}}`, `required:["goal"]`) collected into `host_descriptors()`; **granted `mcp:agent.invoke:call` (makes the command appear AND runs it — was absent from the dev member bundle) + the distinct read cap `mcp:agent.runtimes:call`** in `credentials.rs`. **UI:** `lib/agent/runtimes.api.ts`, `palette/argWidgets/RuntimeArg.tsx`+`useRuntimes.ts` (dropdown mirroring `SqlArg`, default preselected), the `"runtime"` `WidgetKind`, a general **text-arg** input in `CommandPalette` (the palette had no plain-text fill path — the agent's `goal` was unfillable), and the `agent.invoke` → **`onSendAgent`** submit route (NOT a raw tool call) threaded `ChannelView → useChannel.postAgent`. **Removed:** `parseAgentCommand` (+test) and the unrendered `MessageComposer.tsx`. Tests (rule 9 — real backends, no `*.fake.ts`): host `agent_runtimes_test` **5** (read-surface unit default-only + extra-runtime · capability-deny opaque · workspace-isolation · catalog-integration: `agent.invoke` present iff the invoke cap is held) + `tools_catalog_test` 4 green; UI `RuntimeArg.test` **3** + `pnpm test` **272**; real-gateway `CommandPalette.agent.gateway.test` **2** (capability-filtered command · accept → runtime dropdown + goal → post `kind:"agent"` → real `drain_channel_agent_runs` → `AgentCard` settles to the answer; new `/_seed/agent_drain` route calls the reactor's own `drain_channel_agent_runs`, the test gateway not spawning the timer). `cargo fmt` clean. |
| **Channel rich responses → the descriptor-driven render contract** (core) — a command/tool/agent answers with a `render` block in its channel `Item` body; the channel mounts it through the **shipped** dashboard `WidgetView`, leashed to the viewer's grant. The frontend is a **generic** MCP front-end: it names only `tools.catalog`, renders each command's schema widgets **by string**, and posts each command's **declared** render — **zero `tool.name` branches** | channels | post-S8 | **shipped** | [channels-rich-responses](scope/channels/channels-rich-responses-scope.md) | [channels-rich-responses](sessions/channels/channels-rich-responses-session.md) | **The design correction (headline):** the first pass leaked tool-specific knowledge into the palette (reshaping `reminder.create` args, hardcoding `reminder.list`'s render); corrected to **100% backend-driven**. New **`rich_result`** kind-tagged payload (`channel/payload.rs` + `payload.types.ts`, mirrored: `{kind,v:2,view,source?,data?,options?,action?,tools?}`, additive — a body with no recognized kind stays chat). New **`ToolDescriptor.result`** field (`crates/mcp/registry.rs`) — the `x-lb-render` **output** half of the contract (`skip_serializing_if=None`). UI **`ResponseView`** (builds a v2 `Cell` from the envelope → the shipped `WidgetView`; threads `installed` so `ext:<id>` response views mount) + **`ResponseTable`** (the one interactive-list piece — per-row `SwitchControl`/`ButtonControl` reused with the **row object as `VarScope.values`**, so `${id}` binds the row field and `{{value}}` the interaction — the shipped vars engine, no new templating slot). **Open widget registry** (`palette/argWidgets/registry.ts`) — UI built-ins (`cron`/`select`/`sql`/`entity`/`text`/`number`/`boolean`/`date`) **∪** `ext:<id>/<widget>` (the shipped `ExtWidget` federation), unknown→text. **Generic palette submit**: collect schema fields → `tool.result` present ? post `encodeRichResult(...args interpolated)` : `onCallTool(name, args)` — no reminder branch (`grep reminder` in the palette → 0 dispatch hits). Fixed-vs-generative tier unchanged (built-ins in-process, `template`/`plot`/`d3` sandboxed — no new in-process path for generated UI). `query_result` is now **expressible** as a `rich_result` (`view:"table"|"chart"`) with the old `QueryCard` path kept + a no-regression test. Arg-side ext widgets fall back to text (the shipped `mountWidget` contract has no value channel — honest limitation, documented); response-side ext widgets mount for real. Tests (rule 9): Rust `payload` round-trip + `ToolDescriptor.result` serialize; UI `pnpm test` (registry incl. `ext:<id>`, cron round-trip, generic-dispatch fixture, ResponseView mount/degrade). **Named follow-ups:** make the legacy `agent.invoke`/`federation.query` palette branches descriptor-declared routes too (finish the tool-agnostic palette); A2UI/JSON-render as an additional sandboxed view; pin-response-to-dashboard; live-updating card. |
| **Channel rich responses → reminders as the first tenant** — the whole reminders CRUD-and-list surface is drivable from a channel with **zero reminders-specific UI**: `/remind` is a backend-declared cron+action form; `/reminders` is an interactive table with pause/run-now/delete row controls — every pixel a shipped widget, gated + ws-scoped | reminders | post-S8 (over shipped reminders) | **shipped** (surface; one pre-existing fire bug deferred) | [reminders-rich-responses](scope/reminders/reminders-rich-responses-scope.md) | [reminders-rich-responses](sessions/reminders/reminders-rich-responses-session.md) | **Two descriptors + one new verb, all backend:** `reminder.create` descriptor declares the **flat** form (`schedule`→`x-lb:cron`, `action_kind`→`x-lb:select` over the 3 kinds, the action fields, `max_runs?`/`enabled?`) and the **verb accepts the flat form** (builds the nested `Action`, derives `id`, supplies `now` in seconds — server-side; the nested `action:{…}` form still works). `reminder.list` descriptor carries its **`result`** render (`view:"table"`, `source:{tool:"reminder.list"}`, `options.rowControls` = pause `switch`→`reminder.update{id:"${id}",enabled:"{{value}}"}`, run-now `button`→`reminder.fire{id:"${id}"}`, delete `button`→`reminder.delete{id:"${id}"}`, `tools:[list,update,fire,delete]`); the generic palette posts it verbatim. New **`reminder.fire`** verb (`reminder/fire_now.rs`) — gated `mcp:reminder.fire:call` (granted in the member bundle), idempotent on `(reminder_id, now)`, **reusing** the shipped internal `fire_reminder` path (not duplicated); a manual fire keys on `now` and does not advance the schedule. Tests (rule 9, real node): **`reminder_fire_test` 9** — fire deny/ws-isolation/idempotency, **flat-form create (no nested action, no id)** + nested-form back-compat, catalog shows `reminder.create`/`.list`/`.fire` only with their cap (+ `list` carries its render), and the `ts`-default regression; UI real-gateway loop (create→`reminder.get`; list→table renders seeded rows; pause/delete drive real verbs; deny with `list`-not-`update`; two-session isolation; token-never-crosses). **Bugs surfaced by the real-gateway loop** (the loop earned its keep): (1) `ts` unit ms→s ([debug](debugging/reminders/ts-unit-mismatch-cron-search-limit.md)); (2) write verbs required `ts` while the generic controls send none — **fixed** (default to host clock, matching `create`) ([debug](debugging/reminders/reminder-write-verbs-require-ts.md)); (3) `reminder.list`'s `{reminders:[…]}` wasn't in `viz.query`'s `ROW_KEYS` unwrap set, so the table rendered one JSON-blob row not N — **fixed** (added `"reminders"` to both mirrors + regression) ([debug](debugging/reminders/reminder-list-not-unwrapped-to-table-rows.md)); (4) **pre-existing, deferred** — fire re-resolves the creator's caps from the **durable grant store**, so a **dev-login/token-only** reminder won't fire (run-now *and* the scheduled reactor); the run-now control is correct and works the instant the fire path is fixed ([debug](debugging/reminders/reminder-fire-reresolve-misses-token-caps.md), a named follow-up: persist member caps durably on login). |
| **Reminders** — a durable, workspace-scoped scheduled trigger that fires ONE action (channel post / MCP tool / outbox effect) on a 5-field cron + optional `max_runs` + `enabled` switch | reminders | post-S8 (over S5 jobs + S6/S7 outbox + reactors) | **shipped** | [reminders](scope/reminders/reminders-scope.md) | [reminders](sessions/reminders/reminders-session.md) | New **`lb-reminders`** pure crate (record + raw store verbs `save`/`load`/`list`/`due` + the cron **`next_after`** on the injected clock via **`croner`** `find_next_occurrence(after, inclusive=false)` — never wall-clock). Host **`reminder`** service (one verb/file, each gated + deny-tested): `create`/`update`(pause-resume + reschedule)/`delete`/`get`/`list` (the scope's full MCP surface; live-feed + batch are explicit non-goals), `fire` (dispatch one action under the **stored principal, caps re-resolved at FIRE time** against its real seam — channel→`lb_inbox::Item`, mcp-tool→re-enter `call_tool`, outbox→`Effect`), and **`react_to_reminders`** — a dedicated durable scan (its own file, same altitude as `react_to_approvals`/`relay_outbox`) that finds enabled+due reminders, enqueues ONE `kind="reminder-fire"` job per firing (deterministic id from `(reminder_id, scheduled_ts)` → idempotent, one instant→one job→one effect), dispatches, then advances (decrement `max_runs`→`Done` at 0; recompute `next_attempt_ts`; **fire-once-then-skip** missed-firing, no backfill storm). A **revoked action grant → logged deny, reminder left scheduled, no effect, no escalation**. Wired store→cap→MCP→`/mcp/call`→`reminders.api.ts`→**`RemindersView`** (the `react-js-cron` builder wrapped + restyled to Tailwind/shadcn, antd kept out of the global theme; `ActionEditor`), registered as a core nav surface gated on `mcp:reminder.list:call`. Tests (real `mem://` store / bus / caps / jobs / outbox / gateway, seeded via the real write path, no mocks, no `*.fake.ts`): **lb-reminders 6** (cron next-after: multi-day, one-shot, every-minute, strictly-after) · **lb-host `reminders_mcp_test` 5** (per-verb deny · ws-isolation list/get · bad-cron-is-bad-input-not-denied · CRUD round-trip) · **`reminders_reactor_test` 10** (channel/mcp-tool/outbox firing each end-to-end against its real seam · revoked-grant logged deny · max_runs countdown→done · disabled skip/resume · idempotent re-scan · ws-B never fires/advances a ws-A reminder · due-during-outage fires exactly once on catch-up) · **frontend `RemindersView.gateway.test.tsx` 3** (create→list · pause/resume · delete tombstone, all real path). **Bug fixed this session:** the UI passed `Date.now()` (ms) as the host's logical `ts` (seconds) → croner "time search limit exceeded"; fixed at the seam (`nowSecs()`), regression-tested ([debug](debugging/reminders/ts-unit-mismatch-cron-search-limit.md)). |
| **API keys** — long-lived machine credentials (bearer `lbk_{ws}.{id}.{secret}` over the existing authz model) | auth-caps | core capability | **shipped** | [api-keys](scope/auth-caps/api-keys-scope.md) | [api-keys](sessions/auth-caps/api-keys-session.md) | New **`lb-apikey`** pure crate (HMAC-SHA256 peppered hash over the secret field alone + constant-time compare + Crockford-base32 id/secret + the `lbk_{ws}.{id}.{secret}` parse/format + the `apikey-read`/`apikey-write` role bundles + the list-view badge) + host **`apikey`** service (one verb/file: `create` returns the secret ONCE, `revoke` tombstone+cache-bust+grant-revoke, `rotate` new-secret-old-dead, `list`/`get` never carry a hash/secret, `authenticate` per-request verify→resolve→`Principal::for_key`, `ApiKeyCache` 5s TTL busted on revoke) + `lb-authz` `Subject::Key` variant + the generalized `resolve_subject_caps` (the load-bearing seam — a key resolves direct grants + roles, NO team edge) + gateway bearer-auth (the `authenticate` chokepoint now branches JWT vs `lbk_` API key, async) + `/admin/apikeys*` routes + `mcp:apikey.manage:call` + admin-console **API Keys** tab. Three locked decisions held: (1) key IS the bearer, verified per request (no token exchange), `Principal::for_key` (not `routed`), hash→principal cache busted on revoke; (2) permissions are grants on `Subject::Key`, two built-in roles, NO new grammar; (3) lazy expiry at auth (the outbox only tombstones + notifies — security never depends on a job). The **privilege-escalation guard** runs in `apikey.create` (effective resolved caps ⊆ creator's — covers the built-in-role path `grants_assign`'s `role:` exemption would miss). Tests (real store + real gateway, no mocks): lb-apikey **20** unit (hash round-trip, constant-time compare, secret-field-only hash, parse + reject malformed, lazy-expiry boundary) · lb-authz resolve_key **3** (incl. the zero-caps guard for a key through `resolve_caps(&str)`) · host cache **5** · gateway **8** (cap-deny per verb · escalation deny incl. the role path · read-only denied write · ws-isolation incl. forged-ws bearer · revoke idempotent · lazy-expiry now==/`>`expires_at · create→auth→allow→deny→revoke→refused · rotate old-dead/new-works · **cache-bust immediate not after TTL**) · UI Vitest `ApiKeysAdmin.gateway.test` **2** + `AdminView` cap-gate **4** (create-shows-secret-once · list renders no hash/secret · revoke→revoked · tab hidden without `apikey.manage`). Pepper from `LB_APIKEY_PEPPER` env (dev default per-process random). Multi-node revoke = local-bust floor + documented sync+TTL bound (the bus cache-bust broadcast is a deferred nicety). |

| **Access console** — the access-first rebuild of `/admin`: resolved effective caps WITH provenance, a live-token revoke lever, `roles.delete`, an overview + a no-widening capability picker | auth-caps | core capability | **shipped** | [access-console](scope/auth-caps/access-console-scope.md) | [access-console](sessions/auth-caps/access-console-session.md) | The admin console turned from a flat directory into an **access-management tool**. Three new verbs wired store→cap→MCP→gateway→`http.ts`→UI, each admin-only + deny-tested: **`authz.resolve`** (`mcp:authz.resolve:call`) = resolved effective caps WITH provenance via `resolve_caps_sourced` — the **provenance-tagging WRAPPER over the one shipped `resolve_caps`/`resolve_subject_caps` fold** (NOT a parallel resolver; `CapSource = Direct | Role{name} | Team{name}`), so the displayed set and the enforced (token) set cannot drift (cross-check pinned). **`authz.revoke-tokens`** (`mcp:authz.revoke-tokens:call`) = the **live-token revoke lever** — writes a per-`(ws,subject)` **`token_revoke` tombstone** the verify chokepoint reads every request (the subject's CURRENT token is refused on the next verify, single-node instant) AND **composes** with the shipped `revoke_subject` (grant-revoke, next re-mint); multi-node worst case bounded by TTL (stated, not "instant global"). **`roles.delete`** (`mcp:roles.manage:call`) = cascade-un-assigns `role:<name>` from every subject AND deletes the role in ONE store tx (new bounded `write_batch` — the generalization `write_tx` is a 2-upsert special case of); built-ins immutable (`400`); idempotent. The verify-path check lives in `session/authenticate.rs::verify_token` (one read, opaque `401`, no oracle). UI (shadcn-first): new `tabs`/`table`/`select` primitives (token-bound); `AdminView`+`AccessEditor` rebuilt onto `AppPageHeader`+Tabs (removed from LEGACY_VIEWS); an **Access overview** landing (People/Teams/Roles/Keys + direct-grant subjects + keys-expiring<7d + admin-cap-holders; honest counts, hide-with-reason when a verb is absent); `EffectiveCaps` provenance detail; a catalog-driven no-widening **`CapabilityPicker`** (raw string demoted to Advanced); the **`RevokeTokensLever`** on revoking actions. The 5 scope open questions all resolved (tombstone-record revoke · client-side "who can do X" · the security-posture tile set · cascade roles.delete · no audit fields). Tests (real infra, no mocks): lb-authz `access_console_test` **5** (no-drift cross-check · provenance tags · key resolve · token_revoke round-trip · role_delete cascade idempotent) · lb-host `authz_test` +2 (per-verb deny · ws-iso at the bridge) · gateway `access_console_routes_test` **5** (forged-call 403 · resolve provenance · **revoke_tokens refuses the prior token on next verify** · roles.delete cascade+built-in+idempotent · ws-iso resolve-empty/delete-nothing) · UI `AccessConsole.gateway.test` **6** + RolesAdmin delete; `pnpm test` 168/168, `pnpm test:gateway` 179/180 (1 pre-existing SystemView flake), `pnpm lint` 0 errors, `tsc` clean, `pnpm build` green, `cargo fmt` clean. Follow-up: full shadcn migration of the remaining admin view *bodies* (still in LEGACY_VIEWS). See `public/auth-caps/access-console.md`.
| **`lb-prefs` units + formatting core** — the canonical-in/localized-out preferences + unit-conversion + locale/tz formatting library the dashboard fieldConfig depends on | prefs | core crate (S8 unit add-on) | **shipped** | [user-prefs](scope/prefs/user-prefs-scope.md) | [lb-prefs](sessions/prefs/lb-prefs-session.md) | New **`lb-prefs`** crate (one verb/file): the **closed axis set** (8 dimensions, 29 units each tied to one dimension, date/time/number/unit-system/first-day/language enums); nullable `user_prefs:[ws,user]` + `workspace_prefs:[ws]` SCHEMAFULL records (composite-id MERGE upsert, LWW, offline-idempotent); the **pure resolution fold** (request→user→ws-default→builtin, each axis independent, overrides merge per-dimension). **Conversion = `uom`** (affine °C↔°F verified; cross-dimension rejected at the type level). **Rendering** = locale separators + date order + 12/24h from the closed axes, **tz over a UTC instant incl. DST via `chrono-tz`**; `format.quantity` is the chart bridge (`12 m/s`→`43,2 km/h` es / `23.3 kn` en+knots). **`icu4x` deferred to Phase 2** (localized month/unit names + the MessageFormat plural/select engine) behind the same `format::*` signatures — the closed axes carry the numeric styles with zero CLDR data-size cost (satisfies the Pi-profile risk directly); **MessageFormat dialect now pinned: ICU MF1 / `intl-messageformat`**. **8 MCP verbs**: gated `prefs.get/set/resolve/set_default` (OWN forced to caller `sub`; `set_default` admin-gated) + **grant-free** `format.datetime/number/quantity`+`convert.unit` (pure math, no tenant data — dispatched before the host-native authorize gate). Gateway routes 1:1 (`GET/PUT /prefs`, `POST /prefs/resolve`, `PUT /prefs/default`, `POST /format/*`+`/convert/unit`). Generated client constants `ui/src/lib/prefs/dimensions.generated.ts` (drift-tested). Tests (real store + MCP + gateway, seeded, no mocks): **lb-prefs 30** (convert 7 · format 8 incl. DST · resolve 6 · store 5 · isolation 2 · generated_ts 2) + **lb-host 8** (prefs_deny 5 · prefs_mcp 3) + **gateway 5** (prefs_routes); `cargo build --workspace` + `cargo fmt` clean; **all existing host/gateway suites still green**. **Clears the prefs blocker noted on the viz row** — `lb_prefs` is now a workspace member, host+gateway link it, the gateway lib compiles. Deferred (named): i18n catalogs + per-recipient fan-out **([now SHIPPED — Phase 2](sessions/prefs/i18n-catalogs-session.md); see the row below)**, icu4x swap-in + en/es CLDR slice, settings/bootstrap-locale UI. |
| **`lb-prefs` i18n catalogs (Phase 2)** — MF1 MessageFormat catalogs + per-recipient server localization on the shipped renderer | prefs | Phase 2 (rides `lb-prefs`) | **shipped** | [i18n-catalogs](scope/prefs/i18n-catalogs-scope.md) | [i18n-catalogs](sessions/prefs/i18n-catalogs-session.md) | Adds a `catalog::*` module to the shipped `lb-prefs` crate + 3 host verbs + gateway routes — **no new crate, no SDK/WIT change**. **Hand-written ICU MF1 subset parser/renderer** (`catalog/message.rs`+`plural.rs`+`interpolate.rs`): argument, `plural` (`one`/`other`+exact `=0`/`=1`), `select` (arbitrary keywords + mandatory `other`), typed `{ts,date}`/`{n,number}`/`{v,quantity,<dim>}` placeholders **dispatched to the shipped `format::*`** (never re-derived; a `{v,quantity,dim}` converts from `Dimension::canonical_unit()`), one nest, the `#` count token, `'{'`/`'}'` escapes — anything outside is a **catalog-lint error** (rejected on write + a build-time test), never a silent parse. **en/es plural per CLDR 44** (`n==1→one` else `other`; the flagged icu4x swap point). **Placeholder failure → the literal `[<arg>]`** (never panics, never blank); fallback chain **override→builtin(lang)→builtin(en)→key**. **Built-in en/es `.mf` catalogs** compiled in (`include_str!`, `catalog-version:` header) + **generated to the client** `ui/src/lib/prefs/catalog.generated.ts` (`gen-prefs-catalog`, twin of `gen_ts`) — byte-identity drift-tested; the client renders the SAME MF1 with `intl-messageformat` (a **cross-check test** asserts host==client byte-for-byte). **Per-workspace override** `message_catalog:[ws,locale]` (SCHEMAFULL, flat dotted keys→MF1; **per-message-key** read-merge-write so two offline edits to different keys both survive; composite id→idempotent replay; same-key LWW). **3 MCP verbs** wired store→cap→MCP→gateway→client: `message.render(key,args,recipient?)` (member for SELF; recipient≠self needs the `mcp:message.render_recipient:call` fan-out grant — like `prefs.get(other)`), `prefs.catalog(locale)` (member — the merged override-over-builtin map), `message.set_catalog(locale,messages)` (**admin**, beside `prefs.set_default`; lints then publishes the `ws/{ws}/prefs/catalog-changed` hint). Render is **gated** (a catalog carries tenant overrides — unlike grant-free `format.*`). Gateway routes 1:1: `POST /message/render`, `POST /prefs/catalog`, `PUT /message/catalog`. **Server-side per-recipient fan-out**: one canonical event → **N distinct renders** (per member's resolved prefs; a team has no language of its own). Tests (real store+MCP+gateway, seeded via the real write path, no mocks): **lb-prefs 15** (catalog 13 incl. **placeholder-parity** datetime+quantity byte-identical to direct `format::*`, plural/select en+es, fallback, placeholder-failure `[ts]`, lint accept/reject; generated_catalog 2 drift) + **lb-host `catalog_mcp_test` 8** (deny per verb incl. **render-for-another denied without the fan-out grant** · two-ws distinct overrides · offline replay + per-key merge · **the fan-out headline** es `43,2 km/h`/Madrid vs en `23.3 kn`/New-York · lint→BadInput) + **gateway `catalog_routes_test` 7** (set→render override · fan-out-needs-grant 403 · admin-deny · out-of-subset 400≠403 · two-session per-ws · fan-out over the gateway) + **UI `renderMessage.test.ts` 7** (the intl-messageformat cross-check). `cargo build --workspace`+`cargo fmt` clean; `pnpm test`+`pnpm test:gateway` green. Deferred (named): the **icu4x swap-in** (localized names + full-CLDR plural for `pl`/`ar` + en/es CLDR slice) behind the same signatures; the client **settings/bootstrap-locale + RTL UI**. |
| Grafana-compatible visualization (`viz/`) — **Phase 1: `timeseries` end to end** (additive v3 panel model + the `timeseries` renderer + the fieldConfig→user-prefs bridge + the one add≡edit panel editor) | frontend | S9+ | **shipped** | [viz](scope/frontend/dashboard/viz/README.md) | [dashboard-viz-phase1](sessions/frontend/dashboard-viz-phase1-session.md) | Additive over the shipped v2 cell — no v1/v2 break. **Spine:** `Cell` gains serde-default v3 fields (`sources[]` Target[], `fieldConfig`, `transformations` **config-only** — pipeline is backend-resolved in Phase 3 per invariant B, `description`, `pluginVersion`) + `Dashboard.schemaVersion`(=3); host stores `fieldConfig`/`transformations` opaquely + **bounds** the record (≤32 transforms, ≤64 overrides/mappings/steps, rejected server-side). `view` adopts Grafana panel ids; `chart`→`timeseries` alias (a v2 chart cell renders unchanged). **Renderer:** `timeseries` with the full Grafana surface (legend/tooltip per-viz `options`; drawStyle/lineWidth/fillOpacity in `fieldConfig.custom`) replacing the bad single-`unit` string. **fieldConfig path:** unit/decimals/min-max/thresholds/mappings/color + byName/byType overrides, ALL formatted through ONE bridge (`fieldconfig/format.ts`) — documented fallback (canonical + static unit + decimals) behind a `format.*`-shaped call site until `lb-prefs` ships (swap is data-only, `viaPrefs` guardrail); thresholds COLOR not alert; Grafana color names→theme tokens. **One data hook** (`usePanelData`) = the single Phase-3 `viz.query` swap point (invariant A). **Editor:** the ONE `PanelEditor` (shadcn Sheet) for add AND edit via ONE pure `cell↔editorState` (de)serializer with the pinned identity `editorStateToCell(cellToEditorState(c))≡c` (v1/v2/v3) — fixes "edit loses my SQL options / add≠edit"; full Query/Transform/PanelOptions/Field/Overrides tabs from day one; reuses source picker + SQL Builder⇄Code + RefreshControl + WidgetView/WidgetHost; retired WidgetBuilder add-bar + deleted CellSettings ⚙. Tests (real gateway/store, no mocks): Rust `lb-host dashboard_test` +3 (v3 round-trip · v1/v2 compat · over-cap reject) + gateway `dashboard_routes_test` 6/6; UI `pnpm test` 138 (cellEditorState round-trip · format bridge · thresholds/mappings/resolve); `pnpm test:gateway` +6 `panelEditor` (ADD≡EDIT parity headline · backward-compat · live-preview real+denied · edit-cap host backstop · ws-isolation) + DashboardView updated to the editor flow; tsc/lint clean on new files. **Pre-existing (not this slice):** an untracked in-flight `role/gateway/src/routes/prefs.rs` (concurrent `lb-prefs` work) imports unlinked `lb_prefs` → blocks the gateway *lib unit-test* compile only (binary + integration tests + UI gateway suite all green); + 2 pre-existing lint errors in unmigrated `VariableEditor`/`StudioView` (no diff from HEAD). |
| Grafana-compatible visualization (`viz/`) — **Phase 2: the rest of the everyday chart set** (`stat`/`gauge`/`bargauge`/`table`/`barchart`/`piechart` on the Phase-1 spine) | frontend | S9+ | **shipped** | [viz](scope/frontend/dashboard/viz/README.md) | [dashboard-viz-phase2](sessions/frontend/dashboard-viz-phase2-session.md) | Additive over the shipped Phase-1 spine — **UI-only, no backend change** (host already stores `fieldConfig`/`options` opaquely + bounds the record), **no new datastore/cap, no client transform** (invariant B holds), all data through the one `usePanelData` hook (invariant A). **Six renderers**, one file per view under `views/<type>/`, recharts (no visx — Phase 3); the shipped v2 `stat`/`gauge`/`table` views **retired+replaced** (a v2 cell renders through the new renderer unchanged — canonical id is itself). **Typed per-viz `options`** per view (Grafana names + defaults verbatim from `/tmp/grafana/.../panelcfg.cue`): stat (graph/color/justify/text mode + reduceOptions), gauge (threshold markers/labels + reduceOptions), bargauge (basic\|lcd\|gradient + values/showUnfilled), table (showHeader/cellHeight/sortBy/pagination), barchart (orientation/stacking/showValue + legend/tooltip), piechart (pie\|donut/displayLabels + reduceOptions). **`reduceOptions` = ONE shared frame→value bridge** (`views/reduce.ts`: reduceFrame/reduceFrameValues/frameCategories + the calc set, shared with the timeseries legend) — the explicit, visible collapse for the single-stat family; empty/non-numeric→`null` honest "no value", never a fake 0; NOT the transform pipeline. **fieldConfig via the existing bridge** (`views/field.ts` resolves the value field's options + threshold/fixed/palette color once; every value formatted through `fieldconfig/format.ts`, thresholds COLOR not alert — no local toFixed/color in any renderer). **Result-shape↔type validation** (`views/shape.ts` conservative scalar/series/table/unknown detector + `usePanelShape` over the one hook; the VizPicker offers only shape-honest views, disabled-with-reason not hidden). **Editor extended not forked**: viewOptions +6 defaults, VizPicker shape-filtered + buildable, `PanelOptionsTab` → thin dispatcher to one per-view editor under `tabs/options/` (timeseries extracted there too); add≡edit identity unchanged (new option keys owned by editor groups). Tests (real gateway/store, no mocks): UI `pnpm test` **147** (cellEditorState round-trip +full stat/gauge/bargauge/table/barchart/piechart cell · `viz.phase2` reduce+shape units) + `pnpm test:gateway` **+6** `viz.phase2` (alias fidelity · options round-trip · result-shape↔type over real samples · fieldConfig-one-bridge no-stored-string · capability-deny across stat/gauge/table · ws-isolation); Rust `lb-host` + `dashboard_routes` unchanged+green; `cargo build --workspace`/`fmt` clean; tsc/lint clean on new files (the 1 SystemView gateway flake + the 3 pre-existing `VariableEditor`/`StudioView`/`WorkspaceSwitcher` lint errors are not this slice — no diff from HEAD). |
| Grafana-compatible visualization (`viz/`) — **Phase 3: backend-resolved transforms + datasource binding** (the `lb-viz` lib + the `viz.query` host verb + the one-file client swap + the real Transform/datasource editor) | frontend | S9+ | **shipped** | [viz](scope/frontend/dashboard/viz/README.md) | [dashboard-viz-phase3](sessions/frontend/dashboard-viz-phase3-session.md) | Panel data is now **resolved in the backend** — one impl for every client (web/RN/email/webhook), the `format.*` doctrine. **`lb-viz`** (new pure Rust crate `rust/crates/viz/`, the `lb-prefs` twin — no store/bus/I/O): the ONE implementation of Grafana's transformer set over a canonical columnar `Frame`, one transformer per file (`reduce`/`organize`/`filterFieldsByName`/`filterByValue`/`groupBy`/`joinByField`+`seriesToColumns`/`calculateField`/`sortBy`/`limit`/`merge`/`seriesToRows`) — ids+option shapes **verbatim** (Phase-4 import near-identity); empty/non-numeric → honest result, **never a fabricated 0**; a `Matcher` mirrored Rust+TS. **`viz.query(panel) -> {frames, rows}`** host verb (`rust/crates/host/src/viz/`, gated `mcp:viz.query:call`, member-level): for each non-hidden `sources[]` target it **re-enters the host MCP dispatcher** under the CALLER's principal+ws → each target tool's OWN cap + the workspace wall re-checked, **no render bypass**; a denied/failed target → **honest empty frame** (no fabrication, no host-privilege read); then assembles frames, runs the `transformations[]` pipeline via `lb-viz`, returns frames + the primary frame flattened to the SAME `rows` shape (renderers unchanged). Workspace from the **token**; cell still stores transforms/fieldConfig OPAQUELY (no record fork); per-panel frame budget. **Datasource binding:** a `DataSourceRef{type,uid}` selects the target tool (`surreal`→`store.query`, `series`→`series.*`, `federation`→`federation.query`), dispatched leashed by that tool's cap+the ws wall (ws-B can't resolve a ws-A datasource). **One-file client swap (invariant A):** `usePanelData` body → `viz.query` (`builder/useVizQuery.ts`, debounced; target args interpolated against the resolved VarScope pre-call so `${host}` repoints); a `series.watch`/`bus.watch` panel keeps the live SSE path until the named `viz.stream` follow-up; **no client transform lib (invariant B)** — `views/reduce.ts` stays the per-viz reducer. **Editor:** Query tab datasource dropdown (`datasource.list`); Transform tab is now a **real pipeline editor** (add/reorder/disable/configure) writing config `viz.query` runs. Wired into `tool_call.rs` (`viz.` host-native + dispatch threading `depth`); `mcp:viz.query:call` added to the gateway dev-session `member_caps` (the new member-level render path). Tests (real infra, no mocks): **lb-viz 49 units** (each transformer incl. empty/non-numeric honesty) + **`lb-host viz_query_test` 7** (store target+pipeline · no-transform parity vs direct `store.query` · multi-target `joinByField` · **viz.query deny** opaque · **denied-target honest-empty not a bypass** · **ws-isolation** · federation-bound target routes through `federation.query`) + **gateway `viz.phase3`** (usePanelData renders via viz.query == Phase 2 · Transform-tab authoring) + **dashboard_test 10**/**gateway lib 2** unchanged green; `cargo build --workspace`/`fmt` clean; UI `pnpm test` 147; tsc/lint clean on new files. **Deferred (named, not silent):** `viz.stream` (live frames over SSE), `federation.datasource.schema` (SQL-builder column dropdowns — a federation-plane add; federation uses raw-SQL meanwhile), the `format.ts`→`format.*` swap (sync→async cascade at 13 callsites → its own session, [followup](sessions/frontend/format-prefs-swap-followup.md)). One pre-existing full-run-only `SystemView` gateway flake (passes 9/9 isolated) is not this slice. **Phase 4 (Grafana JSON import/export) remains** the last viz follow-up. |
| rules workbench: Playground · chain canvas · datasources admin (gateway routes + UI clients + React surface over the shipped `rules.*`/`chains.*`/`datasource.*` verbs) | frontend | S9+ | **shipped** | [rules-workbench](scope/frontend/rules-workbench-scope.md) | [rules-workbench](sessions/frontend/rules-workbench-session.md) | All 3 phases shipped in one session (fanned out to 3 parallel sub-agents; lead reconciled the shared shell + route registration). **Playground** — CodeMirror editor + `rules.run` rendering `RuleOutput` 3 ways (scalar/grid/findings) + log + ms/ai budget; full CRUD rail; honest cage/deny/AI-budget/AI-not-configured states (`BadInput` verbatim, `Denied` opaque). **Chain canvas** — React Flow DAG (nodes=steps, edges=needs); cyclic edge → inline host error; Run + **bounded** `chains.runs.get` settle-poll colours nodes pending/running/ok/err/skipped, Halt subtree greyed. **Datasources admin** — first-party shell page (federation ext stays headless) over `datasource.*`: list (DSN-redacted), add (DSN write-only, implied grants shown), test (honest green/red), remove. Gateway re-checks every cap via `lb_host::call_tool` (ws+principal from token); `ToolError`→HTTP Denied/403·BadInput/400-verbatim·NotFound/404. Decisions honored exactly (CodeMirror not Monaco; React Flow v12; poll not `chains.watch` SSE; no new caps/tables/`localStorage`/`if cloud`). Tests (real in-process gateway, no mocks): Rust gateway 13+5+5, UI Vitest 6+4+5, all green; deny-per-verb + two-ws isolation + DSN-redaction + the cage/deny honesty cases. **Fixed a shipped host bug** found while building: `rules.list`/`chains.list` dropped every row by not unwrapping the `{data}` store envelope ([debug](debugging/host/rules-chains-list-drops-every-row-envelope.md)). |
| rules editor UX: a guided, explorable authoring surface (function palette · examples · data explorer · reusable insert-at-cursor + shared schema reader) | frontend | S9+ | **shipped** | [rules-editor-ux](scope/frontend/rules-editor-ux-scope.md) | [rules-editor-ux](sessions/frontend/rules-editor-ux-session.md) | Frontend-only extension of the shipped rules Playground — **no host changes, no new MCP verbs/caps**. A tabbed authoring panel beside the editor: **Functions** — a searchable, categorized **palette** of the engine's registered Rhai verbs (Data·Grid·Timeseries·AI·Output), **click-to-insert** at the cursor; the catalog is a **static typed mirror** of `rust/crates/rules/src/verbs/*` (registered set is compile-time known — no invented "list functions" verb), one data file per family. **Examples** — ready-to-run rules (bodies reuse the proven gateway-test bodies); one click loads into the buffer with a **dirty-confirm** guard. **Data** — registered datasources (`datasource.list`→`source()`), local store schema (`store.schema`→name), discoverable series (`series.list`→`history()`), each click-to-insert with **honest** loading/deny/empty states (a denied list is a deny, never a fake roster; DSN never rendered). **Reuse (hard requirement):** extracted the `store.schema` reader from the dashboard-named `lib/dashboard/sql.api.ts` to a shared **`lib/schema/`** consumed by **both** the dashboard SQL builder **and** the rules explorer (both suites green over it); a shared **`components/codeeditor`** (`CodeEditor` + `insertSnippet` ref handle = the one insert-at-cursor primitive) and **`components/schema/SchemaBrowser`** (click-to-pick tree). **Honest gap:** no per-external-datasource table introspection verb exists (`datasource.list` is kind+endpoint only) — a **named follow-up**, not a silent omission. Tests (real in-process gateway, no mocks, seeded via real `seedIotDemo` + real `datasource.add`): UI Vitest `AuthoringPanel.gateway.test.tsx` **6** (palette categories + click-to-insert · search filter · example loads+runs green · dirty-confirm guard · explorer lists real datasource+schema+series no-DSN · denied section honest deny) + the dashboard SQL suite **8** stays green over the extracted reader; `RulesView.gateway.test.tsx` 6 unchanged; `pnpm test` 114; `tsc`/`eslint` clean. |
| undo journal: reverse state / compensate motion (store `rev` · atomic before-image at `write_journaled` · conditional stale-refusing restore · runtime-taint classification · **auto-capture-on-dispatch** · cap-gated `undo`/`redo`/`history.*` MCP verbs · **exposure: grants + gateway routes + app-SDK seam**) | undo | S10 | **building** (backend exposed & green; **shell affordance not built**) | [undo](scope/undo/undo-scope.md) · [exposure](scope/undo/undo-exposure-scope.md) | [undo-build](sessions/undo/undo-build-session.md), [auto-capture](sessions/undo/undo-autocapture-session.md), [exposure](sessions/undo/undo-exposure-session.md) | Mechanism + verbs + **auto-capture-on-dispatch** shipped & green. **Auto-capture (new slice)** — every mutating tool call through the `call_tool` dispatch seam is journaled automatically, classified from **runtime outbox taint** (not manifest metadata): new `lb-store` task-local taint (`taint_scope`/`mark_outbox_reached`/`mark_store_written`, marked at `write`/`write_tx`/`lb_outbox::enqueue`) bubbles through nested host-callback calls so a reversible-declared tool whose nested call reaches the outbox is irreversible as a whole (the `max` rule, enforced by scoping); `lb-undo::record_captured` journals an already-applied reversible single-record change (before-image snapshot at the seam); `crates/host/src/undo_capture/` plans the call (reversible single-record `inbox.record` floor · non-generic→not-undoable · read→skip) and journals at depth 0 only (undo/redo/history verbs exempt; `undo_group` threads a group id for grouped-undo groundwork). Tests: `lb-store` taint 4 (composition/no-op-outside-scope) · host auto-capture 4 (reversible→undoable, outbox→irreversible, cap-deny, ws-isolation). **Store seam** — new store-managed monotonic `rev` (server-side bump in `write`/`write_tx`/new `write_journaled`; forward-compatible — `read` unchanged, legacy rows default `rev=1`), `read_versioned` (`Versioned{value,rev}`, absence=rev 0), `write_journaled` (change+journal in one tx). **New `lb-undo` crate** — immutable `JournalEntry` events + mutable `StackState` cursor; `record_change` (atomic before-image), `record_irreversible` (not-undoable marker), `restore` (one-tx guarded conditional write → `Stale` not clobber; `undo_live:{seq}` holds live predicate revs across undo↔redo cycles), `apply_undo`/`apply_redo`, `classify` (reached-outbox⇒irreversible, derived not trusted; compensation only adds, never downgrades), `peek` (for the host no-escalation check). **Host** — `crates/host/src/undo/` gates `mcp:<verb>:call` + no-escalation (original tool's cap) + `undo.any`; wired into `tool_call.rs` returning UI-shaped outcomes (`ok:false reason:stale|not_undoable|empty`). Tests (real store/node, no mocks): rev-probe 2 · undo 9 (stale-refused, create→absence, ws-wall, irreversible/compensable, redo-truncation, classification) · host-undo 5 (cap-deny, no-escalation, undo.any, ws-isolation, round-trip). `cargo test --workspace` 175 binaries green; `fmt`/`clippy` clean. Debug: [rev-subquery scalar footgun](debugging/store/rev-subquery-always-returns-first.md). The previously-noted `offline_sync` bus-timing flake is now **fixed** (publisher-side `await_subscriber` readiness barrier + deterministic loopback link; [debug](debugging/host-tools/offline-sync-replay-races-subscription.md)). **Exposure slice (2026-07-15)** — undo is now reachable by a member, end to end. **Hardening (2 real bugs):** a failed before-image read used to flatten `Err→None` and journal a *create* whose undo DELETES a live record — `undo_capture/decide.rs` is now the pure outcome table (a read error is `NotUndoable`, only a successful absent read is a create); and the depth cap trimmed the cursor without deleting the fallen-off events (unbounded journal) — new `undo/src/prune.rs` commits the trimmed cursor + `DELETE undo:{seq}`/`undo_live:{seq}` in ONE tx (`push_do` is `#[must_use]` and reports the pruned seqs; `depth_cap: Option<usize>` on the `Record*` structs makes it provable in 3 writes). **Grants:** `mcp:undo:call`/`mcp:redo:call`/`mcp:history.compensations:call` → member; `mcp:undo.any:call` → admin (the dotless verbs match no `mcp:*.<verb>:call` wildcard). **Behaviour change:** `history_compensations` now authorizes on its OWN verb, not `history.list` — a caller holding only `history.list` loses it. **Gateway:** `routes/undo.rs` (`POST /undo`·`/redo`, `GET /undo/history`·`/undo/history/{seq}/compensations`), ws+principal from the token, typed `ok:false` refusals as `200` data, `Denied`→opaque 403; body takes `surface` only (no cross-actor undo in v1 — admins use MCP). **Client:** 4 verbs added to the app-SDK `invoke.ts` verb→route map + `undo/undo.types.ts`. **§2.3 sync proof — the scope's long-owed load-bearing case — now discharged** (`undo_sync_test.rs`, 3 tests): an edge-captured undo is REFUSED at a hub whose copy moved on, with an unmoved-hub control proving the refusal is caused by the intervening write. **Caveat:** there is NO journal replication in the product (`ChannelSync` mirrors inbox `Item`s only), so the test carries the edge's real journal rows itself — transport stubbed, mechanism real; what is proven is the predicate's node-agnosticism. **Tests (all revert-checked — each confirmed to FAIL when its bug is reintroduced):** `lb-undo` 10 · host undo 5 · host undo_sync 3 · `decide` 6 · grants 1 · gateway routes 8 · app-SDK real-node 6. `cargo build --workspace` + `fmt` clean. **NOT built: the shell affordance** — no toolbar/Ctrl+Z/toast; `app/shell` is a thin login→extension mount (no toolbar, no dock, no global shortcuts, no `hasCap`), and the focus contract vs `ce-wiresheet`'s own Ctrl+Z (`CeEditor.tsx:1209-1260`) is still an open question (recommendation: a DOM `data-owns-undo` attribute, since a federated remote can't share a JS registry but does share the DOM). Follow-ups: grouped undo (group id now threaded), reversible capture beyond the single-record floor, manifest `compensation` WIT field, file/blob undo. |
| agent-run: streamable, externally-drivable, interactively-gated run (durable typed transcript · `RunEvent` stream · `agent.watch`/SSE · per-tool Allow/Deny/Ask first-settle · ACP adapter · model-activated skills) | agent-run | S10 | **shipped** | [agent-run](scope/agent-run/agent-run-scope.md) | [agent-run](sessions/agent-run/agent-run-session.md) · [part2](sessions/agent-run/part2-policy-decision-session.md) | All 6 parts (peer-review order 0→1→3→2→4→5). **Part 0** — `lb_jobs::TranscriptEvent` (`#[non_exhaustive]`, `#[serde(tag="kind")]`) replaces the opaque-`String` step; `schema_version`; `Suspended`/`Cancelled` statuses; `append_event`/`cancel`/`suspend`; `lb_store::create` (first-write→`Conflict`); `rehydrate` so resume CONTINUES the conversation, not re-asks ([debug](debugging/agent/resume-re-derived-from-goal-not-transcript.md)). **Part 1** — new low-level `lb-run-events` crate (`RunEvent`+`ToolCallArgsDelta` from day one), `project`/`project_one`; live==replay pinned. **Part 3** — `lb_host::watch_run` (`agent.watch`, snapshot-then-deltas, ws-walled subject `ws/{id}/run/{job}/events`) + gateway `GET /runs/{job}/stream?token=` (mirrors channel stream); the loop `emit`s after each durable append; start/resume-vs-watch split (`agent.invoke` kept as compat wrapper). **Part 2** — `agent_policy:{ws}` (glob+shallow-arg, Deny>Allow>Ask) via `agent.policy.set` (admin); dedicated first-settle `agent_decision:{job}:{tool_call}` via `lb_store::create` + `agent.decide` (NOT last-writer-wins `Resolution`); Deny + Allow→replay resume. **Part 4** — new `role/acp` (`lb-acp` bin): ACP v1 lifecycle over stdio, trusted-session auth (token bound to one ws), disconnect-mid-permission durable→`session/resume`, client `mcpServers`/`cwd` rejected cleanly. **Part 5** — granted-skills catalog injected once/run + loop-internal `skill.activate` (S4 grant is the wall), recorded in transcript (survives resume). Tests (real infra, only LLM provider stubbed): jobs 6 · store create 3 · rehydrate 3 · run-events 4 · decision 7 + policy 10 · watch 4 · acp 5 (real spawned binary + disconnect-mid-permission e2e) · skill 5. `cargo test --workspace` 208 passed / 1 failed (the 1 = a **pre-existing** `cross_node_routing` Zenoh flake, fails identically on clean HEAD); `fmt` + `build --workspace` clean. Follow-ups: decision-reactor, `UseDecisionAsResult`, token deltas, AI-SDK encoder, wire `lb-acp` into `node`, run-feed UI. |
| built-in `host.*` introspection verbs (networking · timezone · filesystem metadata) | host-tools | S10 | **shipped** | [host-tools](scope/host-tools/host-tools-scope.md) | [host-tools](sessions/host-tools/host-tools-session.md) | Backend/agent-facing v1 shipped: `host.net.info`, `host.net.reach`, `host.time.now`, `host.time.zones`, `host.fs.stat`, `host.fs.list`. One `host.` arm in `tool_call.rs` delegates to `host_tools::call_host_tool`, mirroring `agent.*`; each verb gates through `authorize_tool`. No store/bus state, no registry, no UI panel. `host.net.reach` is TCP-only bounded `connect_timeout` (2s default, 5s cap); `host.fs.*` is metadata-only with one-level sorted/capped list. Focused host-tools tests green after final patch; `cargo fmt --check` green. Full workspace passed serially before the last tiny host-tools hardening patch, and the post-hardening serial rerun passed `host_tools_test` before being stopped in unrelated suites at user request. Default parallel `cargo test --workspace` exposed a pre-existing cross-node routing timeout that passes isolated/serially; logged under [debugging](debugging/host-tools/cross-node-routing-parallel-timeout.md). |
| extension SDK + built-in Studio | extensions | S10 | **building** | [ext-sdk](scope/extensions/ext-sdk-scope.md) | [ext-sdk](sessions/extensions/ext-sdk-session.md) | Implemented `lb-devkit` signing/scaffold/build/inspect, `lb-pack` wrapper, host `devkit.*` verbs, server-side devkit publish over `POST /extensions`, and Studio UI. Green: `cargo test -p lb-devkit -p lb-pack`; `cargo test -p lb-host --test devkit_test --test devkit_e2e_test`; focused Studio real-gateway test; `pnpm test`; `pnpm exec tsc --noEmit`; `cargo fmt`. Blocked from full `cargo build --workspace`/`pnpm test:gateway` by unrelated `rust/crates/host/src/agent/run.rs` compile error (`crate::run_events` unresolved); per collision rules this slice did not edit agent files. |
| hermetic devkit container builds | extensions | S10 | **shipped** | [devkit-container-build](scope/extensions/devkit-container-build-scope.md) | [devkit-container-build](sessions/extensions/devkit-container-build-session.md) | Fixes a real Studio build failure: `devkit.build` shelled `cargo`/`pnpm` out as a bare child of the node process, inheriting VS Code's `GIT_ASKPASS` — a private-dep extension (`control-engine`'s `NubeIO/ce-client-rust`) died `exit status: 101` fetching it, even though the identical build succeeded from a shell ([debug](debugging/extensions/devkit-build-fails-exit-101-private-git-dep.md)). Added `ContainerToolchain` (`rust/crates/devkit/src/container_toolchain.rs`) behind the existing `Toolchain` trait, running `cargo`/`pnpm` inside the pinned `docker/build/` image instead — selected by **config, not a branch** (`LB_DEVKIT_BUILDER=container`; `ProcessToolchain` stays default). Mounts the `rust/` workspace root (not just the extension subtree — generated extensions have `path = "../../crates/..."` deps that escape it) as the **host uid/gid** (build output isn't root-owned). A build-scoped git token comes from `lb-secrets` (`devkit/build-git-token`, host-mediated `ext:devkit` principal) and reaches the container only as `LB_BUILD_GIT_TOKEN`, consumed by a baked-in git credential helper — never a tokenized URL, never in the streamed log. Also fixed stdout/stderr interleaving in both toolchains (a failing `cargo`'s stderr no longer buried after the full stdout dump). `docker/build/Dockerfile` extended with Node 20 + pnpm (same image serves the pre-existing cross-build path and devkit builds); dropped its `ENTRYPOINT` so `ContainerToolchain` can run `cargo`/`pnpm` directly (cross-build Makefile target now passes `lb-build <target>` explicitly). Tests (real store/caps/Docker CLI, no mocks): `rust/crates/host/tests/devkit_container_build_test.rs` — fallback selection (unit) · toolchain-parity (a real native extension with its `lb-supervisor` path-dep builds via both executors) · credential-never-logged (seeds a real secret, asserts the streamed log never contains it — the exit-101 regression test) · clean failure on a missing image. Capability-deny + workspace-isolation for `devkit.build` were already covered executor-agnostically by the pre-existing `devkit_test.rs`. Green: `cargo test -p lb-devkit -p lb-host --test devkit_test --test devkit_e2e_test --test devkit_container_build_test` (12/12), `cargo build --workspace`, `cargo fmt --all -- --check`. Follow-ups: Docker-only v1 (no Podman abstraction), one shared cache volume per node (not per-workspace), no published pinned image tag. |
| widget config + Grafana-style variable system (vars · refresh · live · JSON payloads) + generic `bus.publish`/`bus.watch` | frontend / bus | S10 | **shipped** | [widget-config-vars](scope/frontend/dashboard/widget-config-vars-scope.md) | [slice1](sessions/frontend/widget-config-vars-slice1-session.md) · [vars-lib](sessions/frontend/widget-config-vars-lib-session.md) · [slice2](sessions/frontend/widget-config-vars-slice2-session.md) · [slice3](sessions/frontend/widget-config-vars-slice3-session.md) · [bus](sessions/frontend/widget-config-vars-bus-session.md) · [slice4](sessions/frontend/widget-config-vars-slice4-session.md) · [slice5](sessions/frontend/widget-config-vars-slice5-session.md) | All 5 slices + the platform fix shipped on the shipped widget-builder v2. **Slice 1** — `Cell.title` (additive serde, no new verb) + a per-cell ⚙ settings drawer reusing the builder fields in edit mode (`seed`/`onSave`/`bare`), gated on `mcp:dashboard.save:call`. **Shared `vars` lib** (`ui/src/lib/vars/`, pure TS, federation-shared, `VARS_LIB_V`): `interpolate` (3 syntaxes + format hints + multi-value + unknown-left-literal), `interpolateArgs` (deep, type-preserving — generalizes the control `{{value}}`; `argsTemplate` delegates to it), `resolveBuiltins` (pure, token/range-derived), `extractVarNames`. **Slice 2** — `Dashboard.variables[]` (additive serde) + a variable bar (single/multi/include-all) + editor + **URL-synced selection** (`?var-<name>=`, `varsFromSearch`/`withVar`); definitions on the record, selection in the URL. **Slice 3** — `interpolateArgs` wired into every cell `useSource` call + each control; the widget **ctx gains `vars`/`timeRange`** (additive v2, `WIDGET_CTX_V`); identity resolved shell-side (un-spoofable). **Platform fix** — generic workspace-walled `bus.publish` (fire-and-forget) / `bus.watch` (stream) host verbs (`crates/host/src/bus/`, one verb/file), gated `mcp:bus.publish\|watch:call`, subject namespaced `ws/{id}/ext/{subject}` + reserved-prefix denylist (a cross-ws/`series/` subject refused), gateway `POST /bus/publish` + `GET /bus/stream?subject=&token=` (auth-first 401/403). **Slice 4** — `RefreshControl` (URL `?refresh=30s`, tab-hidden pause) bumps a `refreshKey` re-resolving vars + re-running reads; `bus.watch` SSE wired into the bridge `watch`. **Slice 5** — `JsonPayloadField` (CodeMirror JSON template + target picker `bus.publish`/`ingest.write`/ext write tools → `interpolateArgs` → leashed bridge call; "published" not a fake "delivered"). Tests (real infra, no mocks): Rust `cargo test --workspace` green (host `dashboard_test` 8, `bus_test` 6; gateway `bus_routes_test` 4 — 1 **pre-existing** Zenoh `offline_sync` flake, passes isolated, untouched); UI `pnpm test` 114, `pnpm test:gateway` 117 (1 pre-existing SystemView-sheet flake, passes isolated). Mandatory deny + workspace-isolation + identity-un-spoofable + URL round-trip all green. |
| theme switcher (dark/light + 3 accent palettes) | frontend | S9/S10 | **shipped** | [theme-switcher](scope/frontend/theme-switcher-scope.md) | [theme-switcher](sessions/frontend/theme-switcher-session.md) | local shell preferences over a reusable `ui/src/lib/theme` layer: validated `lb.theme` storage, pre-React HTML application, `ThemeProvider`/`useTheme`, and a shadcn-style sidebar `ThemeSwitcher`. Palettes: amber default + teal + blue, all token-bound through the existing CSS variable contract; accent contrast clears AA in light/dark. Also hardened `Button`/`Sheet` ref forwarding for Radix composition. Tests: `pnpm test` 56/56, `pnpm test:gateway` 110/110, `pnpm build` green, `pnpm lint` 0 errors (legacy warnings remain). |
| **theme customizer** (Theme + Layout tabs; supersedes the switcher) | frontend | post-S10 | **shipped** (step 1 of the theming feature-set; steps 2–4 = workspace-branding, ext theme-inheritance, css-isolation) | [theme-customizer](scope/frontend/theme-customizer-scope.md) | [theme-customizer](sessions/frontend/theme-customizer-session.md) | Ports the shadcn-store **Customizer** into the shell as a slide-out sheet with **Theme** (light/dark · preset library = 3 built-in accents + curated shadcn/tweakcn subset · radius · **paste-to-import** tweakcn CSS · per-token **brand colors**) and **Layout** (sidebar variant/collapsible/side, spread onto the shipped shadcn `<Sidebar>` by `NavRail`) tabs. **The load-bearing fact:** presets write the project's **BASE** tokens (`--bg/--panel/--fg/--muted/--accent/--border`), NOT shadcn tokens — a **token-bridge adapter** (`lib/theme/preset-adapter.ts`) maps incoming shadcn-vocab presets back onto base tokens (any oklch/hex/hsl → `"H S% L%"` via `color-to-hsl.ts`), written inline on `<html>` so `globals.css` derives the shadcn tokens and **every token-driven surface — charts, panels, nav, editor — re-themes live** (a literal shadcn-token port would half-theme the app). Built-in accents keep the `data-theme-accent` path; custom/imported/library presets write inline base tokens + clear the attr. `ThemePreference` **hard-replaced** `{mode,accent}`→`{mode,preset,radius,layout,custom?,imported?}` (no compat shim — young project). **Persistence rides the existing `prefs` verbs, no new verb/table/cap:** a new nullable **opaque `ui_theme` axis** on the closed `lb_prefs::Prefs`/`ResolvedPrefs` record (the scope's original "key/value `ui.theme`" was unbuildable — the prefs record is a closed struct), stored as one JSON blob, **folds WHOLE** through the shipped resolve chain **member → workspace-default → built-in** — so a theme roams, an admin sets a workspace default via admin-gated `prefs.set_default`, member override wins; `localStorage` = first-paint cache only. Host + gateway **needed zero change** (serde-default field flows through `prefs.set/get/resolve/set_default`). Hand-authored token-bound primitives (`components/ui/{label,separator,accordion,color-picker}` — no new Radix dep). Tests (real store/gateway, no mocks): **Rust `lb-prefs`** `ui_theme_test` (6: round-trip · patch-preserves-i18n · member-wins-whole · ws-default-fills-in · none→None · **ws-isolation**) + `resolve_test`+1; **UI `pnpm test` 466** (adapter round-trip = the "existing UI re-themes" guard · import fail-closed · color conv · dom base-token/accent/variant-flip/radius · storage no-legacy-compat · provider cache→persist · LayoutTab pickers · NavRail themed-layout→`data-variant`/`data-side`); **`pnpm test:gateway` `theme-prefs.gateway.test` 5/5** (member round-trip+roam · ws-default fold · **capability-deny** member-no-`prefs.set` & non-admin-no-`prefs.set_default` · **ws-isolation**). `cargo fmt`/`tsc`/`eslint` clean; one **unrelated** `control_engine_appliance_routing` bus-timing flake (passes in isolation, zero prefs/theme refs). **Scope updated:** persistence correction + Layout non-goal reversed (the shell DOES use shadcn `<Sidebar>` with full variant/collapsible/side — the deferral rationale was factually wrong). **Follow-up for step 2:** `mcp:prefs.set_default:call` is absent from dev-login `member_caps()` — grant it or seed an admin via `signInWithCaps` for workspace-branding admin writes. **UX follow-up (landed):** per user, the customizer moved OUT of the nav footer INTO **Settings → Theme** (Theme+Layout sub-tabs); the nav-footer `ThemeSwitcher`/`Customizer` sheet was **deleted**. Settings tabs are now **URL-routable** — `/t/<ws>/settings/{preferences,theme,agent}` (bare `/settings`→`preferences`; `surfaceForPath` prefix-matches `/settings/*`→`settings`, same precedent as `/system/mcp`). `SettingsView` is URL-controlled (`tab`+`onTabChange`); tests use a `SettingsHarness`. Final: `pnpm test` **472** + gateway 16/16 (settings+theme) green. |
| host-callback ABI (a wasm **guest** calls host MCP tools) | extensions | S10 | **shipped** | [host-callback](scope/extensions/host-callback-scope.md) | [host-callback](sessions/extensions/host-callback-session.md) | the §11.2 **forever-ABI** addition — a guest is no longer a one-way box. WIT minor bump **`@0.2.0`** adds ONE import `host.call-tool(name,input)` so a guest reaches the SAME MCP surface the page bridge does (`series.*`/`ingest.write`/`outbox.status`/`inbox.*`/other `<ext>.<tool>`), under its **delegated `caller ∩ install-grant`** authority (S5 `Principal::derive`), through the existing `lb_host::call_tool` chokepoint — **zero new trust surface**. Identity carried into `HostState` **per-call** (set→clear, never instance-sticky — the instance is node-global) via a narrow **`HostBridge`** trait (`lb-runtime` stays below `lb-host`, no dep inversion). Re-entrancy bounded: fixed **depth guard (8)** + `try_lock` borrow-discipline (self-re-entry → "extension busy", never a deadlock; cross-instance → "call depth exceeded"). World **major still 0** so `@0.1.0` guests keep loading — but a `0.x` MINOR is semver-breaking at wasmtime's *link* time, so the runtime **links both `host` versions + falls back to frozen 0.1.0 export bindings** ([debug](debugging/extensions/wit-minor-bump-breaks-0_1-guest-linking.md)). Reference guest gained **`proof.derive`** (reads `proof.demo`→writes `proof.derived`=value×2 via the callback) + a thin `[ui]` "Run derive" card (one hook/section, frozen contract untouched). Also **hardened `Principal::derive`** (nested delegation preserves the original caller's constraint — no cross-hop widening). Tests (real wasm + store + caps): **+7 host** (deny-per-DIRECTION both ways · ws-iso through the callback · happy round-trip asserted via separate `series.latest` · re-entrancy bound · **ABI-compat 0.1.0+0.2.0 coexist** · no-identity-leak) + **2 proof-panel unit** + **2 real-gateway** (live `proof.derive` round-trip over a real socket + deny) + e2e "Run derive" step. `cargo test --workspace` **387 passed, 0 failed** + fmt green; live node proof: `proof.derive`→`{"derived":42}` over `POST /mcp/call`. **Open Qs 1–5 all resolved.** (E2e nav-slot precondition blocked by an unrelated concurrent shell rework — noted, not a callback regression.) |
| workflow-sim: a wasm **guest** PRODUCES inbox→approval→outbox motion via the host callback | extensions | S10 | **shipped** | [proof-workflow-sim](scope/extensions/proof-workflow-sim-scope.md) | [proof-workflow-sim](sessions/extensions/proof-workflow-sim-session.md) | completes the [host-callback](scope/extensions/host-callback-scope.md) bridge: it exposed only the READ/RESOLVE half of the durable-workflow surface, so a guest could read motion but not **produce** it. Two **write verbs** added to the `lb_host::call_tool` chokepoint + `is_host_native`, each gated identically (workspace-first, then `mcp:<verb>:call`, against **`caller ∩ install-grant`**), reusing the REAL durable write paths (no guest store handle): **`inbox.record`** `{channel,id,body,ts}→{ok}` (`lb_inbox::record`; **author host-forced** to the principal's `sub` = `ext:proof-panel` for a guest callback, never spoofable) + **`outbox.enqueue`** `{id,target,action,payload,ts}→{ok}` (`lb_outbox::enqueue`'s transactional change+effect, staged **Pending** — delivery stays the relay's). Reference guest gained **`proof.simulate`** (built ONLY via `host.call-tool`): `inbox.record`→`inbox.list`→`inbox.resolve` Approved→`outbox.enqueue`→`outbox.status`, returning `{inbox_id,resolved,outbox_pending}`. Manifest `[[tools]]`+`[capabilities] request`+`[ui] scope` carry `proof.simulate` + the write verbs (the `ui_decl::narrow` drop-bug avoided); dev claims +`mcp:inbox.record\|outbox.enqueue:call` (member-level) +`mcp:proof-panel.proof.simulate:call`. UI: one hook `useSimulate` + `SimulateSection` + thin `Panel.tsx`; on simulate the page bumps a `refreshKey` so `InboxSection` (re-pointed to `proof-triage`) + `OutboxSection` re-read and the user SEES the produced item + effect (frozen contract untouched). Tests (real wasm + store + caps): **+7 host** (deny-per-DIRECTION ×2 for EACH new write verb · ws-isolation · happy round-trip asserted via SEPARATE `inbox.list`/`outbox.status` reads · direct-gate defense-in-depth) + **2 proof-panel unit** + **2 real-gateway** (live `proof.simulate` produces motion the page reads back + deny). `cargo test --workspace` **398 passed, 0 failed** + fmt green. **Live:** fresh in-mem node → `make publish-ext` → `proof.simulate`→`{"inbox_id":"proof-sim-item","resolved":true,"outbox_pending":1}`, confirmed via separate `inbox.list`/`outbox.status` over `POST /mcp/call`. **E2E PASSES** (built shell, real Chromium: click "Run workflow simulation" → produced item + risen pending count, no hook/console errors, fresh screenshot) — the host-callback session's nav-slot block is resolved. **Open Qs 1–3 resolved.** |
| system map (topology + status console) | observability / frontend | S10 | **shipped** | [system-map](scope/system-map/system-map-scope.md) | [system-map](sessions/system-map/system-map-session.md) | a first-class, read-only **workspace topology + status console** — the map you open first when the chain (gateway → MCP → store / bus / outbox / job / extension) misbehaves. Two views over **one** workspace-scoped read so they never disagree: a **status grid** (a `Card` per subsystem with live numbers + `ok`/`idle`/`degraded`/`down` health) and a **react-flow topology** (nodes coloured by live health, fixed architectural wiring as edges, never dangling). Host service `rust/crates/host/src/system/` mirrors the `dbview` admin read-lens **exactly** (one verb/file, single gate, opaque error, `tool.rs` MCP dispatcher): `system.overview` + `system.topology` authorize **once** then read **raw** subsystem state (`lb_store::tables` counts · `list_installs`+sidecars · `lb_outbox` lifecycle) — not the gated wrappers (the snapshot is *one* cap, not the union it summarizes). `idle` = up-but-empty (never a fault); `degraded` on a dead-lettered effect or an enabled-but-stopped extension. Gateway `GET /system/overview`+`/system/topology` (mirror `/store/*`; ws from token; re-check server-side; opaque `403`); caps `mcp:system.overview\|topology:call` **admin-only** beside `store.*`. UI is a **first-class shell page** (`ui/src/features/system/`, NOT a federated ext), obeys the UI standard (shadcn-first — generated the `card` primitive token-bound like `sidebar.tsx`; `AppPageHeader`; responsive 1→3 cols + react-flow degrades to pan/zoom); `NavRail`/`App`/`admin-caps` registration, `Network` icon. Tests (no mocks): **5 host** (real `Node`+seeds: fixed set present · counts match · stopped-ext+dead-letter `degraded` · empty-ws all `ok`/`idle` · no dangling edge · **cap-deny** · **2-ws isolation**) + **5 real-gateway** (`SystemView.gateway.test.tsx`: live grid · outbox `degraded` · Refresh re-fetch · graph toggle · **360px responsive smoke**) + nav cap-gating; `tsc`/`lint` (0 err) green. **Caveat:** a concurrent session's in-flight `lb-runtime` refactor currently breaks the whole-workspace `cargo build`; system-map was verified green in a clean `HEAD` worktree (`cargo test -p lb-host --test system_map_test` 5/5, `cargo build -p lb-role-gateway` clean) — that crate left untouched per scope. **Session 2 (real stats + clickable):** the bus card now reports **real Zenoh liveness** — `lb-bus::bus_stats` reads the live `session.info()` (peer/router counts + node `zid`), so `idle`=0-peers-on-mesh (honest) and `ok`=connected, not handle-presence; `lb-mcp` `Registry::summary()` feeds the mcp card live extension+tool counts; gateway shows `role`. Cards are now **clickable drill-ins** to the page that owns each subsystem (`store`/`ingest`→data·ingest, `inbox`/`outbox`, `extensions`/`registry`→Extensions) via `features/system/navigate.ts` + `SystemView` `onNavigate`/`allowedSurfaces` (keyboard-operable, gated to allowed surfaces; gateway re-checks); `gateway`/`bus`/`mcp` have no page → stay static. Tests grew **5→7** real-gateway (bus peers/routers/zid metrics · card-click navigates). **Session 3 (subsystem detail — no more dead ends):** a third read verb `system.subsystem` (`GET /system/subsystem/{id}`, cap `mcp:system.subsystem:call` admin-only, one verb/file mirroring dbview/system) returns the full `ServiceStatus` for one subsystem + an `extra` blob — for `bus`, the **live peer/router zid lists** (`lb-bus::BusStats` now carries `peer_zids`/`router_zids` from `peers_zid()`/`routers_zid()`), `{}` otherwise; unknown id → opaque `Denied`. **Every** card is now clickable: page cards still navigate (Session 2), no-page cards (`gateway`/`bus`/`mcp`) open an in-place shadcn `Sheet` detail drawer (`SubsystemDetailSheet` + `useSubsystemDetail`) showing health/group/role/all-metrics and the bus zid lists. Tests: **5→9 host** (right card + bus zid arrays · unknown-id opaque · **cap-deny** overview/topology don't grant subsystem · **2-ws isolation** B's detail ≠ A's) + **7→9 real-gateway** (no-page card opens sheet w/ live zid list · ≤360px sheet smoke); full `cargo build/test --workspace` green **in-tree** this session (the earlier `lb-runtime` caveat resolved); `tsc`/`lint` 0 err; `pnpm test:gateway` 82/82. Follow-ups: live `system.watch` feed · pub→sub echo probe · typed per-crate `status()` · control-actions-inline (deferred, read-only). **Session 4 (tool catalog + MCP & ACP service pages — tool-catalog scope):** two new read verbs beside the three. `system.tools` → `SystemTools { ws, role, tools: ToolInfo[] }` = the **full reachable MCP tool catalog with descriptions** — both halves: a **static host-native catalog** (`system/catalog.rs` `const`, `host.*`/`system.*`/`agent.*`/`bus.*`/`store.*`/`inbox.*`/`outbox.*`/`dashboard.*`/`template.*`/`devkit.*`/`series.*`/`ingest.*`, one-liner each, drift-guarded by a test asserting every `is_host_native` prefix has ≥1 row) **+** every extension's registry tools (`<ext>.<tool>`, name-only — `Registry::entries()` added; the load-bearing `Hosted.tools: Vec<String>` left **unchanged**, descriptions joined at read time not stored). `system.acp` → `AcpInfo` = the **ACP adapter's static facts** (protocol v1, handled `session/*` methods, capabilities, JSON-RPC error codes, auth notes; `system/acp.rs` mirrors `role/acp` so the UI never imports the role binary) — honest that ACP is a per-stdio-session adapter, **capabilities not a live feed**. New `acp` subsystem card (Idle) + `acp→mcp` topology edge. Gateway `GET /system/tools`+`/system/acp`; caps `mcp:system.tools\|acp:call` admin-only (added to `credentials.rs`). UI: two **new shell pages** drilled from the grid (not in sidebar) — `features/system-mcp/` (searchable, source-grouped tool table + live counts) + `features/system-acp/` (labelled fact sections); new `CoreSurface`s `system-mcp`/`system-acp`, routes `/system/mcp`+`/system/acp`, `navigate.ts` maps `mcp`/`acp`→pages. **Fake removed (CLAUDE §9):** hard-deleted `ui/src/lib/session/admin-caps.ts::ADMIN_CAPS` — a dead client-side re-implementation of the gateway's `member_caps()` cap list (real caps come from `POST /login`); `CAP`/`hasCap`/`isAdmin` (display-gating strings) kept. Tests: **9→13 host** + **3 unit** (catalog lists host-native+real-registry-ext tools sorted/well-formed · **cap-deny** each verb needs own cap · **2-ws isolation** host facts identical · ACP protocol+methods · drift-guard · ACP shape) + **9→16 system-area real-gateway** (`McpServiceView.gateway.test.tsx` 5: real host tools+descriptions render · search filters · live counts · **cap-deny**; `AcpServiceView.gateway.test.tsx` 2: real protocol/methods/codes · **cap-deny**; SystemView updated: mcp/acp drill, phone-sheet uses no-page `bus`); `cargo build/test --workspace` green, `cargo fmt`, `tsc`/`lint` 0 err, `pnpm test` 114/114. (Pre-existing unrelated `DashboardView.gateway.test.tsx` drag-sim flake untouched.) |
| self-contained extension over real Module Federation (`fleet-monitor`) | extensions | S10 | **shipped** | [ui-federation](scope/extensions/ui-federation-scope.md) + [dashboard-widgets](scope/frontend/dashboard-widgets-scope.md) | [fleet-monitor-federation](sessions/extensions/fleet-monitor-federation-session.md) | an extension is now **one folder = backend + frontend** (each optional). New `rust/extensions/fleet-monitor/`: a **native Tier-2 sidecar** (own PID, supervised over stdio, `fleet.summary` MCP tool) **+** its co-located `ui/` built as a **real Vite Module Federation remote** (`@originjs/vite-plugin-federation`, shared React singletons — not a hand-rolled `import()`), real **shadcn/ui + Tailwind**, mounting a cap-gated sidebar **page with 3 nested routes** + declaring **2 dashboard widgets**. Data only via `mount(el,ctx,bridge)` → `POST /mcp/call` (frozen `series.*` reads; never token/DB). Shell is the federation **host** (`ui/vite.config.ts` shares react/react-dom; `ext-host/federation.ts` loads remotes by gateway URL; `ExtHost` rewritten raw-import→federated). **Contract refactor:** `[widget]`→**`[[widget]]`** (`widgets: Vec` end to end). **Load-bearing fix:** the **native** install path now persists `[ui]`/`[[widget]]` (shared `host/src/ui_decl.rs`) — it silently didn't before. **Fake removed** from page discovery. `ui/extensions/hello-ui` + `rust/extensions/hello-ui` **hard-deleted**. **Pre-existing CI red fixed** (`cargo build --workspace`: `test_gateway_seed` stray-bin → `autobins=false`). Tests: 3 backend + 12 manifest + 2 ui_decl + 3 ext_ui + **2 native e2e** (real `OsLauncher` child + page/2-widgets in `ext.list`) + **6 ext-UI Vitest** + 20 shell + **50 real-gateway** (incl. `fleet-monitor` page slot + both widget tiles from a real `Install`); `cargo build/test --workspace` green (1 pre-existing sync flake, untouched). Widget *rendering* + iframe tier = follow-ups |
| Tier-1 WASM reference extension (`proof-panel`) — all-features demo | extensions | S10 | **shipped** | [proof-panel](scope/extensions/proof-panel-scope.md) | [proof-panel](sessions/extensions/proof-panel-session.md) | **Session 2:** the "whole platform, one page" demo — the page now proves the **full round-trip** from one cap-gated federated page through the bridge, not just reads. **(1) Ingest→read** (Write sample → `ingest.write` → `series.latest` reads it back live: write→stage→drain→read in the browser — the page CREATES its data), **(2) outbox.status** card + Refresh, **(3) inbox triage** (`inbox.list`→Approve/Reject→`inbox.resolve`, the first durable-workflow WRITE; actor host-forced), + the original series browse. **Load-bearing:** `call_tool` now dispatches the workflow verbs (`outbox.status`/`inbox.list`/`inbox.resolve`) too, and `ingest.write` **drains synchronously** (no bg worker; mirrors `POST /ingest`) so the read-back is immediate. Manifest's `[capabilities] request` + `[ui] scope` carry the four verbs (persisted scope verified). FILE-LAYOUT: one hook/verb (`useIngestWrite`/`useOutboxStatus`/`useInboxList`/`useInboxResolve`) + section components + thin `Panel.tsx`; frozen contract untouched. Tests (real infra, seeded via real write path): **9 host** (5 new: ingest round-trip, ingest/outbox/inbox deny-per-verb, inbox list→resolve, ws-isolation) + **8 proof-panel unit** + **9 real-gateway** (5 new live round-trips) + shell `test:gateway` **65** + **Playwright e2e** (click Write sample → value renders; Refresh outbox → counts; no hook/console errors, fresh screenshot); `cargo test --workspace` (0 failures) + fmt green. **Finding:** persistent SurrealKV throws `Invalid revision` on the 2nd ingest drain ([debug](debugging/store/surrealkv-invalid-revision-on-drain-reread.md), pre-existing engine bug, reproduced on the untouched `POST /ingest`) → live demo runs on the in-memory engine. fleet-monitor rework deferred (separate slice). **Session 1:** the **WASM (Tier-1)** counterpart to native `fleet-monitor` — the first artifact proving the basics composed end-to-end on the in-process path, **no placeholders**. New `rust/extensions/proof-panel/` (one folder, both halves): a **wasm guest** serving one MCP tool `proof.ping` (stateless, `{ok,ws,node,tier:"wasm"}`; caller-cap convention, no host-side cap) **+** a co-located **federated page** that lists series via `series.find` (tag-facet search) and reads a selected one via `series.latest`, **only** through the host bridge. `[ui]` (shield-check), **no `[[widget]]`** (deferred to dashboard). **Load-bearing fix:** `lb_host::call_tool` (the `POST /mcp/call` bridge entry) could NOT dispatch host-native `series.*` — it resolved only the runtime registry, so a federated page's reads `NotFound`-ed; it now authorizes then delegates to `call_ingest_tool` (**no new verb, no WIT change**). New `/_seed/series` test-gateway route (real ingest+drain+tag write) + `mcp:tags.add:call` in dev claims. **Finding:** `series.find` discovery needs tag edges the ingest path doesn't create from `labels` (worked around by seeding the edge; root fix tracked). Tests: **4 wasm unit** + **4 host** (callable·cap-deny·**grant-intersection-at-call-time**·**ws-isolation**, real `proof_panel_ext.wasm`) + **4 real-gateway** (empty→seed→find→latest→deny) + 6 in-memory page/mount; `cargo build/test --workspace` (**359 passed, 0 failed**) + fmt + `build.sh` (wasm + `remoteEntry.js`) green |
| extension UI pages (ui-federation slice 1) | frontend | S9+ | **shipped** | [ui-federation](scope/extensions/ui-federation-scope.md) + [dashboard-widgets](scope/frontend/dashboard-widgets-scope.md) | [extension-pages](sessions/frontend/extension-pages-session.md) | an extension now contributes a **full sidebar page** (and/or a dashboard **widget**) end to end. Frozen manifest `[ui]`/`[widget]` blocks → carried on `Install` (scope-narrowed to the grant) → surfaced via `ext.list` `ExtRow.ui`/`.widget` → shell `features/ext-host/` renders it (trusted **in-process dynamic-import `mount(el,ctx,bridge)`**; iframe tier = follow-up). Data via the **host-mediated bridge** (`POST /mcp/call` → `lb_host::call_tool`, cap+ws re-checked; page never holds the token/DB). Gateway serves bundles (`GET /extensions/{ext}/ui/{*path}`, traversal-guarded). Reference ext `hello-ui` (Vite React, served in both dev + gateway paths). Tests: 6 manifest + 3 host (persist·**scope-narrow**·**bridge-deny**) + 3 gateway (serve·traversal·deny) + 3 Vitest (slot·bridge-filter); 63 Vitest + workspace build + fmt green. **Widgets-on-dashboard (slice 2) deferred** (needs dashboard core) |
| Spine | core | S1 | **shipped** | [core](scope/core/core-scope.md) | [s0-s1-spine](sessions/core/s0-s1-spine-session.md) | host+store+bus+caps+mcp+runtime+1 WASM ext |
| Messaging | bus | S2 | **shipped** | [bus](scope/bus/bus-scope.md) | [messaging](sessions/bus/messaging-session.md) | pub/sub + presence + inbox + channel svc + React/Tauri UI + hot-reload |
| Sync / SSE | sync | S3 | **shipped** | [sync](scope/sync/sync-scope.md) | [multi-node-sync](sessions/sync/multi-node-sync-session.md) | 2nd node (role=config) + queryable routed MCP + edge↔hub sync + axum SSE/HTTP gateway + UI transport swap; 61+8+2 green |
| Shared assets | files | S4 | **shipped** | [files](scope/files/files-scope.md) + [skills](scope/skills/skills-scope.md) | [shared-assets](sessions/files/shared-assets-session.md) | `lb-assets` crate + host 3-gate (ws→cap→membership) doc/skill svc + grant-gated skills + persisted install records + `assets.*` MCP bridge + UI DocView; 83+11+2 green |
| AI core | agent | S5 | **shipped** | [agent](scope/agent/agent-scope.md) + [ai-gateway](scope/ai-gateway/ai-gateway-scope.md) + [jobs](scope/jobs/jobs-scope.md) | [ai-core](sessions/agent/ai-core-session.md) | central agent (owns the loop) + routed edge→hub invoke + grant delegation (`agent ∩ caller`) + `lb-jobs` resumable session + `lb-role-ai-gateway` (mock + idempotency cache) + `agent.*` MCP bridge + UI AgentView; 105+14+2 green |
| Coding workflow | coding-workflow | S6 | **shipped** | [coding-workflow](scope/coding-workflow/coding-workflow-scope.md) + [outbox](scope/inbox-outbox/outbox-scope.md) | [coding-workflow](sessions/coding-workflow/coding-workflow-session.md) | `lb-outbox` (transactional `Effect` + at-least-once relay) + `lb_store::write_tx` + `lb_inbox::Resolution` + host `workflow` service (ingest→triage→approval-GATE→job→outbox) + `workflow.*` MCP bridge + UI WorkflowView; 124+18+2 green |
| Signed registry | registry | S7 | **shipped** | [registry](scope/registry/registry-scope.md) | [registry](sessions/registry/registry-session.md) | `lb-registry` (digest binds manifest+wasm + `verify_artifact` + `VerifiedArtifact` newtype → verify-before-cache) + host `registry` service (pull·verify·cache·catalog·install behind a `Source` seam; rollback = install prior ver) + `registry.*` MCP bridge + UI RegistryView; 145+22+2 green |
| Native Tier-2 | extensions | S7 | **shipped** | [native-tier](scope/extensions/native-tier-scope.md) | [native-tier](sessions/extensions/native-tier-session.md) | `lb-supervisor` (spawn·frame·health·shutdown·restart behind a `Launcher` seam) + `echo-sidecar` reference binary + `[native]` manifest block + host `native` service (stateless: runtime `SidecarMap` + durable `Install`/`native_status`; `mcp:native.<verb>:call` gate; crash-restart-on-fault) + `install_native_from_registry` + `native.*` MCP bridge + UI NativeView; ~163+26+2 green (real-process restart proof) |
| Registry HTTP transport | registry | S7 | **shipped** | [registry](scope/registry/registry-scope.md) | [http-source](sessions/registry/http-source-session.md) | `lb-role-registry-host` = real HTTP **server** (`router`/`serve`, dumb origin serving signed `Artifact`s at `GET /artifacts/{ext}/{ver}`) + **`HttpSource`** client filling the host `Source` seam's last mock; verify-before-cache holds over the wire (tamper-in-transit rejected); `reqwest`/`axum` in the role crate, never in core. +5 Rust (round-trip · offline-from-cache · tamper · isolation · deny over a real socket); ~168+26+2 green |
| github-bridge as wasm | extensions | S7 | **shipped** | [github-bridge](scope/extensions/github-bridge-scope.md) | [github-bridge](sessions/extensions/github-bridge-session.md) | the S6 deferral resolved — the workflow's inbound edge ships as an installed **Tier-1 wasm** artifact (2nd real ext after `hello`). **Pure-transform** guest (`github-bridge.normalize`: webhook → `{issue_id,payload,ts}`, no host callback — WIT unchanged) + host **`ingest_via_bridge`** composing normalize→`ingest_issue` (2 gates). Orchestrator stays a host service. +7 Rust (install-deny · isolation · offline · rollback · transform branches, all through real wasm); finding: node-global stateless instance, wall is caps+store ([debug](debugging/extensions/loaded-extension-instance-is-node-global.md)); ~175+26+2 green |
| github-webhook ingress | extensions | S7 | **shipped** | [github-webhook](scope/extensions/github-webhook-scope.md) | [github-webhook](sessions/extensions/github-webhook-session.md) | the github-bridge follow-up resolved — the **live HTTP ingress**. `lb-role-github-webhook` (beside `lb-role-registry-host`): `POST /webhook` → **constant-time HMAC-SHA256** verify of `X-Hub-Signature-256` over the **raw body** (mediated secret, never logged) → `ingest_via_bridge`. Two-layer boundary: authenticity (`401` forgery) *before* authority (`403` ungranted). +12 Rust (bad-sig · deny · isolation · idempotent re-delivery · happy/real-socket · malformed→`422` · HMAC units, through real wasm). `axum`/`hmac` in the role crate, no core/WIT/cap-grammar change; ~187+26+2 green |
| outbox egress + hardening | coding-workflow | S7 | **shipped** | [outbox](scope/inbox-outbox/outbox-scope.md) | [outbox-egress](sessions/coding-workflow/outbox-egress-session.md) | the outbox's **live HTTP egress** + relay hardening (2 scope follow-ups). `lb-role-github-target` delivers `create_pr`/`comment` over GitHub REST (`reqwest` in the role crate; `422 already-exists` = idempotent success, no double-PR; token mediated). **Backoff + dead-letter** in `lb-outbox`+relay: `Effect` gains `max_attempts`/`next_attempt_ts`, new `DeadLettered` status, relay scans `due` (backoff-gated) + tallies dead-letters. +11 Rust (2 outbox backoff/dead-letter + 9 github-target: mapping units + happy·422·dead-letter·transport over real socket); 8 workflow regression updated; no core/WIT/cap change; ~198+26+2 green |
| close the loop | coding-workflow | S7 | **shipped** | [outbox](scope/inbox-outbox/outbox-scope.md) + [coding-workflow](scope/coding-workflow/coding-workflow-scope.md) | [close-the-loop](sessions/coding-workflow/close-the-loop-session.md) | ingress+egress **connected end to end into a live PR** (2 scope follow-ups). **Producer enrichment**: `PrSpec{repo,head,base,title,body}` record keyed by approval → `start_coding_job` emits the structured `create_pr` payload github-target maps (was `{scope_doc}`; also fixes a `format!` escaping bug). **Resolution reactor**: `react_to_approvals` = durable scan over `lb_inbox::approved` → auto-`start_coding_job` on `Approved` (relay's altitude; LIVE-query version a follow-up); idempotent on a deterministic job id (re-resolve/re-scan → ONE job/PR). +8 Rust (5 reactor: deny·isolation·idempotency·skip + 1 **full-loop over a real socket** (reactor→real GithubTarget opens PR) + 2 pr_spec units); no core/WIT/cap change; ~206+26+2 green |
| webhook front door | extensions | S7 | **shipped** | [github-webhook](scope/extensions/github-webhook-scope.md) | [github-webhook-multitenant](sessions/extensions/github-webhook-multitenant-session.md) | the webhook ingress went **multi-tenant**: `tenant_router` (`POST /webhook/{tenant}`) over a `TenantRegistry` (opaque slug → `{ws, principal, secret}`) fronts many workspaces from one process, each with its own secret. Routing by URL slug (chosen BEFORE the HMAC check → authenticity-before-parse holds); the **workspace wall holds at the front door** — A's secret on B's slug → `401`, never crosses; unknown tenant = opaque `401` (no enumeration oracle). Single-tenant `/webhook` untouched. +4 Rust (per-tenant routing · cross-tenant-secret isolation · unknown-tenant 401 · capability-deny); no core/WIT/cap change; ~210+26+2 green |
| workflow driver + node wiring | coding-workflow | S7 | **shipped** | [workflow-driver](scope/coding-workflow/workflow-driver-scope.md) | [workflow-driver](sessions/coding-workflow/workflow-driver-session.md) | the loop now **runs in a process**, not just tests. New `lb-role-github-workflow`: `drive_once`/`run_workflow_loop` tick the **reactor then the relay** per workspace (reactor-first → same-tick PR), over a list of `WorkflowBinding`s (isolation structural), with an **injected clock** (no wall-clock in the crate). Env-gated **node wiring** (`node/src/github.rs`): mounts the webhook front door + the driver loop by config (`LB_WORKFLOW_WS`/`LB_WEBHOOK_*`/`LB_GITHUB_*`), no `if cloud`; `node` is now `Arc<Node>`. The host owns the verbs, the role owns the cadence; GitHub `Target` behind the trait, no net dep in core. +4 Rust (one-tick close · idempotent re-tick · per-ws isolation · injected-clock); no core/WIT/cap change; ~214+26+2 green |
| dynamic workspace directory | coding-workflow | S7 | **shipped** | [workflow-driver](scope/coding-workflow/workflow-driver-scope.md) | [dynamic-directory](sessions/coding-workflow/dynamic-directory-session.md) | onboard/retire a workspace **without a restart**. New host **directory** (`register_workspace`/`deregister_workspace`/`enabled_workspaces`, `WorkspaceEntry{ws,channel,status,ts}`) in a **reserved namespace** `_lb_workflow_directory` (node-level config, secret-free, no MCP surface). Driver `run_directory_loop`/`drive_directory_once` re-reads the directory **each tick** (binding via an injected `principal_for`), so a runtime `register` is picked up next tick; `node` seeds the directory from `LB_WORKFLOW_WS` then drives it. +8 Rust (5 host: register/deregister/idempotent/durable/ns-isolation + 3 driver: register-mid-loop · deregister-drops · multi-ws isolation); no core/WIT/cap change; ~222+26+2 green |
| persistent store + spike | store | S8 | **shipped** | [persistent-backend](scope/store/persistent-backend-scope.md) | [persistent-backend](sessions/store/persistent-backend-session.md) | slice 0 / the gate. `Store::open(path)` on the pinned **SurrealKV** engine (both engines compiled in; constructor by `LB_STORE_PATH`, no code-branch) + raw `query_ws` seam. Permanent hermetic **capability-spike matrix** (5 LOAD-BEARING ✓ → GO; DEGRADABLE: bucket ✗→record-as-content, SEARCH ✓, HNSW ✓, materialized-view defines-but-doesn't-populate, LIVE ✓). **Crash set** (subprocess SIGABRT): write→reopen, kill-mid-tx→rollback, flush-burst→last-commit-survives. Isolation/parity re-run on disk. 6 spike + 4 crash + 4 parity green |
| generic ingest | ingest | S8 | **shipped** | [ingest](scope/ingest/ingest-scope.md) | [ingest](sessions/ingest/ingest-session.md) | slice 1. New `lb-ingest`: `Sample{series,producer,ts,seq,payload,labels,qos}`; durable staging **append** (cheap path) → commit worker **one-tx-per-batch** UPSERT on `[series,producer,seq]` + delete-staged same-tx (atomic + exactly-once on re-drain); `series.read/latest`; overflow drop-oldest/dead-letter. Host `ingest` svc (MCP gate, producer=authenticated principal, drain worker = ingest role) + `ingest.write`/`series.read`/`series.latest`/`series.find` bridge. **Anti-IoT held** (no device/sensor/MQTT in core). Tests: deny · ws-iso (store+MCP) · **kill-mid-commit re-drain** · two-producer collision · overflow both QoS; 5 crate + 3 durable + 6 host green |
| typed tag graph | tags | S8 | **shipped** | [tags](scope/tags/tags-scope.md) | [tags](sessions/tags/tags-session.md) | slice 2. `lb-tags` built from stub: `tag:[key,value]` typed nodes + `(entity,tag,source)` provenance edges; `add`/`remove`/`of`/`find` (exact/key-only/faceted intersection) + required per-workspace **tag-node cap** (deny). Spike-gated add-ons: **BM25 full-text** ✓, **HNSW vector** ✓ (dimension pinned, mismatch rejected), **per-dimension counts** (per-query — view doesn't populate). Host `tags` svc + `tags.*` bridge (no event verb); `series.find` wired on top. Tests: deny-per-verb · **identical-tag two-ws isolation** · idempotent re-tag · index-correctness; 5+1+4 crate + 3+1 host green |
| authz grants/roles/teams | auth-caps | S9+ | **shipped** | [authz-grants](scope/auth-caps/authz-grants-scope.md) | [authz-grants](sessions/auth-caps/authz-grants-session.md) | **slice 1** of the admin-CRUD/lifecycle/console build. New **`lb-authz`** crate (raw, ws-namespaced, no auth — mirrors `lb-assets`): `grant(subject→cap)` store (`assign`/`revoke`/`list`, revoke = idempotent **tombstone-upsert** §6.8), `role(name→caps[])` bundles (role-assign = a grant of `role:<name>`, no nesting), first-class `team(team,name)` records (member edges stay `lb_assets`). `resolve_caps(ws,user)` = `union(direct, roles, team-inherited)` deduped/sorted (the **Gate-2 cached half** of the freshness asymmetry, documented). Two **seams** for slice 2: `resolve_caps` (login projection) + `revoke_subject` (revocation-on-delete, returns count). Host **`authz`** service = the cap chokepoint: `grants.*`/`roles.*`/`teams.*` gated (`mcp:grants.assign`/`grants.list`/`roles.define`/`roles.list`/`teams.manage`/`teams.list`) + `holds_cap` **no-widening** guard (assign/define only caps you hold) + `call_authz_tool` MCP bridge. +5 host (deny-per-verb · 2-ws isolation store+MCP · grant resolution · no-widening · idempotent+revoke-seam) +2 crate units; cargo build/fmt/file-size green; no SDK/WIT/cap-grammar change |
| admin-crud destructive + user lifecycle | auth-caps | S9+ | **shipped** | [admin-crud](scope/auth-caps/admin-crud-scope.md) | [admin-crud](sessions/auth-caps/admin-crud-session.md) | **slice 2** — the destructive half + a real dev-store **user CRUD**. New host **`users`** svc: `UserRecord{user,active,role,cred_ref}` per `(ws,user)` + credential-free `UserView` (`cred_ref` never serialized); `user.create`/`list`/`disable`/`enable`/`delete` (gated `mcp:user.manage`/`user.disable`); **`user_login_check`** = the un-gated pre-mint seam wired into `POST /login` so **disable bites minting** (absent record still auto-seeds). Workspace lifecycle: `rename`(+un-archive)/`delete`(soft archive, hidden from list)/**`purge`** (hard: distinct `mcp:workspace.purge` cap **AND** typed confirm token; directory tombstone, no resurrection). `teams.delete` (cascade: drop member edges + `revoke_subject` + tombstone, returns count) + `teams.rename`; `members.remove`. `user.delete`/`teams.delete` call slice-1's **`revoke_subject`** seam (one revoke path). Gateway: `/admin/*` routes + `DELETE /teams/{team}/members/{user}`, each **re-checks the cap server-side** (UI gate is convenience); dev claim set now admin. `http.ts` gains every verb (+ `delJson`) + `admin.fake.ts` 1:1. +7 host (deny-per-verb · 2-ws iso · soft-before-hard+confirm · disable-bites-login · delete-revokes-grants · teams-cascade · tombstone-not-resurrected) +3 gateway (**server-deny-on-forged-call** · admin round-trip · login-refuses-disabled) ; 40 Vitest + tsc green; cargo build/fmt/file-size green; no SDK/WIT change |
| admin console UI | frontend | S9+ | **shipped** | [admin-console](scope/frontend/admin-console-scope.md) | [admin-console](sessions/frontend/admin-console-session.md) | **slice 4 of 4** — the UI that drives slices 1–3's destructive/admin verbs. One shared **`ConfirmDestructive`** (props: consequence · reversible · escalation `none\|type-name\|second-gate`) every delete/disable/remove/uninstall routes through (blocks until confirmed; type-the-name for ws purge; second-gate for uninstall; cancel = no-op). Cap-gated **`features/admin/`** section — `WorkspacesAdmin` (archive/purge), `UsersAdmin` (create/disable/delete), `TeamsAdmin` (create/rename/delete w/ live member count), `MembersAdmin` (add/remove + freshness-asymmetry copy), `GrantsAdmin` (read + assign/revoke, **no role editor**) under a tabbed `AdminView` (per-control cap gate). Top-level **`features/extensions/`** console over `ext_*` (both tiers · live state · restart count · start/stop/uninstall) — **retires `RegistryView`/`NativeView`**, coverage ported. Caps surfaced to the UI: `LoginReply` gained `caps` + `Session.caps` + `lib/session/admin-caps.ts` (`isAdmin`/`hasCap`, mirrors `dev_claims`); fake returns admin caps. Nav cap-gated in `App.tsx`/`NavRail`. **The gateway is the only boundary** — UI gate is convenience, server deny on a forged call proven in Rust (`admin_routes_test`). **Plus (follow-on):** the dev claims gained the `mcp:ext.*` caps so the Extensions section actually shows (was hidden — cap was missing); a dev-only fake seed so the demo build isn't empty; and **signed-artifact upload shipped end to end** — UI `UploadArtifact` + `publishArtifact`/`http.ts` `ext_publish` over the existing gateway `POST /extensions` → `lb_host::ext_publish` (verify-before-store, **per-workspace install**, `mcp:ext.publish:call`). **59 Vitest passed** (was 40) — confirm flow · per-sub-view on the fake · cap-gated visibility · ext both-tier/lifecycle · **upload (verified/tampered/malformed)**; tsc clean; gateway build + `admin_routes_test` (4) green; no SDK/WIT change |
| extension lifecycle over the gateway | extensions | S9+ | **shipped** | [lifecycle-management](scope/extensions/lifecycle-management-scope.md) | [lifecycle-management](sessions/extensions/lifecycle-management-session.md) | **slice 3** — closes the lifecycle matrix + **exposes it over the gateway** (the biggest gap: host had the mechanisms but only Tauri reached them → browser `unknown command`). `lb-assets` **`Install`** gained `tier{wasm,native}` + durable **`enabled`** intent + `kind` (serde-defaulted); new `list_installs` (union both tiers) + `delete_install` (tombstone, `read_install` reads-absent). New host **`ext`** surface (dispatch by `Install.tier`, no `if tier`): `ext.list` (uniform `ExtRow{ext,version,tier,enabled,running,health,restart_count}`, joins native `SidecarMap`), `ext.enable`/`disable` (durable intent — **disable also stops the native child**, distinct from stop), `ext.uninstall` (stop+tombstone, idempotent, ws-first), + the boot **`reconcile`** verb returning a plan that **honors disable** (a disabled ext is NOT auto-started). Native refactor: idempotent `stop_sidecar_internal` for cascades, `stop_native` keeps `NotRunning` ([bug caught+fixed](sessions/extensions/lifecycle-management-session.md)). **Registry publish**: `ArtifactStore::publish` **verify_artifact-before-store** (tamper/unsigned/foreign rejected, nothing stored; idempotent) + `POST /artifacts`. Gateway `/extensions` routes (re-check caps); `http.ts` `ext_*` + `ext.fake.ts` 1:1. +4 host (deny · 2-ws iso · list-unions-tiers · **reconcile-honors-disable**) +4 registry-host (publish/tamper/foreign/unsigned) +1 gateway (ext reachable + non-admin deny); 40 Vitest + tsc green; native/assets suites green; no SDK/WIT change |
| extension publish → install → load (dev flow) | extensions | S9+ | **shipped** | [lifecycle-management](scope/extensions/lifecycle-management-scope.md) | [dev-publish-flow](sessions/extensions/dev-publish-flow-session.md) | closed the build→upload→**run** chain: it stopped at the catalog (publish ≠ install; nothing called install/reconcile at runtime → an uploaded ext never loaded). Now **`ext_publish` installs + loads live** after verify-before-store (S4 `install_extension`: persist `Install` grant + `load_extension`; publisher = approver so `admin_approved = manifest.requested_caps`), so an upload is reachable with no restart. New host **`load_enabled`** boot verb (`ext/boot_load.rs`): re-loads enabled wasm installs from the **digest-keyed cache** (catalog→digest→`read_cached`→load, reconcile-gated) — the **survives-restart** guarantee; `node/src/main.rs` calls it per `LB_WORKSPACE`. **Trusted keys from env** (`session/trusted.rs`: `LB_TRUSTED_PUBKEYS=key_id=hex,…`) — was always-empty `TrustedKeys::new()` → every publish `422`d. New dev **packager `rust/tools/pack` (`lb-pack`)** — the missing bridge from `build.sh` to the signed `Artifact` JSON the gateway/UI want (same `digest`+ed25519 idiom; generates+persists a dev key; `lb-pack pubkey` for the env var). **`.lazybones/` layout** replaces `.data/`: `data/dev-store` · `keys/dev-publisher.key` · `extensions/*.artifact.json` (one `rm -rf` reset). Makefile `pack`/`publish-ext`/`trusted-pubkey`; `dev`/`cloud` auto-wire `LB_TRUSTED_PUBKEYS`. +3 gateway (publish→install→**callable** · untrusted→422+nothing · no-cap→403, real routes+real wasm) +5 host `ext_publish_test` (installs+loads+callable · deny-nothing-stored · tamper-rejected-with-grant · **survives-restart** via real on-disk node1→node2+`load_enabled` · disabled-not-revived) +2 trusted-env units; verified live (`POST /extensions`→204, `GET /extensions`→`hello@0.2.0 running`); no SDK/WIT/cap change |
| gateway assets+workflow wiring | frontend | S9+ | **shipped** | [files](scope/files/files-scope.md) + [coding-workflow](scope/coding-workflow/coding-workflow-scope.md) | [gateway-assets-workflow](sessions/frontend/gateway-assets-workflow-session.md) | Next-up item 4 (partial): routed the host **`assets.*`** + **`workflow.*`** verbs over the SSE/HTTP gateway (were Tauri-only → `unknown command` in the browser). New `routes/assets.rs` (`GET\|POST /docs`, `GET /docs/{id}`, `/share`,`/link`; `POST /skills`, `GET /skills/{id}`, `/grant`) + `routes/workflow.rs` (`POST /approvals/{id}/request\|resolve\|start`; `start` reads the `PrSpec` back by approval id, the S6 gate surfaces as `started:false`); outbox view stays `GET /outbox`. Each re-checks the gate server-side, ws+principal from the **token not the body** (§7). `http.ts` + `workflow.api.ts` (added `requestApproval`, `PrSpec`) mirror 1:1. **`agent_invoke` deferred** — needs the real model provider (no mock). +7 Rust (deny-per-verb + ws-iso doc/skill/approval + approval-gate); ~221+26+2 Rust + 58 Vitest green (1 red is a separate in-flight roles refactor); no SDK/WIT/cap change |
| data console (DB browser + ingest) | frontend | S9+ | **shipped** | [data-console](scope/frontend/data-console-scope.md) | [data-console](sessions/ingest/data-console-session.md) | two non-SQL pages on the shipped S8 data plane. **Data** = admin, READ-ONLY raw-store lens: new generic `lb_store::{tables,scan,graph}` reads (id-cursor, hard-capped) + host **`dbview`** svc (`store.tables`/`scan`/`graph`, **admin-only** — relaxes gate 3, so granted to ws-admin not `member_caps`) + `/store/*` gateway routes; UI table-picker+counts / paged row-grid (expand→JSON) / **react-flow** graph (`@xyflow/react`, code-split). **Ingest** = the S8 `ingest.*`/`series.*` verbs over the gateway (new small `series.list(prefix)`; `POST /ingest` writes-then-drains so a manual sample is instantly visible) + UI series list/search / latest+recent / manual write. **Built the real-gateway Vitest harness** (`test_gateway` bin + `vitest.gateway.config.ts`, `pnpm test:gateway`) — first step of retiring the fakes (#00), NO new `*.fake.ts`. +7 Rust route (deny-per-verb · ws-iso, real node seeded via real write path) ; 60 Vitest (incl. NavGating member-hides-Data) + **7 real-gateway** (4 ingest · 3 data) + tsc + code-split build green; no SDK/WIT change |
| admin console redesign | frontend | S9+ | **shipped** | [admin-console](scope/frontend/admin-console-scope.md) | [admin-console-redesign](sessions/frontend/admin-console-redesign-session.md) | the AI-built admin UI "looked like a chat window" and hid relationships — rebuilt **relationship-first**. Four tabs (**People · Teams · Roles · Workspaces**); the old Users/Members/Grants tabs folded in (members live inline under a selected Team; grant/role assignment lives in each subject's detail). New master-detail: a selected user shows **the teams they belong to** (assembled from the real membership endpoints via `useDirectory`, never typed) + roles + advanced caps. **Real role editor** (the headline gap): added gateway **`POST /admin/roles`** (`define_role`→`roles_define`, no-widening server-side) — the UI builds a role by **checking caps from a list** (the admin's own session caps = the no-widening set), no `role:<name>` typing; `roles.list` now keeps each role's caps (was discarded). Chat-composer create replaced by a header action everywhere. Shared `AdminPanel`/`AccessEditor`/`useSubjectGrants`/`useRoles`. Deleted `UsersAdmin`/`MembersAdmin`/`GrantsAdmin` + hooks. +1 Rust gateway test (define/list/no-widening/deny → **5 admin-route tests**); UI **57 Vitest** + tsc + build green; no SDK/WIT/cap-grammar change |
| tool-driven widget builder (dashboard v2: any view → any MCP tool) | frontend | S9+ | **shipped** | [widget-builder](scope/frontend/dashboard/widget-builder-scope.md) | [widget-builder](sessions/frontend/widget-builder-session.md) | the **generalization** of the dashboard widget: a cell binds a **view** to an **MCP tool call** — any tool in the install grant, **read OR write** — superseding the frozen v1 read-only/four-series-verb contract to **v2** (forwardable set = `cell.tools ∩ install-grant`, re-checked at the host per call; `v` field on every cell/manifest/message). **Backend:** `Cell` gained serde-defaulted `v`/`view`/`source{tool,args}`/`action{tool,args_template}` (v1 cells unchanged); new **`render_templates`** table + `template.save`/`get`/`list`/`delete` host verbs (workspace+author-scoped, gated `mcp:template.*:call`, size-capped — durable Plot/D3/JSX snippets, never localStorage) wired into the `call_tool` bridge dispatch (`is_host_native` now also matches `dashboard.`/`template.`) + dev claims. **Frontend:** **WidgetBridge v2** (`call` any granted tool read/write + `watch` over the shipped series SSE; **token never crosses**); **sandboxed-iframe runtime** (opaque-origin `sandbox="allow-scripts"`, CSP, postMessage bridge) for scripted views (`plot`/`d3`/`template` — may WRITE a granted tool) + untrusted ext widgets; **trust-tier routing** (allow-listed publisher key → in-process federation; else + all scripted → iframe; allow-list empty by default); **`ext:<id>/<widget>` renderer** (named `mountWidget` on the same remote); the **full view vocabulary** (chart/stat/gauge/table/plot/d3/template/switch/slider/button) over a generic `useSource` hook; the **builder** with a **source picker** over `series.list`/`ext.list` (friendly label → `{tool,args}`, no tool name shown) + live preview + Add-to-dashboard via the unchanged `dashboard.save`. Reference: **`proof-panel` ships a `[[widget]]`** tile (resolves its deferred open Q). Tests (real gateway/store/caps, no fakes): **6 host `render_templates`** (CRUD · deny-per-verb · ws-iso · author-ownership · size cap · upsert idempotency) + **9 UI unit** (picker mapping · typed `{{value}}` fill · trust default) + **11 UI real-gateway** (CRUD+deny+ws-iso · **capability-deny incl. WRITES server-side even if bridge filter bypassed** · **ws-iso across a write** · **token-never-crosses** · **write-control e2e** real side effect · scripted-template write deny · trust-tier iframe routing · ext-widget palette+uninstall-evicts) + updated `DashboardView.gateway.test.tsx`. `cargo test --workspace` **404 passed** (1 pre-existing `offline_sync` Zenoh flake, passes in isolation) + fmt; UI **36 unit + 96 gateway** + tsc + 0 eslint errors; `proof-panel` build.sh green. **All 5 open Qs resolved** (ext key `ext:<id>/<widget-id>` · `mountWidget` named export · 4 KB inline cap → row · typed `argsTemplate` · optional control self-read). No core/WIT/cap-grammar change (the v2 contract is a frontend+host-verb additive supersession). |
| widget-builder follow-ups (SQL source + in-app editors) | frontend | S9+ | **shipped** | [widget-builder](scope/frontend/dashboard/widget-builder-scope.md) (Follow-up slices) | [widget-builder-followups](sessions/frontend/widget-builder-followups-session.md) | the three additive follow-up slices over the shipped v2 builder — **no contract change** (a SQL source is just another `{tool,args}`; the editors are the authoring UI for the shipped `plot`/`d3`/`template` views). **Slice A — `store.query`/`store.schema` (read-only SurrealDB):** new host service `rust/crates/host/src/store_query/` (one verb/file, mirroring `dbview`/`render_templates`). `store.query(sql, vars?) -> {columns, rows}` gated `mcp:store.query:call` — **READ-ONLY load-bearing**: PARSE with `surrealdb::syn::parse` and allowlist by **statement KIND** (single `SELECT` + `INFO`/`SHOW`; `CREATE/UPDATE/UPSERT/DELETE/INSERT/RELATE/DEFINE/REMOVE/ALTER/REBUILD`, multi-stmt, txn-control, `USE` each refused before the store — never a substring check), **workspace-walled** (`query_ws`, ns from the token, never the SQL), **bounded** (`SELECT * FROM (<sql>) LIMIT 10k TIMEOUT 5s`). `store.schema() -> {tables:[{name,columns:[{name,type}]}]}` gated `mcp:store.schema:call` from `INFO FOR DB`/`INFO FOR TABLE` (+ row-sample fallback) — feeds Slice C. Wired into `call_tool` (`is_host_native` matches the two exact verbs, NOT all `store.`) + gateway `POST /store/query`/`GET /store/schema` + dev claims; `ui/.../sql.api.ts` + a "Direct SurrealDB" source-picker entry (`useSource` already unwraps `{rows}` → every view renders unchanged). **Slice B — CodeMirror editors** (`@uiw/react-codemirror` + `@codemirror/lang-{javascript,sql}`, ported from rubix-cube, REST→bridge): `builder/editors/{theme,CodeEditor,PlotCodeField,TemplateSourceField,SqlEditor}.tsx` (one/file); `TemplateSourceField` reads `template.list` over the bridge (not REST); Plot/D3 defaults match the SHIPPED iframe runtime signature `async (bridge, el, engine)`; `WidgetBuilder` swaps its raw `<textarea>` for them; the editor edits only the snippet STRING → runs only in the sandboxed iframe (trust unchanged). **Slice C — Grafana-style Builder⇄Code SQL editor** (`builder/sql/`, ported from `grafana-sql`, `@grafana/*` stripped → shadcn primitives): typed `SqlBuilderQuery` + `toSurrealQL()` (one file) emitting `SELECT…FROM…WHERE…GROUP BY…ORDER BY…LIMIT`; `SqlQueryEditor`/`SqlQueryHeader`(toggle + Format + confirm-on-switch-back)/`VisualEditor`(rows + live preview, dropdowns from `store.schema`)/`RawEditor`(wraps `SqlEditor`); the cell stores BOTH the raw string (what `store.query` runs) AND the `SqlBuilderQuery` (so reopening returns to the builder); Builder only generates SELECT, Code stays parse-allowlisted by `store.query`. Loki files NOT ported (future LogQL reference). Tests (real store/gateway, no mocks): **6 host `store_query_test`** (deny · write-per-kind rejected at PARSE, store unmutated · 2-ws iso + `USE` refused · row-cap · SELECT round-trip · schema deny+iso) + **8 UI unit** (`toSurrealQL` columns/agg/filter/quote/group/order/limit + Builder→Code→Builder round-trip) + **8 UI real-gateway** (`store.query` deny/round-trip/Code-write-rejected/iso · `store.schema` deny/iso · visual-editor→Run→table+chart render on real seeded rows). `cargo test --workspace` green + fmt; `pnpm test` **44** + `pnpm test:gateway` **104** + tsc + 0 eslint errors. No core/WIT/cap-grammar change; the frozen v2 widget/bridge contract untouched. |
| extension widgets in the builder palette | frontend | S9+ | **shipped** | [widget-palette](scope/frontend/dashboard/widget-palette-scope.md) | [widget-palette](sessions/frontend/widget-palette-session.md) | the **last mile** of the extension-widget story over the shipped v2 builder — a packaged `[[widget]]` tile is now **addable from the palette**, not only renderable from a hand-authored cell key. **No backend, no v2/`mountWidget`/`[[widget]]` contract change** — a pure frontend discovery-and-gating slice. `sourcePicker.ts` gains a **`"widget"` group** + **`extWidgetEntries(rows)`** (one entry per `row.widgets[]` tile, label `<ext> · <tile.label>`, carries the tile icon + the resolved `ext:<id>/<widget>` view key; `widgetIdOf` **exported from `ExtWidget.tsx`** so picker and renderer share ONE slug — the key built == the key parsed), folded into `buildSourceEntries` (tool-harvesting `extension`/`action` entries kept — a tile and its tools are both useful). `WidgetBuilder.tsx` gains an **"Extension widgets" `PickerGroup`**; selecting a tile **hides the view chooser** (`viewsFor` returns the single packaged view) and forces the candidate cell to `{ v:2, view:"ext:<id>/<widget>" }` (no source/action — the tile owns its data via `scope ∩ grant`); preview routes through the shipped `WidgetView → ExtWidget` over the **real bridge**, trust-tiered unchanged. **The edit gate:** a new `canEdit` prop renders the whole add surface **only** when the session holds `mcp:dashboard.save:call` — derived in `DashboardView.tsx` from `useAppRoutingContext().caps` (the shell's existing grant source the nav gates on; **no new backend read**); the host re-check on `dashboard.save` stays the authoritative backstop. Tests (real gateway, real installed `proof-panel`, no fakes): **+4 UI unit** (one `widget` entry per tile · viewKey = `widgetIdOf` slug · no source/action · folded-with-tool-entries · disabled-skipped) + **+6 UI real-gateway** (full round-trip: palette lists `Proof Ping` → select hides chooser → preview mounts real `ExtWidget`, `proof.demo` latest asserted live → **Add** persists `view:"ext:proof-panel/proof-ping"` → `getDashboard` re-reads · **cap-deny headline**: `canEdit=false` empty add surface **AND** `dashboard.save` denied server-side for a principal lacking the cap · **ws-isolation** ws-B picker lists only ws-B tiles · **trust-tier from the palette path** installed widget → **in-process**); `DashboardView.gateway.test.tsx` now wraps the view in a **real `RoutingContextProvider`** fed the real session caps (no mock). **Trust-tier follow-up (same session):** publishing `proof-panel` live exposed that the iframe tier could NOT render an installed widget — the remote externalizes React to the **shell import map** (in-process only), so it rendered blank with `Failed to resolve module specifier "react"`. Fixed by routing **every installed extension widget in-process** (the publish/install cap IS the trust gate); the sandboxed iframe tier is now **scripted author code only** (`trust.ts` `extWidgetTier()` → `"in-process"`, `ExtWidget` dropped its dead iframe branch + `remoteIframeCode`). [debug](debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md). **Live e2e** `ui/e2e/dashboard-widget.spec.ts` (built shell :4173 + real node :8080, real Chromium): login → Dashboards → create → pick "proof-panel · Proof Ping" from the **Extension widgets** group → tile mounts **in-process** w/ the host's single React → renders the **real `proof.demo`** value over the bridge → Add persists → re-renders in the grid; **no iframe, no error wrapper, no `Failed to resolve module specifier react`, no hook-call crash**; existing `proof-panel.spec.ts` page e2e still green (no regression). Verified live: `make publish-ext EXT=proof-panel` → HTTP 204 installed+loaded, `ext.list` shows the `Proof Ping` widget, `remoteEntry.js` HTTP 200, dashboard.save round-trip of an `ext:proof-panel/proof-ping` cell over the live node. `pnpm test` **48** + `pnpm test:gateway` **110** + 2 Playwright e2e + tsc + **0 eslint errors** green. **SSE follow-up (same session):** to exercise the **live feed** + prove the picker's one-entry-per-`[[widget]]` with a real N>1 ext, `proof-panel` now ships a **2nd tile "Proof Ping Live"** — backfills with `series.latest` then **subscribes** via `bridge.watch("series.watch")` → the shipped `openSeriesStream` → gateway SSE `GET /series/{s}/stream` → ws motion subject, ticking per live sample, no reload/poll (the whole SSE chain was already shipped — this is the first widget that USES it). `mountWidget` now **dispatches by `widgetId`** (`proof-ping`/`proof-ping-live`); 2nd `[[widget]]` block + `series.watch` in scope + `mcp:series.watch:call` in `[capabilities] request` (so `ui_decl::narrow` keeps it — verified live in `ext.list`; SSE authorizes on `series.read`). +3 proof-panel unit (`WidgetLiveTile.test.tsx`: backfill→live-tick×2→badge · unsubscribe-on-unmount · deny) over a `watchBridge` double + **1 live Playwright e2e** `dashboard-widget-live.spec.ts` (built shell + real node: add Proof Ping Live from palette → backfill → **write a new sample → tile ticks live, no reload**). proof-panel ui **15** + all **3** dashboard/page e2e green; republished live (HTTP 204, both widgets in `ext.list`). All scope open Qs were pre-decided; the `widget_type` vs `view` detail resolved in-build (`view` drives render, `widget_type` stays the v1 `"chart"` fallback). |
| dashboard surface (grid + widgets) — Phase 1 | frontend | S9+ | **shipped** | [dashboard](scope/frontend/dashboard-scope.md) + [dashboard-widgets](scope/frontend/dashboard-widgets-scope.md) | [dashboard](sessions/frontend/dashboard-session.md) | **Phase 1 SHIPPED** — the `vision/0003` IoT dashboard, first-party over real seeded series. Full vertical: **`seed_iot_demo`** (real ingest path → `cooler.temp`/`fryer.state` + tag-graph tags) · host **`dashboard`** svc (`get`/`list`/`save`-UPSERT/`delete`/`share`, 5 caps, one verb/file) with the **full S4 three-gate authz** — gate-3 `visibility.rs` reuses the shipped `share`/`member` edges (private→team→workspace, **non-member denied**) · **series motion** (`ingest/motion.rs`: `publish_sample`+`subscribe_series` on `ws/{id}/series/{series}` — the one piece the scope assumed but didn't exist) · `routes/dashboard.rs` mirror + **`GET /series/{s}/stream` SSE** (the channel-stream analog) + `POST /ingest` now publishes motion. UI `features/dashboard/`: `react-grid-layout` grid (layout↔`cells[]` in the `dashboard:{id}` record, **not** localStorage), built-in **chart/stat/gauge** widgets (`useSeries`: `series.read` backfill + live SSE fold), tag/series binding palette, cap-gated nav. **5 host + 6 gateway + 3 real-gateway Vitest** (CRUD · deny-per-verb · gate-3 member/non-member · 2-ws iso · seed integrity · **live sample over a real socket** · UI create→bind→render→persist · tag-bound `series.find`). `cargo test --workspace`/fmt + `pnpm test`(20)/`test:gateway`(56)/`build` green; no SDK/WIT/cap-grammar change. **Phase 2 (federated widgets, contracts frozen `v:1`) NOT started** — the trust boundary warrants its own reviewed slice; Phase 1 proves the binding contract first. **Phase 3** (real fleet) unbuilt |
| make collaboration real | frontend | S7 | **shipped** | [collaboration](scope/frontend/collaboration-scope.md) | [collaboration](sessions/frontend/collaboration-session.md) | the UI went from a 1-screen S2 demo on fakes to a **real collaboration app over a real session**. **Identity keystone:** demo principal DELETED — `POST /login` mints a signed `lb_auth` token (dev credential store); **every** gateway route `verify`s the bearer token → workspace+caps from the **token, not the request** (§7); SSE auth via `?token=` (EventSource can't set a header). New host services: `channel_registry` (`channel_create`/`channel_list` + create-on-post, reuses chan pub/sub gate), `members` (`list_members`/`add_team_member` over S4 edges, `mcp:members.*`), `inbox` (`list_inbox`/`resolve_inbox` over `lb_inbox`, `mcp:inbox.*`), `outbox` (read-only `outbox_status`, `mcp:outbox.status`), `workspaces` (`workspace_list`/`create` in reserved ns). New gateway routes mirror each 1:1. UI: `lib/session/`+`useSession`, workspace switcher, channel list, members view, **rendered presence** (`usePresence` idempotent roster), real inbox view (replaces the workflow fake on the real path — Approve/Reject = S6 gate), read-only outbox view; `App.tsx` hardcoded `WS`/`CHANNEL`/`AUTHOR` gone. **Two real sessions** make the ws-isolation test real (ws-B sees none of ws-A). +5 host collab (cap-deny + ws-iso each verb) +14 gateway (session: issue/verify/forged/expired/ws-from-token · deny · 2-session iso · registry · real inbox · outbox pending→delivered · live SSE) +6 Vitest views; cargo build/fmt/file-size + pnpm build/test green; no core/WIT change |
| **SQLite datasource, first-class + the Docker-free demo dataset** — surface the shipped `source/sqlite.rs` engine as a real datasource kind and emit the demo building dataset into one `.db` file (answers Data Studio 10x OQ2 the lite way) | datasources | post-S10 | **shipped** (2026-07-05) | [sqlite-datasource-demo](scope/datasources/sqlite-datasource-demo-scope.md) | [session](sessions/datasources/sqlite-datasource-demo-session.md) | **All 4 goals landed.** Seeder: `seed.py --sqlite <path>` + new `sinks_sqlite.py` (same `inventory`/`generators`/`tags` brains, stdlib sqlite3, drop+recreate idempotent; lite defaults `--months 1 --interval 15` ≈956k readings in ~15s — verified twice-run-identical + element-wise equal to the generator output). **`make seed-demo-sqlite`** (→ `docker/postgres/seed-demo-sqlite.sh`) generates `.lazybones/data/demo/buildings.db` and registers `demo-buildings` via the normal `datasource.add`; `FED_ENDPOINTS` default grew **`127.0.0.1:0`** — the convention endpoint for file sources (rejected: exempting sqlite from `enforce_endpoint`, a rule-10 kind-branch in a core mediation chokepoint). UI: `AddDatasourceForm` kind is a **select** over a `KINDS` data array (per-kind DSN placeholders; sqlite prefills the convention endpoint + shows the node-local-path note). `source/sqlite.rs` now **refuses a missing path** with "resolves on the node running the federation sidecar, not the client" — SQLite would otherwise silently create an empty db that probes green; the path (= DSN) is never echoed. Tests green: NEW no-Docker **`host/tests/federation_sqlite_test.rs`** (probe/schema/query vs a real seeded `.db` · missing-path error + no-empty-file-created · path-DSN redaction in list+result · **cap-deny** · **ws-isolation**) + `DatasourcesAdmin.gateway.test.tsx` 6/6 (kind select, sqlite add, redaction holds for a path DSN). Promoted to `public/datasources/datasources.md`. |

---

## Scopes authored (ready to build)

The `scope/<topic>/` docs exist for all areas (see `scope/README.md`). **Fully authored:** core,
auth-caps, mcp, crate-layout, extensions (+ **native-tier**, + **ui-federation** — mount an extension's
own pages in the shell, module-federation/iframe by trust, host-mediated MCP bridge; scoped 2026-06-27,
not yet built), jobs, bus, inbox-outbox
(+ outbox), tenancy, frontend, sync, testing, debugging, ai-gateway, agent, coding-workflow (+ the
**workflow-driver**, new this slice), registry,
**node-roles** + **platform-targets** (filled this slice — placement × role + the native target tag),
and the **S10 cross-cutting retrofit** trio — **observability**, **audit**, **undo** (scoped
2026-06-27; three projections of the host dispatch chokepoint, sharing the `write_tx` seam — not yet built).
**extensions/host-callback** (**SHIPPED 2026-06-27** — see the slice row above + the
[session](sessions/extensions/host-callback-session.md)): the **forever-ABI** addition. A WASM **guest**
gained one WIT import `host.call-tool(name,input)` (`@0.2.0`, world major unchanged so `0.1.0` guests
keep loading) so it calls host MCP tools (inbox/outbox/db/series/other extensions) under its delegated
`caller ∩ grant` authority through the existing `call_tool` chokepoint — the symmetric backend dual of
the page's `POST /mcp/call` bridge. Identity carried into `HostState` **per-call** via a narrow
`HostBridge` trait; re-entrancy bounded by a depth guard + try-lock discipline; the `0.x`-minor link
break solved by linking both `host` versions. Reference guest's `proof.derive` + a `[ui]` card prove it
live. All 5 open questions resolved.
**frontend/dashboard** (scoped 2026-06-27 — the grid-of-widgets dashboard over real series: Phase 1
first-party/seeded `dashboard.*` CRUD + series live SSE + chart/stat/gauge widgets, Phase 2
widgets-as-extensions via the federation bridge, Phase 3 the real edge fleet; `vision/0003` made
buildable — not yet built).
**Promoted to `public/`:** core, auth-caps, mcp, crate-layout, bus, **workspace** (session boundary +
directory + lifecycle), **channels** (registry + durable history + SSE/presence), inbox-outbox (+
outbox + the resolution facet), tenancy, **store** (persistent backend + spike matrix, this slice),
**ingest** (this slice), **tags** (this slice), frontend, sync, files, skills, agent, coding-workflow, registry,
**extensions** (the runtime + two tiers), **frontend/data-console** (the DB-browser + ingest-explorer
pages, this slice) (+ `public/SCOPE.md`).

---

## Next up

000. **Dashboard editor parity — Phase 3.5 (scoped 2026-07-03, NOT built, user-reported).** A
    hands-on pass found the panel editor unusable for a real person despite the green spine:
    `organize` + 6 other transforms edit via a raw-JSON textarea, overrides take free-typed dotted
    property ids, value mappings/color schemes have no editor (though the render path applies them),
    per-viz options cover ~20% of Grafana's surface, and the Query tab is single-target. Plan:
    [`editor-parity-scope.md`](scope/frontend/dashboard/viz/editor-parity-scope.md) (primitives →
    option registry → typed transform editors → overrides pickers → per-viz parity → multi-target).
    Session: [`dashboard-editor-parity-review`](sessions/frontend/dashboard-editor-parity-review-session.md).
00. **Retire the `*.fake.ts` mock backend — DONE (2026-06-27, CLAUDE §9, testing §0).** The UI's 14
    `lib/ipc/*.fake.ts` files + the `fake.ts` dispatcher (a hand-written parallel backend that let work
    *look* shipped on an unbuilt path) are **deleted**. `src/lib/ipc/` now holds only `invoke.ts` (the
    seam) + `http.ts` (the real transport); `invoke` **throws** if no real node is reachable (no fake
    fallback), and the browser defaults `gatewayUrl()` to the local dev node. **Every** UI test now runs
    against a **real spawned gateway node**: `role/gateway/src/bin/test_gateway.rs` (feature-gated
    `test-harness`) boots a real gateway-role node + the production router PLUS test-only `/_seed/*`
    routes (real `lb_inbox::record`/`lb_outbox::enqueue`/`lb_assets::record_install` writes — seeding,
    not faking, §3.1); `ui/vitest.gateway.config.ts` + `src/test/real-gateway.ts` spawn it and run all
    `*.gateway.test.ts[x]` against it, seeded through the real write path. **Vitest now: 6 default
    (pure component/hook/logic) + 18 real-gateway files / 20 + 50 = 70 tests green.** The migration
    surfaced **real gaps the fakes had hidden**: the dev-login claim set was missing the `store:doc/*`,
    `store:skill/*`, and `mcp:workflow.*` caps (so those routes 403'd over the real gateway — now fixed
    in `credentials.rs`), and `useWorkflow.start()` passed an empty channel (invalid bus key — fixed).
    The two production hooks that imported a fake demo-seed (`useExtensions`/`useExtensionPages`) no
    longer do. **Follow-up:** the `agent` surface is unit-tested (its data hook mocked), not
    real-gateway, because `agent_invoke` needs a real model provider the gateway deliberately doesn't
    mock (documented S5 deferral) — wire it when the real provider lands (#3).
0. **S10 — cross-cutting retrofit (scoped 2026-06-27, NOT built)**: three concerns missed since S1,
   each a projection of the host dispatch chokepoint (§6.5/§6.6) and reusing the `write_tx` seam.
   **(a) Observability** (`scope/observability/`) — `tracing` spans/logs/metrics on every node, a
   `trace_id` that propagates across the routed Zenoh hop + into jobs/outbox, secret-safe by
   construction, OTLP export (no in-core dashboard). **(b) Audit** (`scope/audit/`) — an immutable,
   hash-chained, workspace-walled ledger of every allow/deny, appended at the chokepoint (complete by
   construction) and same-`write_tx` durable; generalizes §6.14's model-call audit. **(c) Undo**
   (`scope/undo/`) — a before-image reversible-command journal; the hard line is *reverse state,
   compensate motion* (host derives irreversibility from reaching the outbox). Build order:
   observability → audit → undo. **Co-design note:** observability's `trace_id` propagation and the
   open **token-on-the-bus** item should share **one** routed-call attachment envelope, not two.
1. **S7 platform maturity** (`STAGES.md`): the **extension registry** AND the **native Tier-2
   supervisor** are **shipped** — the **S7 exit gate is fully MET** (~~install from a signed registry,
   run offline once cached, roll back~~; ~~a native sidecar is supervised and restarts cleanly~~).
   Remaining S7 work: ~~**packaging the S6 workflow/github-bridge as installed wasm artifacts**~~
   **(github-bridge SHIPPED** — a pure-transform Tier-1 wasm artifact; the orchestrator deliberately
   stays a host service since it drives host-internal seams a guest can't reach). ~~Open follow-up here: a
   **webhook-receiver role crate** that drives `ingest_via_bridge` on a real HTTP POST~~ **(SHIPPED —
   `lb-role-github-webhook`: HMAC-verify → `ingest_via_bridge`)**; remaining webhook opens — ~~a
   **multi-tenant front door**~~ **(SHIPPED — `tenant_router` + `TenantRegistry`, route-by-slug,
   per-tenant secret)**, a **dynamic** tenant directory (onboard without a restart) and an
   `lb-secrets`-backed secret (~~a **resolution reactor** that auto-starts the job on approval~~ **SHIPPED** — `react_to_approvals`).
   ~~And the `host.call_tool`
   WIT question if a guest ever needs to call a host tool (a forever-ABI change, its own scope).~~
   **(SHIPPED — the `host.call-tool` `@0.2.0` ABI; see the host-callback slice.)** **Native-tier follow-ups:** a boot reconciler (re-spawn `lifecycle=started`
   from records), OS-level hardening (cgroups/seccomp/userns), a background health-poll reactor (the
   slice restarts on-demand at the call boundary), ~~the child→host MCP callback transport~~ **(SHIPPED
   2026-07-02 — `lb-sidecar-client`; see the native-callback-transport slice above)**, and native
   platform-target enforcement.
1b. **Registry follow-ups** (in `scope/registry/`): ~~a real HTTP `Source`/`registry-host` server~~
   **(shipped — `lb-role-registry-host`)**; remaining — a **durable backing** for the registry-host
   catalog + a **publish** endpoint (an outbox `Target` write) + TLS/read-auth on the server; a durable
   publisher-key allow-list + admin trust-management flow; key rotation/revocation (needs the hub
   identity directory); cache eviction/GC; the public catalog read-only union (per-workspace entries
   ship now); `registry.update` semantics; gateway/Tauri wiring for `registry_*`.
2. **Outbox `Target` adapters + relay hardening** — ~~GitHub HTTP~~ **(SHIPPED — `lb-role-github-target`)**
   and ~~backoff + dead-letter~~ **(SHIPPED — `max_attempts`/`next_attempt_ts`/`DeadLettered` + `due`
   scan)**, the **producer payload enrichment** a live PR needs **(SHIPPED — `PrSpec` + structured
   `create_pr`)**, and a **resolution reactor** that auto-starts the job on approval **(SHIPPED —
   `react_to_approvals`, a durable scan)** are all done. Remaining: **email / sync-publish** adapters
   behind the `Target` trait + **search-before-create** dedup; the **multi-relay atomic claim**,
   FIFO-per-target ordering, and the **LIVE-query** driver (the **poll-tick driver SHIPPED** —
   `lb-role-github-workflow`'s `run_workflow_loop` ticks reactor+relay per ws, mounted in the `node`
   binary by config; the LIVE push is the latency optimization on top). The **dynamic workspace
   directory** (hot-add without a restart) **SHIPPED** (`register_workspace`/`deregister_workspace`,
   reserved-namespace record, re-read each tick); remaining: the **webhook tenant directory** (paired
   with `lb-secrets` for per-tenant secrets), an admin/MCP surface for the register verbs, and GC.
3. **Real model provider + streaming** behind the S5 gateway contract — the mock is the only stub;
   add an OpenAI-compatible / local adapter and stream tokens as Zenoh motion. Agent/job progress can
   now also ride the durable outbox for the must-deliver transcript.
4. **Gateway/Tauri wiring for `agent_invoke` + `assets_*` + `workflow_*`** (S4/S5/S6 follow-up):
   ~~route the host verbs through the SSE/HTTP gateway to a real node~~ **`assets_*` + `workflow_*`
   SHIPPED over the gateway** (`routes/assets.rs` + `routes/workflow.rs` + `http.ts`/`workflow.api.ts`;
   gate re-checked server-side, ws+principal from the token — see [gateway-assets-workflow
   session](sessions/frontend/gateway-assets-workflow-session.md)). **Remaining: `agent_invoke`** —
   deferred because it needs the real model provider (#3 below; **no mock** in the gateway, by
   decision) — and the **Tauri desktop** command layer for all three (the browser/gateway path is the
   one shipped; the desktop shell still fixes its workspace).
5b. **Management CRUD — close the create-only gap** (scoped 2026-06-27; **slices 1–4 of 4 SHIPPED**). Built
   as four independently-shippable vertical slices: **(1) authz-grants model — SHIPPED** (`lb-authz` +
   host `authz` service: grant/role/team records, `resolve_caps` login projection, `revoke_subject`
   revoke seam, no-widening guard, `grants.*`/`roles.*`/`teams.*` over MCP). **(2) admin-crud backend —
   SHIPPED** (host `users` svc + workspace rename/archive/purge + teams delete/rename cascade +
   members.remove; the **login active-check** wired into `POST /login`; `/admin/*` gateway routes
   re-checking caps; `http.ts` + `admin.fake.ts`). **(3) extensions lifecycle — SHIPPED** (host `ext`
   surface: `ext.list`/`enable`/`disable`/`uninstall` dispatching by tier + boot `reconcile` honoring
   disable + `Install` gaining `tier`/`enabled`; registry **publish** verify-before-store + `POST
   /artifacts`; `/extensions` gateway routes + `http.ts`/`ext.fake.ts` — see [lifecycle-management
   session](sessions/extensions/lifecycle-management-session.md)). **(4) admin console UI — SHIPPED**
   (`features/admin` tabbed section + top-level `features/extensions` console + one shared
   `ConfirmDestructive` every destructive path routes through; `RegistryView`/`NativeView` retired into
   the console, coverage ported; caps surfaced via `LoginReply.caps` → cap-gated nav/tabs, the gateway
   the only boundary; 56 Vitest green — see [admin-console
   session](sessions/frontend/admin-console-session.md)). **All four slices done.** No SDK/WIT change.
   **Remaining follow-ups:** install-from-catalog/upload in the extensions console; a role editor;
   live multi-admin refresh; the Tauri desktop session.
5. **Fit-and-finish carryover:** ~~render presence in the UI~~ **(SHIPPED — `usePresence` roster)**;
   ~~a real login→token→principal session (replacing the gateway's demo principal)~~ **(SHIPPED — the
   "make collaboration real" slice: `POST /login` mint + per-route `verify`, demo principal deleted)**;
   **token-on-the-bus** so the hub can verify a routed caller's grant (S5/S6 are in-process co-trust —
   still open); sync the asset/job/outbox tables (all `(table,id)` upserts the channel sync path
   already covers); explicit edge→hub router endpoints (S7); the **Tauri desktop** command layer's
   session (the collaboration slice wired the browser/gateway path; the desktop shell still fixes its
   workspace).

---

## How to keep this current

Every session that changes state updates the relevant cell here as its **last step**
(`HOW-TO-CODE.md` §3 step 9). Keep it to one screen — if a section grows past a few rows,
the detail belongs in the per-feature docs, not here.
