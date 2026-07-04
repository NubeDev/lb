---
name: e2e-datasources
description: >
  Use when asked to end-to-end test datasources / federation, or to test any CHART that
  reads external time-series. Prove a datasource works as designed against a REAL node +
  a REAL TimescaleDB — MUST bring up docker/postgres AND run its seed.sh first (charts are
  blank without the seed), then run the CRUD / permissions / access / functional checks.
---

# E2e datasources runbook — prove a datasource (and its charts) work as designed

Status: scope (the standard). Design intent: [`../../scope/datasources/datasources-scope.md`](../../scope/datasources/datasources-scope.md).
The checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** The automated code tests
(`cargo test`, vitest) are the **scope/session's** job and are assumed already green — this
runbook does **not** re-run them. Its job is to **drive the actual running system and
observe it behave**: a live node, a live DB, a chart that either shows the data or doesn't.

**It proves a datasource does what it was designed to do — it is not for reporting bugs.**
A datasource is the one seam that reaches a **true external** you can't run in-process: a
real Postgres/TimescaleDB. So it has a hard prerequisite — **the external DB must be up and
seeded** — and everything downstream (the federation sidecar, the `datasource.*` verbs, and
every **chart** that reads them) is blank or wrong until it is.

> The federation fake-of-a-true-external rule (testing-scope §0): the provider you can't
> run locally is faked behind one trait; but here the DB *can* run locally (it's just
> Docker), so **run the real DB** — don't fake it. Seed real rows; charts read real data.

---

## ⚠️ Step 0 — bring up AND seed the database (do this first, always)

Charts read time-series from TimescaleDB via the federation datasource. **No seed → empty
tables → blank charts → a false "the chart is broken".** This is the #1 datasources
false-bug. Seed before anything else.

```bash
cd docker/postgres
docker compose up -d                 # TimescaleDB → 127.0.0.1:5433 (NOT 5432), db lb / lb:lb_secret
./seed.sh                            # one year of energy + water readings, 15-min slots (~490k rows)
```

The seed builds the model the charts expect:

```
site (3)  →  meter (7)  →  point (14: kWh / kW / L·min / m³)  →  point_reading (hypertable, ~490k rows)
```

**Verify the seed landed** before trusting any chart result:

```bash
PGPASSWORD=lb_secret psql -h 127.0.0.1 -p 5433 -U lb -d lb -c \
  "SELECT COUNT(*) FROM point_reading;"          # expect ~490 000, not 0
```

- `seed.sh` is idempotent (`ON CONFLICT DO NOTHING` + a windowed delete/reinsert of
  readings) — safe to re-run. Defaults match `docker-compose.yml`; override with
  `--host/--port/--user/--db/--password`.
- No local `psql`? `seed.sh` auto-falls back to `docker exec` inside `lb-timescaledb`.

---

## Step 1 — read the design (what is "correct"?)

- **[`../../scope/datasources/datasources-scope.md`](../../scope/datasources/datasources-scope.md)** — the ask: what a datasource is, the
  `datasource.add/remove/list` verbs, **DSN redaction**, endpoint pre-approval, ownership.
- Paging / decimation behaviour the charts depend on:
  [`federation-paging-scope.md`](../../scope/datasources/federation-paging-scope.md),
  [`series-decimation-scope.md`](../../scope/datasources/series-decimation-scope.md),
  [`series-paging-scope.md`](../../scope/datasources/series-paging-scope.md).
- **`../../skills/<name>/SKILL.md`** — the drivable `datasource.*` / query surface, if one
  exists yet (note the gap if not).

---

## Step 2 — point the node at the seeded DB

The Makefile already wires the node to this exact DB (`FED_*` defaults target
`127.0.0.1:5433`). Booting `make dev` installs + supervises the federation sidecar,
pre-approves the endpoint (`net:tls:127.0.0.1:5433`), and pre-registers one source
(`timescale`) so the Datasources page + charts work on first boot:

```bash
make dev                             # federation on by default (FED_ENDPOINTS=127.0.0.1:5433)
# make dev FED_ENDPOINTS=            # would DISABLE federation — don't, for a datasources test
```

Now you have a **live system** — a real node, wired to a real seeded DB, serving the
Datasources page and the charts. The checks below drive *that*, by hand or by driving the
real surface. You do **not** re-run `cargo test` here — the code-level suites
(`federation_test.rs`, `datasource_crud_ownership_test.rs`, `viz_query_test.rs`, the
`/datasources` route test) are the scope/session's job and are assumed green already. This
runbook is the **real-world** check on top: does the running thing actually behave?

---

## Step 3 — the checklist (prove it works as designed)

The four dimensions from [`../README.md`](../README.md#what-to-check--the-functional-dimensions),
against the real node + real seeded DB.

### 3.1 CRUD — the datasource lifecycle round-trips
On the live Datasources page (or by driving the real `datasource.*` verbs):
- **add** a source (a real DSN to `127.0.0.1:5433`), see it **appear** in the list.
- prove **remove** on a **throwaway** second source you add just for the check — then
  **leave the working `timescale` source in place** so the user (and the chart) can use it.
- **Redaction (design rule):** the DSN/password **never** shows in the list — only a
  secret ref. Confirm the raw password is nowhere in the UI/response.

  *(The design rule is pinned by `datasource_crud_ownership_test.rs` — that's the
  scope/session's automated guard; here you're confirming the running system honours it.)*

### 3.2 Permissions — the capability wall holds
Every verb needs its grant; without it, refused:
- `mcp:datasource.add:call`, `mcp:datasource.remove:call`, `mcp:datasource.list:call`.
- Connecting out needs the endpoint cap `net:tls:<host>:<port>:connect` — **without** the
  pre-approved endpoint, the connect is denied (not silently attempted). Assert the deny.

### 3.3 Access — the workspace wall holds
A datasource is workspace-scoped. Add a source in workspace A; assert workspace B can
neither `list` nor query it. Ownership is checked before caps (see the ownership test).

### 3.4 Functional — the charts render the seeded data
This is the payoff, and the whole reason the seed matters — **look at a real chart**:
- Open a chart bound to a seeded point (e.g. `pt-001` Energy kWh) over a window inside the
  seeded year. It must **show the series** — not an empty plot (empty = the un-seeded
  symptom, go back to Step 0).
- The **time axis must be right**: readings are 15-min-spaced across the last year. A
  chart squashed to 1970 or a single point is the epoch-seconds-vs-ms mixup — the viz
  bridge renders epoch-**seconds** (`dateUnit:"s"`), memory: flow-ts-prefs-display.
- The shape must be **plausible**: daily energy/water cycles, not a flat line or noise.
- This is eyes-on the running chart (via `make dev`, real gateway → federation → seeded
  DB). No `*.fake.ts` anywhere in that path (testing-scope §0).

---

## Step 4 — on a blank/wrong chart, diagnose the seam in order

Before opening a debug entry, rule out the cheap causes (most "broken chart" reports are
one of these, not a code bug):

1. **Is the DB up and seeded?** `SELECT COUNT(*) FROM point_reading;` → 0 means re-run
   Step 0. This is the most common cause.
2. **Is federation enabled?** `make dev` with `FED_ENDPOINTS` non-empty; the sidecar
   installed; the endpoint pre-approved.
3. **Stale node?** The node doesn't hot-reload Rust — `make kill && make dev` after a
   crate change (memory: flows-dev-node-no-hot-reload).
4. **Wrong time unit?** Epoch seconds vs ms — see 3.4.

Only once the seam is real and still wrong: open `../../debugging/datasources/<symptom>.md`
per [`../../scope/debugging/debugging-scope.md`](../../scope/debugging/debugging-scope.md),
find the root cause, add a **regression test** (a `cargo test -p lb-host …` or
`*.gateway.test.tsx` that fails-before/passes-after), and update `debugging/README.md`.

---

## Step 5 — what to leave behind (definition of done)

- Proof the DB was **seeded** (the `COUNT(*)` output) and the **observed result** in the
  session doc.
- The CRUD + permissions + access checks (mandatory) and the functional chart-renders-data
  check.
- **Left inspectable so the user can confirm:** the DB still up + seeded, the `timescale`
  datasource still registered, the node still running (`make dev`), and the chart still
  bound to seeded data. Your **final response hands the user the exact page** — e.g. "open
  **Datasources → timescale** and the **Energy** chart at http://127.0.0.1:8080; I left
  them in place so you can check the chart shows the year of readings." Do **not** remove
  the source or the chart (README "Leave it inspectable").
- On any failure: a completed `debugging/datasources/…` entry + regression test, cross-linked.
