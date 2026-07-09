---
name: e2e-datasources
description: >
  Use when asked to end-to-end test datasources / federation, or to test any CHART that
  reads external time-series. Prove a datasource works as designed against a REAL node +
  a REAL SQLite building dataset — MUST seed it first (charts are blank without the seed).
  The Docker-free path: `make seed-demo-sqlite` generates a real `.db` and registers it as
  a `kind:sqlite` datasource on the running node — no container. Then run the CRUD /
  permissions / access / functional checks.
---

# E2e datasources runbook — prove a datasource (and its charts) work as designed

Status: scope (the standard). Design intent:
[`../../scope/datasources/datasources-scope.md`](../../scope/datasources/datasources-scope.md)
and the Docker-free demo:
[`../../scope/datasources/sqlite-datasource-demo-scope.md`](../../scope/datasources/sqlite-datasource-demo-scope.md).
The checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** The automated code tests
(`cargo test`, vitest) are the **scope/session's** job and are assumed already green — this
runbook does **not** re-run them. Its job is to **drive the actual running system and
observe it behave**: a live node, a live datasource, a chart that either shows the data or doesn't.

**It proves a datasource does what it was designed to do — it is not for reporting bugs.**
A datasource is the one seam that reaches an **external** engine through the federation
sidecar. It has a hard prerequisite — **the source must be registered and its data
present** — and everything downstream (the `datasource.*` verbs, `federation.query`, and
every **chart** that reads them) is blank or wrong until it is.

> The federation fake-of-a-true-external rule (testing-scope §0): a provider you can't run
> locally (a remote Postgres) is faked behind one trait. But the demo dataset **can** run
> locally with zero external dependency — it's a real SQLite file the shipped `source/sqlite.rs`
> engine reads for real. So **seed the real file and register it as a real datasource** — don't
> fake it. Real rows; charts read real data. (This is the Docker-free path; a real Postgres/
> TimescaleDB on `127.0.0.1:5433` still works if you have it — see the appendix.)

---

## ⚠️ Step 0 — seed AND register the demo dataset (do this first, always)

Charts read time-series from the datasource via the federation sidecar. **No datasource →
no tables → blank charts → a false "the chart is broken".** This is the #1 datasources
false-bug. Seed and register before anything else.

The Docker-free path — one command against a **running** node — generates the real SQLite
building dataset and registers it as a first-class `kind:sqlite` datasource through the
normal admin verb (`datasource.add`), no container:

```bash
make dev                             # boot the node first (federation sidecar on by default)
make seed-demo-sqlite                # generates .lazybones/data/demo/buildings.db AND registers it
```

`make seed-demo-sqlite` wraps `docker/postgres/seed-demo-sqlite.sh`, which:
- runs `seed.py --sqlite <path>` (the **lite profile**: 1 month @ 15-min — 332 points ×
  ~2880 slots ≈ **950k readings**, seconds to generate, a few MB) into
  `.lazybones/data/demo/buildings.db` under the node's
  own data dir (so the sidecar — which resolves the DSN as a **node-local file path** — can see it);
- logs in as `user:ada` / `acme` and calls `datasource.add {name:"demo-buildings",
  kind:"sqlite", endpoint:"127.0.0.1:0", dsn:<path>}`.

It builds the **same** model the charts expect (identical `inventory`/`generators`/`tags`
as the Postgres seeder — one dataset definition, two sinks):

```
site (8)  →  meter (69)  →  point (332: kWh / kW / L·min / m³ / temp / status)  →  point_reading (~200k rows)
```

**Verify the file landed and the source probes green** before trusting any chart result:

```bash
# rows exist in the generated SQLite file
sqlite3 .lazybones/data/demo/buildings.db "SELECT COUNT(*) FROM point_reading;"   # expect ~950 000, not 0

# the source is registered and reachable (probe/list via the running node)
BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"ada","workspace":"acme"}' | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
curl -s $BASE/datasources -H "authorization: Bearer $TOKEN"     # 'demo-buildings' present, probe green
```

- The DSN for a `kind:sqlite` source is the **database file path**, resolved on the node
  running the federation sidecar (not the browser). The `127.0.0.1:0` convention endpoint (a
  file has no network endpoint) is pre-approved by `make dev`'s default `FED_ENDPOINTS`
  (`127.0.0.1:5433,127.0.0.1:0`).
- `seed.py --sqlite` truncates + reseeds every table on each run — safe to re-run.
- No local `sqlite3` binary? Query the file through the running node instead (drive
  `federation.query` against the `demo-buildings` source), or `python3 -c "import sqlite3, …"`.

---

## Step 1 — read the design (what is "correct"?)

- **[`../../scope/datasources/datasources-scope.md`](../../scope/datasources/datasources-scope.md)** — the ask: what a datasource is, the
  `datasource.add/remove/list` verbs, **DSN redaction**, endpoint pre-approval, ownership.
- **[`../../scope/datasources/sqlite-datasource-demo-scope.md`](../../scope/datasources/sqlite-datasource-demo-scope.md)** — the Docker-free demo
  dataset: `kind:sqlite` as a first-class UI kind, the DSN-is-a-file-path caveat, the lite
  profile sizing, `make seed-demo-sqlite`.
- Paging / decimation behaviour the charts depend on:
  [`federation-paging-scope.md`](../../scope/datasources/federation-paging-scope.md),
  [`series-decimation-scope.md`](../../scope/datasources/series-decimation-scope.md),
  [`series-paging-scope.md`](../../scope/datasources/series-paging-scope.md).
- **`../../skills/<name>/SKILL.md`** — the drivable `datasource.*` / query surface, if one
  exists yet (note the gap if not).

---

## Step 2 — point the node at the seeded source

`make dev` installs + supervises the federation sidecar and pre-approves both the Timescale
endpoint (`net:tls:127.0.0.1:5433`) and the sqlite convention endpoint (`127.0.0.1:0`), so
the Datasources page + charts work as soon as `make seed-demo-sqlite` has registered
`demo-buildings`:

```bash
make dev                             # federation on by default (FED_ENDPOINTS=127.0.0.1:5433,127.0.0.1:0)
# make dev FED_ENDPOINTS=            # would DISABLE federation — don't, for a datasources test
```

Now you have a **live system** — a real node, wired to the real seeded SQLite dataset,
serving the Datasources page and the charts. The checks below drive *that*, by hand or by
driving the real surface. You do **not** re-run `cargo test` here — the code-level suites
(`federation_test.rs`, `datasource_crud_ownership_test.rs`, `viz_query_test.rs`, the
`/datasources` route test) are the scope/session's job and are assumed green already. This
runbook is the **real-world** check on top: does the running thing actually behave?

---

## Step 3 — the checklist (prove it works as designed)

The four dimensions from [`../README.md`](../README.md#what-to-check--the-functional-dimensions),
against the real node + real seeded SQLite dataset.

### 3.1 CRUD — the datasource lifecycle round-trips
On the live Datasources page (or by driving the real `datasource.*` verbs):
- **add** a source (`make seed-demo-sqlite` already added `demo-buildings`; adding via the UI
  uses the kind **select** — pick `sqlite`, DSN = the file path), see it **appear** in the list.
- prove **remove** on a **throwaway** second source you add just for the check — then
  **leave the working `demo-buildings` source in place** so the user (and the chart) can use it.
- **Redaction (design rule):** the DSN/secret **never** shows raw in the list — only a
  secret ref. (For sqlite the DSN is a path, not a password, but the same redaction seam
  applies.) Confirm no raw secret leaks in the UI/response.

  *(The design rule is pinned by `datasource_crud_ownership_test.rs` — that's the
  scope/session's automated guard; here you're confirming the running system honours it.)*

### 3.2 Permissions — the capability wall holds
Every verb needs its grant; without it, refused:
- `mcp:datasource.add:call`, `mcp:datasource.remove:call`, `mcp:datasource.list:call`.
- Querying a source needs `mcp:federation.query:call`.
- Connecting out needs the endpoint cap — for sqlite the pre-approved `127.0.0.1:0`
  convention; **without** the pre-approved endpoint, the register/probe is denied (not
  silently attempted). Assert the deny.

### 3.3 Access — the workspace wall holds
A datasource is workspace-scoped. `demo-buildings` was registered in workspace `acme`;
assert another workspace (e.g. `globex`) can neither `list` nor query it — the same file
path registered in two workspaces would be two independent grants. Ownership is checked
before caps (see the ownership test).

### 3.4 Functional — the charts render the seeded data
This is the payoff, and the whole reason the seed matters — **look at a real chart**:
- In Data Studio, the source picker now lists `demo-buildings` (tables:
  `site` / `meter` / `point` / `point_reading` + the `*_tag` Haystack tables). Open a chart
  bound to a seeded point (e.g. an Energy kWh point) over a window inside the seeded month.
  It must **show the series** — not an empty plot (empty = the un-seeded symptom, go back to
  Step 0).
- The **time axis must be right**: readings are 15-min-spaced across the last month. A
  chart squashed to 1970 or a single point is the epoch-seconds-vs-ms mixup — the viz
  bridge renders epoch-**seconds** (`dateUnit:"s"`), memory: flow-ts-prefs-display.
- The shape must be **plausible**: daily energy/water cycles and HVAC state, not a flat line
  or noise.
- This is eyes-on the running chart (via `make dev`, real gateway → federation sidecar →
  seeded SQLite file). No `*.fake.ts` anywhere in that path (testing-scope §0).

---

## Step 4 — on a blank/wrong chart, diagnose the seam in order

Before opening a debug entry, rule out the cheap causes (most "broken chart" reports are
one of these, not a code bug):

1. **Is the dataset seeded and registered?** `sqlite3 …/buildings.db "SELECT COUNT(*) FROM
   point_reading;"` → 0 (or the file missing) means re-run Step 0; `curl $BASE/datasources`
   with `demo-buildings` absent means `make seed-demo-sqlite` didn't register it (was the
   node up first?). This is the most common cause.
2. **Is federation enabled?** `make dev` with `FED_ENDPOINTS` non-empty (default includes
   `127.0.0.1:0` for sqlite); the sidecar installed; the endpoint pre-approved.
3. **Is the DSN path node-local?** The sidecar resolves the DSN on the **node**, not the
   browser — a path that only exists on your laptop probes with a clean error naming this.
   `make seed-demo-sqlite` writes under the node's own data dir for exactly this reason.
4. **Stale node?** The node doesn't hot-reload Rust — `make kill && make dev` after a
   crate change (memory: flows-dev-node-no-hot-reload).
5. **Wrong time unit?** Epoch seconds vs ms — see 3.4.

Only once the seam is real and still wrong: open `../../debugging/datasources/<symptom>.md`
per [`../../scope/debugging/debugging-scope.md`](../../scope/debugging/debugging-scope.md),
find the root cause, add a **regression test** (a `cargo test -p lb-host …` or
`*.gateway.test.tsx` that fails-before/passes-after), and update `debugging/README.md`.

---

## Step 5 — what to leave behind (definition of done)

- Proof the dataset was **seeded** (the `COUNT(*)` output) and **registered** (the source in
  `/datasources`), plus the **observed result** in the session doc.
- The CRUD + permissions + access checks (mandatory) and the functional chart-renders-data
  check.
- **Left inspectable so the user can confirm:** the `buildings.db` still present, the
  `demo-buildings` datasource still registered, the node still running (`make dev`), and the
  chart still bound to seeded data. Your **final response hands the user the exact page** —
  e.g. "open **Datasources → demo-buildings** and the **Energy** chart at
  http://127.0.0.1:8080; I left them in place so you can check the chart shows the month of
  readings." Do **not** remove the source or the chart (README "Leave it inspectable").
- On any failure: a completed `debugging/datasources/…` entry + regression test, cross-linked.

---

## Appendix — the full-year Postgres/TimescaleDB path (optional, if you have Docker)

The SQLite demo above is the default because it needs **zero external dependency**. If you
specifically want the full-year firehose (~35M rows, 8 sites, 5-min interval) against a real
TimescaleDB, that path still works:

```bash
cd docker/postgres && docker compose up -d      # TimescaleDB → 127.0.0.1:5433, db lb / lb:lb_secret
./seed.sh                                        # full year @ 5-min (~6 min); or ./seed.py --months 1 --interval 15
PGPASSWORD=lb_secret psql -h 127.0.0.1 -p 5433 -U lb -d lb -c "SELECT COUNT(*) FROM point_reading;"
```

`make dev` pre-approves `net:tls:127.0.0.1:5433` and pre-registers a `timescale` source, so
the same Step 3 checklist applies — just against the `timescale` source instead of
`demo-buildings`. Everything above is otherwise identical.
