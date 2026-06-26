# Testing scope — how to test

Status: scope (the standard). Promotes to `public/testing/` once the harness exists.

How every AI session (and human) tests work in this repo. The goal is not a coverage
number; it is that **behavior is proven, regressions are caught, and the capability
wall is never quietly breached.**

> Companion: `../debugging/debugging-scope.md` (how to debug + the working-history system).
> Every bug fix ends as a regression test here — that is where the two systems meet.

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

---

## 3. Cross-cutting rules

- **Determinism.** No wall-clock or randomness in test logic — inject a clock and seed
  RNG/IDs. A test that can flake is a bug. (Same reason the agent harness bans
  `Date.now()`/`Math.random()` in scripts.)
- **Real datastore, not mocks, at the integration layer.** SurrealDB is embeddable;
  spin up an ephemeral in-memory namespace per test instead of mocking the store. Mock
  only true externals (a provider HTTP API, GitHub).
- **One test file per source file**, mirroring the tree (FILE-LAYOUT §4 Tests). Split a
  test file by scenario once it passes ~5 tests.
- **Fixtures are factories, not fixtures-of-doom.** A `workspace()` / `member()` /
  `granted(cap)` builder, named by what it creates — never a giant `setup.rs`.
- **Test names state the behavior:** `denies_get_without_read_grant`, not `test_get_2`.
- **Generated code is exempt** from authoring tests by hand (FILE-LAYOUT §4).

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
