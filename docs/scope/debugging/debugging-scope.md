# Debugging scope — how to debug + the working-history system

Status: scope (the standard). Promotes to `public/debugging/` once stable.

Two things, designed together:

1. **How to debug** — the standard approach when "it's not working".
2. **The working history** — a persistent, symptom-searchable record of every issue and
   how it became working, so **nothing is ever debugged twice.**

> Companion: `../testing/testing-scope.md`. Every resolved bug ends as a regression test
> there. A debug entry without a regression test is not finished.

---

## 1. Where the history lives

`docs/debugging/` is a peer knowledge base (like `vision/` — outside the scope→sessions→
public flow). It is **append-only and symptom-led**:

```
docs/debugging/
  README.md                         ← index: every entry + status (the "history")
  <area>/<symptom-slug>.md          ← one issue per file, named by the symptom
```

- One file per issue, named by **what you observe**, not the cause you found later:
  `caps/cross-workspace-read-leaks.md`, `sync/edge-writes-lost-after-reconnect.md`.
- `<area>` matches the topic names used across the docs (`caps`, `sync`, `mcp`, …).
- The `README.md` is the **working history**: a dated table of entries and their status,
  newest first. It reads as the project's debugging memory.

### Debug entry vs session doc vs test

These do not overlap — each has one job:

| Artifact | Job |
|---|---|
| `sessions/<topic>/…-session.md` | The chronological narrative of one work session. |
| `debugging/<area>/<symptom>.md` | The reusable **symptom → cause → fix** card, extracted so it's findable later. |
| Regression test | The machine-checked guarantee the bug stays dead. |

A debug episode happens inside a session; the session narrates it, the debug entry
distills it, the test guards it. Cross-link all three.

---

## 2. The debugging method (how to debug)

Follow this order; record it in the debug entry as you go (the timeline *is* the value):

1. **Reproduce.** Get a reliable, minimal repro first. A bug you can't reproduce, you
   can't fix — write down the exact steps/environment.
2. **Observe, don't guess.** Capture the actual error, logs, and state. Look at the
   thing before theorizing. If a "known" fact contradicts what you see, trust what you see.
3. **Narrow the surface.** Bisect: which crate/layer/commit? Rule things out and *write
   down what you ruled out* — that's what saves the next person.
4. **Find the root cause, not the symptom.** "Added a `?`" is a symptom patch. Name the
   actual cause ("LIVE notification treated as durable; lost on reconnect — §6.2").
5. **Fix at the right layer.** Prefer the fix that makes the whole class impossible over
   the one that silences this instance.
6. **Verify.** Reproduce again — now it works. Capture the proof.
7. **Guard.** Add the regression test (§testing). Note any guardrail that would have
   prevented it (a type, an assert, a capability check).

---

## 3. When to open a debug entry

Open one as soon as something doesn't work and the cause isn't obvious in under a few
minutes. Cheap to start, valuable forever. Always close the loop:

- **Resolved** → fill in root cause + fix + regression test link.
- **Open / blocked** → leave it open with what's been ruled out, so the next session
  resumes instead of restarting.
- **Won't-fix / by-design** → record *why*, so it isn't re-investigated.

---

## 4. Entry template

```markdown
# <Symptom as observed>

- Area: <caps | sync | mcp | jobs | ui | …>
- Status: open | investigating | resolved | wont-fix
- First seen: YYYY-MM-DD
- Resolved: YYYY-MM-DD
- Session: ../../sessions/<topic>/<name>-session.md
- Regression test: rust/<crate>/tests/<...> (or ui/...)

## Symptom
What is observed — error text, wrong behavior, the gap vs expected.

## Reproduce
Minimal exact steps + environment (role, profile, OS) to trigger it.

## Investigation
Timeline. What was checked, what was ruled out, the key observation that cracked it.

## Root cause
The actual cause, at the right layer. Link the offending code path:line.

## Fix
What changed and why this layer. Link the commit/files.

## Verification
How it was proven fixed (the repro now passing; command output).

## Prevention
The regression test added, and any guardrail (type/assert/capability) that makes the
class impossible — or a follow-up if none yet exists.
```

---

## 5. What each AI session must do

- Hit a non-trivial "it's not working"? **Open a debug entry** and keep it as you
  investigate (don't reconstruct from memory afterward).
- On resolution: complete the entry, add the regression test, update `debugging/README.md`,
  and cross-link the session doc.
- Leaving it unresolved? Update the entry with what's been ruled out before ending the
  session — an open entry with a trail is a gift to the next session.

---

## 6. Open questions

- Do we generate `debugging/README.md` from entry frontmatter, or maintain it by hand?
- Retention: do resolved entries ever get archived, or is the full history kept forever
  (preferred — it's the point)?
- Relationship to runtime observability/audit (`key-stack.md`): when does a debug entry
  reference an audit trace or job transcript as evidence?
