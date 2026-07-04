---
name: e2e-backend
description: >
  Use for REAL-WORLD verification of the Rust backend — boot a live node (make dev) and
  drive the feature over its real MCP/REST surface to confirm it behaves as designed
  (CRUD, permissions, access, functional). Assumes the automated cargo-test suite already
  passed (that's the scope/session's job) — this does NOT re-run tests.
---

# E2e backend runbook — verify a backend feature in the real world

Status: scope (the standard). Companion policy: [`../scope/testing/testing-scope.md`](../scope/testing/testing-scope.md).
The checklist (CRUD / permissions / access / functional): [`README.md`](README.md#what-to-check--the-functional-dimensions).

**This is real-world verification, not the test suite.** `cargo test` is the
**scope/session's** job and is assumed already green — this runbook does **not** re-run it.
Its job is to **drive a live node and observe it behave**: boot the real thing, exercise
the feature over its real surface, watch the outcome the scope promised actually happen.

**It proves a feature does what it was designed to do — it is not for reporting bugs.** You
start from the design (scope + skill), drive the running node, and run the CRUD /
permissions / access / functional checks. A bug you *find* gets filed as a `debugging/`
entry (§4) — it does not live here.

The backend is driven as a **real node** — never a mock or a `*.fake` dispatcher
(testing-scope §0): real embedded SurrealDB, real in-proc Zenoh, real capability check.

---

## Step 0. Read the design first (what is "correct"?)

Before driving anything, know what the feature is *supposed* to do — you can't observe
behavior you can't name:

- **`../scope/<topic>/<name>-scope.md`** — the ask: the entities, the access rules, the
  workflow, the edge cases. This is what you verify **against**.
- **`../skills/<name>/SKILL.md`** — the drivable surface: the exact `<id>.<tool>` verbs /
  REST routes / payloads to exercise the feature, grounded in a live node. This is *how*
  you drive it. If the feature has no skill yet and it's agent-drivable, that's a gap to
  note (ABOUT-DOCS definition of done).

---

## Step 1. Stand up the running system

Boot a real node and drive *it*. (The automated suites under `rust/crates/*/tests/` already
exercise these paths in-process — that's the scope/session's green, not something you
re-run here.)

```bash
make build-wasm          # REQUIRED first — the node reads hello/hello-v2 .wasm at startup
make dev                 # boots the dev node (root node on 8080) + gateway
# NOTE: the node does NOT hot-reload Rust — after a crate change, `make kill && make dev`,
#   else you're driving a STALE binary (the #1 false "it's broken").
```

Now drive the feature over its real MCP/REST surface with the same `<id>.<tool>:call`
grammar the UI uses (the verbs your skill documents). Seed via the real verbs; never insert
a fake read-path. You are watching the live system, not asserting on internal state.

---

## Step 2. The checklist — drive it, observe it works as designed

The four functional dimensions from [`README.md`](README.md#what-to-check--the-functional-dimensions),
driven against the live node and **observed**. CRUD + permissions + access are
**mandatory** for any data-touching feature; functional is the feature-specific proof.
Each one is a real interaction over the feature's verbs, then a look at what came back — not
an assertion you write in code (that's the scope/session's suite).

### 2.1 CRUD — the data lifecycle round-trips
For each entity the scope says the feature owns, drive the full lifecycle over the **real
verbs** and read it back over the **real read verb**:
- **create** it → **read** it back, confirm the shape matches the scope doc.
- **update** a field → read again, confirm the change landed.
- **delete** — prove it on a **throwaway** record made just for this check; read again,
  confirm it's gone.

A create you never read back shows nothing. Seed only through real write paths
(testing-scope §3.1) — seeded data must be indistinguishable from produced data.
**Leave the primary record in place** so the user can inspect it on the running node; only
the delete-check throwaway gets removed (see README "Leave it inspectable").

### 2.2 Permissions — the capability wall holds
The *negative* path is the point (capability-first). For each verb: as a member **with**
the grant it succeeds; as a member **without** it, it's **refused** (a real deny, not a
silent no-op or a crash). Dev-login caps are narrow — e.g. `mcp:prefs.set_default:call` is
**not** in `member_caps()`; for a ws-default write, sign in an admin via `signInWithCaps`
(memory: dev-login-missing-set-default-cap).

### 2.3 Access — the workspace wall holds
Seed/create data in workspace A; sign in to workspace B; confirm B can neither read nor
write it — across store, bus, and MCP. Workspace is checked *before* caps.

### 2.4 Functional — the behavior the scope promises
The feature's actual job: the state transitions, the workflow, the computed result, the
edge cases the scope calls out. Drive the real end-to-end path on the live node and watch
the designed outcome happen — e.g. the worked example `inbox → triage → approval → job →
outbox` completes and lands in the outbox. This is scope §"expected behavior", observed on
the running system.

### 2.5 If it applies
**Offline/sync** (synced features) and **hot-reload** (extension-runtime changes) — see
testing-scope §2.

> Observe the real thing, not the harness: read back through the **real read verb**, never
> internal state. If a check needs deterministic time/IDs, the node's own seams handle it
> (`Gateway::new(., now)` in test, `new_live` in prod — memory: flows-run-id).

---

## Step 3. What you found

- **Works as designed?** Record what you drove and what you observed in the session doc —
  a claim of "it works" must show the interaction, not just assert it.
- **Something's off?** That's a *finding* → Step 4. Do not write it up in this folder.

---

## Step 4. On a wrong result — file it as a debug finding

The automated suite is assumed green, so a real-world failure here means the **running
system** diverges from the design. Don't patch it in place — file it:

1. Open `../debugging/<area>/<symptom>.md` named by what you observe
   (`caps/cross-workspace-read-leaks.md`), per `../scope/debugging/debugging-scope.md`.
2. Reproduce minimally, find the root cause at the right layer, fix.
3. Add a **regression test** that fails-before / passes-after (this *does* go in the
   `rust/<crate>/tests/` suite — a real-world finding earns a permanent automated guard).
   Update `debugging/README.md`. Cross-link session ↔ entry ↔ test.

First rule out the cheap false-bugs before blaming code: **stale node** (didn't
`make kill && make dev` after a Rust change — memory: flows-dev-node-no-hot-reload), or a
**pre-existing** red like `agent_routed_test` (memory: preexisting-failing-tests).

---

## Step 5. What to leave behind (definition of done)

- The **observed result** in the session doc — the interaction you drove and what came back.
- The mandatory checks covered (CRUD + permissions + access) plus the functional check.
- **The system left inspectable:** the node still running (`make dev`), the records you
  created still in place (not torn down), and a **hand-off in your final response** telling
  the user the verb/page to open and what to expect — so *they* confirm it works, not just
  you (README "Leave it inspectable").
- On any finding: a completed debug entry + regression test, cross-linked.
