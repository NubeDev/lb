# Testing scope — how to test

Status: scope (the standard). Promotes to `public/testing/` once the harness exists.

How every AI session (and human) tests work in this repo. The goal is not a coverage
number; it is that **behavior is proven, regressions are caught, and the capability
wall is never quietly breached.**

> Companion: `../debugging/debugging-scope.md` (how to debug + the working-history system).
> Every bug fix ends as a regression test here — that is where the two systems meet.
>
> Runbooks: `../../testing/` holds the **e2e runbooks** — the imperative "how to actually
> drive a real backend/frontend end to end" (`e2e-backend.md`, `e2e-frontend.md`). This
> doc is the *policy* they obey; that folder is the *procedure* an AI executes.

---

## 0. No mocks. No fake backends. (the hard rule)

This is the most important rule on the page and a non-negotiable (`CLAUDE.md` §9).

**Nothing that can run in-process gets mocked.** SurrealDB embeds (`mem://`), Zenoh
runs in-proc, capabilities and the gateway are real code. So *use the real thing* —
in unit tests, integration tests, and demos. The cost that normally justifies a mock
(a slow or external dependency) does not exist here.

- **Banned: a parallel re-implementation of node behavior.** No `*.fake.ts` dispatcher,
  no in-memory "faithful node", no hand-written stand-in that answers the way the
  backend would. It lets a feature *look* shipped while the real path is unbuilt, and
  — the reason we care — **an AI reading the code cannot tell the fake from the truth**,
  so it builds the next layer on a lie. Real code fails loudly; a fake passes quietly.
- **Allowed: a fake of a true external only.** Something you genuinely cannot run
  locally — a model-provider HTTP API, GitHub, a paid service. Put it behind **one
  trait**, in **one clearly-named file** (`provider_fake.rs`), swappable with the real
  client. That is the *entire* allow-list.
- **Need data? Seed it — don't simulate it.** Inserting real rows into a real
  (ephemeral) store is not a mock; it flows through the real code path. See §3.1.

Smell test: *if this stand-in disappeared, would an unbuilt code path be exposed?* If
yes, it's a banned fake — build the real path instead. If it only supplies **data**
to a real path, it's a seed, and it's fine.

The frontend is included. UI behavior is proven against a **real node** (an in-process
gateway / real backend over its real transport), seeded with real records — not a
`fake.ts` that re-implements the verbs. A thin transport shim is fine; a second backend
is not.

---

## 1. The layers (test pyramid)

Cheap and many at the bottom, expensive and few at the top.

| Layer | What it covers | Speed | Where |
|---|---|---|---|
| **Unit** | Pure logic in one file: validation, mapping, a state transition. No IO. | ms | beside the source (see FILE-LAYOUT) |
| **Integration** | A crate boundary with real embedded SurrealDB + in-proc Zenoh: e.g. a capability check from token → store access. | 10s–100s ms | `rust/<crate>/tests/` |
| **Contract** | The *stable boundaries*: WIT/SDK interface and MCP tool schemas. Snapshot them so a break is loud. | fast | per boundary crate |
| **Property / fuzz** | The risky invariants: the capability/scope grammar, any parser. Generate inputs, assert invariants. | varies | the core crates that own the risk |
| **Frontend** | Components and hooks (Vitest + React Testing Library). | fast | beside the component |
| **E2E / workflow** | A full edge↔cloud flow end to end (the worked example: inbox → triage → approval → job → outbox). | slow | a dedicated `e2e/` suite |

Most tests are unit and integration. E2E proves the seams, not every branch.

---

## 2. Mandatory test categories (not optional)

These come straight from the core principles in `../../../README.md` §3. A feature is
not done until these exist where they apply:

1. **Capability deny-tests.** Capability-first security means the important test is the
   *negative* one: without the grant, the call is refused. Every new tool/record/bus
   access ships with a "denied without capability" test, not just the happy path.
2. **Workspace-isolation tests.** Workspace is the hard wall. Any feature touching data
   gets a test proving workspace B cannot see/write workspace A's keys — across all
   three surfaces (store, bus, MCP).
3. **Offline / sync tests.** For anything synced: write offline on an edge, reconnect,
   assert idempotent apply and the §6.8 authority/merge rules hold.
4. **Hot-reload tests.** For extension-runtime changes: swap a component and assert no
   durable state is lost (stateless-extension principle).
5. **Regression tests.** Every bug fix adds a test that **fails before the fix and
   passes after** (see the debug system).
6. **Prior-state / upgrade tests.** For anything that changes a **default**, a **stored
   shape**, or a **precedence rule**: seed the state a PREVIOUS version would have left
   behind, then assert the new behaviour actually takes effect. A suite whose every test
   starts from a bare host cannot see an upgrade bug — see §3.2.

---

## 3. Cross-cutting rules

- **Determinism.** No wall-clock or randomness in test logic — inject a clock and seed
  RNG/IDs. A test that can flake is a bug. (Same reason the agent harness bans
  `Date.now()`/`Math.random()` in scripts.)
- **Real everything, not mocks — see §0.** SurrealDB is embeddable; spin up an
  ephemeral in-memory namespace per test instead of mocking the store. Same for Zenoh,
  caps, and the gateway. Fake only a true external, behind one trait in one named file.
- **One test file per source file**, mirroring the tree (FILE-LAYOUT §4 Tests). Split a
  test file by scenario once it passes ~5 tests.
- **Fixtures are factories, not fixtures-of-doom.** A `workspace()` / `member()` /
  `granted(cap)` builder, named by what it creates — never a giant `setup.rs`.

### 3.1 Seeding (the sanctioned way to get data)

When a test or a demo needs existing data, **seed real records through the real write
path** — call the actual create verb (or insert into the embedded store) so the data is
indistinguishable from production data, carries real workspace scoping, and exercises the
real capability check. This is the *opposite* of a mock: a mock replaces the code path; a
seed *feeds* it.

- Seed with the same factory builders fixtures use (`workspace()`, `granted(cap)`), or a
  named `seed_<thing>` helper — one file, says what it creates.
- A dev/demo seed (so a fresh build isn't empty) lives in **one** named seed entrypoint
  and writes to the real store on boot — never a `fake.ts` that answers reads from memory.
- Seeds respect the workspace wall like everything else: seed into workspace A, and the
  isolation test still proves workspace B can't see it.
- **Test names state the behavior:** `denies_get_without_read_grant`, not `test_get_2`.
- **Generated code is exempt** from authoring tests by hand (FILE-LAYOUT §4).

### 3.2 The blind spots a green suite has

Standing work to close these in CI: [#108](https://github.com/NubeDev/lb/issues/108) — **the four
structural gaps below are now closed** (2026-07-27, `sessions/testing/blind-spots-track-{a,b}-session.md`).
What is closed is the *mechanism*, not the whole tree: the fixture, the boot-wiring assertions, the
multi-batch counts and the axis sweeps exist and are revert-checked, but they cover the ingest / viz /
retention paths that motivated them. Applying each shape to a new area is still per-slice work — that is
what the table is for.

| Shape | Now in the tree |
|---|---|
| Prior-state | `crates/host/tests/support/prior_state.rs` (`PriorRetention` / `PriorSeries`), shared via `#[path]`. Mandatory category 6 above. It found a real upgrade bug on its first run — `debugging/ingest/pre-cap-policy-row-aborts-the-retention-pass.md`. |
| Boot wiring | `node/tests/boot_wiring_test.rs`, asserting through `reactors::spawn` itself — deleting a `spawn_*_reactors` line now fails a test. |
| Multi-batch | Drain/paging/eviction loops driven past `COMMIT_BATCH` (256) / `CAP_EVICT_BATCH` (5 000) / the insights page. |
| Axis | `crates/ingest/tests/series_width_axis_test.rs`, `crates/host/tests/viz_resolution_axis_test.rs`. |

Real bugs that shipped past a **fully green** suite, each because the tests all shared one
unstated assumption. These are the shapes to go looking for; they are cheap to test once named.

| Blind spot | What it looks like | The test that catches it |
|---|---|---|
| **Every test starts from a bare host.** | A changed default is correct on a fresh install and dead on every existing one — the old value is still on disc, and if it sits at a *higher-precedence* key it wins forever. The new code reports success; the old data reports nothing. | Seed the previous version's rows/config, run boot convergence, assert the new default governs. A migration is **part of** a defaults change, not a follow-up. |
| **A loop enumerates the outcomes that existed when it was written.** | Adding a third terminal outcome for a unit of work silently breaks every loop-termination condition that listed the old two — including ones in other crates the change never touched. | Drive **more than one batch/page** through the loop. A single-batch test passes against the bug. Grep for the counters, not the call sites. |
| **One configuration is exercised, not the axis.** | A behaviour keyed on an exact match (a width, a version, a size) works at the tested value and silently degrades everywhere else — often falling back to something plausible and wrong. | Assert across the axis, not at one point: loop the widths/sizes and assert the property holds at each. |
| **The boot wiring itself.** | Deleting the one line that spawns a reactor breaks **no test**. The mechanism is covered; its heartbeat is not. | Assert the property with nobody calling the verb (see the retention/drain reactors), and treat "it runs" as part of the feature. |

The common thread: **a test that shares the bug's assumption cannot see the bug.** When a change
alters a default, a precedence rule, or a set of outcomes, the first question is "what does the
suite assume that this change just made untrue?" — and the honest answer usually needs a live run
against a system with history on it (`verify-in-product-not-suite`).

---

## 4. How to run (intended — fill in real commands when the harness lands)

- Rust: `cargo test` (consider `cargo nextest run` for speed/isolation).
- Rust property/fuzz: `cargo test` for proptest; `cargo fuzz` for the capability grammar.
- Frontend: `pnpm test` (Vitest), `pnpm test:e2e` (Playwright).
- All layers run in CI on every PR, alongside the FILE-LAYOUT size check.

---

## 5. What each AI session must do

- Ship tests **in the same session** as the behavior change — never "tests later".
- Add the mandatory categories (§2) that apply to the change.
- For any bug fixed this session: add the regression test and link it from the debug
  entry (`../debugging/debugging-scope.md`).
- Record in the session doc *what was tested and the command output* — green is a claim
  that must be shown, per `../../ABOUT-DOCS.md` definition of done.

---

## 6. Open questions

- Runner: `cargo test` vs `cargo nextest` as the standard; Vitest vs Jest for UI.
- E2E harness: how to stand up a hub + edge pair in-process for workflow tests.
- Contract snapshots: format and where the golden files live for WIT + MCP schemas.
- Coverage signal: do we gate CI on anything, or rely on the mandatory categories?
- Fuzzing the capability grammar: corpus storage and CI budget.
- **Retiring the existing `*.fake.ts` layer** (§0): the UI currently tests against a
  hand-written node fake. Migrate UI tests onto a real in-process gateway seeded with
  real records, then delete the fakes. Decide the smallest real-node harness for Vitest.
