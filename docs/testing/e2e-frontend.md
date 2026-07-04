---
name: e2e-frontend
description: >
  Use for REAL-WORLD verification of the React/UI frontend — boot the live app (make dev)
  and click through the running UI against a real gateway to confirm a feature behaves as
  designed (CRUD, permissions, access, functional). Assumes the vitest / test:gateway suite
  already passed (the scope/session's job) — this does NOT re-run tests.
---

# E2e frontend runbook — verify a UI feature in the real world

Status: scope (the standard). Companion policy: [`../scope/testing/testing-scope.md`](../scope/testing/testing-scope.md).
The checklist (CRUD / permissions / access / functional): [`README.md`](README.md#what-to-check--the-functional-dimensions).

**This is real-world verification, not the test suite.** The vitest suites (`pnpm test`,
`pnpm test:gateway`) are the **scope/session's** job and are assumed already green — this
runbook does **not** re-run them. Its job is to **open the real running app and look at
it**: drive the actual UI against a live gateway and confirm the feature behaves as the
scope promised, on screen.

**It proves a UI feature does what it was designed to do — it is not for reporting bugs.** A
bug you *find* is filed as a `debugging/` entry (§4), not written up here.

The UI is driven against a **real node** — a live gateway over its real transport, seeded
with real records. A `*.fake.ts` that re-implements the verbs is **banned** (testing-scope
§0, CLAUDE.md §9): you can't tell the fake from the truth. A thin transport shim is fine; a
second backend is not.

---

## Step 0. Read the design first (what is "correct"?)

- **`../scope/<topic>/<name>-scope.md`** — the ask: what the screen/flow should do, which
  members can see/do what, the states it must render. Verify **against** this.
- **`../skills/<name>/SKILL.md`** — the drivable surface (verbs/routes) the UI sits on,
  grounded in a live node — the same real surface the UI drives.

You confirm the *rendered UI* matches the scope's promised behavior.

---

## Step 1. Stand up the running app

Boot the real node + UI and drive the app in a browser. (The `*.gateway.test.tsx` suite
already exercises the UI↔gateway seam in-process via `pnpm test:gateway` — that's the
scope/session's green, not something you re-run here.)

```bash
make build-wasm          # required before the node boots
make dev                 # cloud node (gateway on 8080) + UI (browser build) together
```

Open the app, sign in, and go to the feature's screen. You drive the same real gateway the
UI always talks to; data is seeded through real verbs (never a fake read-path). You're
looking at the live product, not asserting on component state.
- Second user? `addMember` before the second-user login (memory:
  app-shell-standalone-and-sse). SSE resume is history catch-up, no `Last-Event-ID`.
- Port gotcha: the RN browser preview gateway is **8087**, not 8080 (8080 = root node) —
  an 8087/8080 collision surfaces as a spurious 403 (memory: app-preview-port-and-prefill).

---

## Step 2. The checklist — drive the UI, observe it works as designed

The four dimensions from [`README.md`](README.md#what-to-check--the-functional-dimensions),
observed on the running screen. CRUD + permissions + access are **mandatory** for any
data-touching screen; functional is the feature-specific proof. Each is a real click-through
and a look at the result — not an assertion you write in code.

1. **CRUD.** Use the feature's create/edit/delete UI: creating shows the new row, editing
   re-renders the change, deleting removes it. Confirm on screen (and it survives a
   refresh — that proves it round-tripped the store, not local state). Prove **delete** on
   a **throwaway** item; **leave the primary item on screen** for the user to inspect (see
   README "Leave it inspectable").
2. **Permissions.** Sign in as a member **without** the grant; the UI must surface the deny
   (disabled/blocked/error) — not a silent success or a broken screen. Dev-login caps are
   narrow — for a ws-default write, sign in an admin via `signInWithCaps` (memory:
   dev-login-missing-set-default-cap).
3. **Access.** Create data in workspace A; sign in to workspace B; B's UI must **never**
   show A's records. The wall is checked before caps.
4. **Functional.** The screen's actual job as the scope describes it: the flow completes,
   the right states render (loading / empty / error / success), a live update arrives over
   SSE. Watch the designed outcome happen — not just that the page mounted.

Eyes-on the running screen. Headless screenshots can come back blank — read `innerText`
if you're scripting the check (memory: app-preview-port-and-prefill).

---

## Step 3. What you found

- **Works as designed?** Record what you clicked and what you saw in the session doc.
- **Something's off?** → Step 4. Not written up in this folder.

---

## Step 4. On a wrong result — file it as a debug finding

The vitest suite is assumed green, so a real-world UI failure means the **running app**
diverges from the design. File it, don't paper over it:

1. Open `../debugging/frontend/<symptom>.md` per `../scope/debugging/debugging-scope.md`.
2. Reproduce, root-cause at the right layer, fix.
3. Add a **regression test** (a `*.gateway.test.tsx` that fails-before/passes-after) — a
   real-world finding earns a permanent automated guard. Update `debugging/README.md`,
   cross-link session ↔ entry ↔ test.

Rule out the cheap false-bugs first: a **stale node** after a Rust change (the node doesn't
hot-reload — `make kill && make dev`, memory: flows-dev-node-no-hot-reload), or a
**pre-existing** red (`SystemView.gateway`, `sqlSource.gateway` — memory:
preexisting-failing-tests).

---

## Step 5. What to leave behind (definition of done)

- The **observed result** in the session doc — what you drove on screen and what you saw.
- The mandatory checks covered (CRUD + permissions + access) plus the functional check.
- **The app left inspectable:** node + UI still running (`make dev`), the records you
  created still on screen, and a **hand-off in your final response** — the page to open and
  what to expect — so the user can confirm it themselves (README "Leave it inspectable").
- On any finding: a completed `debugging/frontend/…` entry + a regression
  `*.gateway.test.tsx`, cross-linked.
