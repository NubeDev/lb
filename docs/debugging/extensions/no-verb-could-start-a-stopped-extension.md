# Nothing could start a stopped extension — every documented recovery pointed at a verb that didn't start anything

**Date:** 2026-07-15 · **Area:** extensions / lifecycle-management · **Status:** fixed
**Follows:** [#64](https://github.com/NubeDev/lb/issues/64)

## Symptom

An installed, enabled extension that was not running could not be started. Not "was awkward to
start" — there was **no verb for it**. Republishing the artifact (re-uploading a binary already on
disk) was the only way back, and that is *why* the #64 boot gap cost hours instead of minutes: the
workaround for a dead sidecar was a full re-publish.

## Root cause — three dead ends, two of them signposted

- **`ext.enable`** flips `Install.enabled` and spawns nothing. Its own doc said *"the boot reconciler
  / next start brings it up"* — a *start* that had no verb.
- **`native.restart`** needs a live handle in the `SidecarMap` → `NotRunning` when the child is gone.
- **`native.reset`** needs a present-but-dead handle → also `NotRunning`. And its doc said: *"If no
  handle exists at all (never started / already removed) it is `NotRunning` — use `ext.enable`/install
  to start a stopped extension."* **`ext.enable` does not do that.**

So the one verb an operator was explicitly sent to was the one that could not help, and the sentence
had been sitting in the source the whole time, read as if it were true.

## Fix

`ext.start` (`mcp:ext.start:call`, workspace-first) + `POST /extensions/{ext}/start`.

It reuses **boot's exact path** — the per-extension body of `spawn_enabled` factored out as
`spawn_one` — rather than a parallel implementation: "start this extension" means precisely what the
node does for it at boot, whoever asks, and the two cannot drift. No new persistence, no new trust:
the artifact is the same verified cached one, the grant comes from the durable `Install`.

`enable`/`start` stay **distinct** (the split `disable`/`stop` already has): `enable` is durable
*intent*, `start` is the *act*. A start **refuses a disabled extension** (`reason: "disabled"`) rather
than override the intent `disable` exists to express — it would resurrect exactly what disable
prevents, silently, until the next boot honored the flag and it vanished again.

It returns the outcome **row** in the boot log's own `spawned` + `reason` vocabulary, so an operator
reads one language in both places. Both false doc pointers (`reset`, `enable`) now name `ext.start`.

Not wired into the `ext.*` MCP bridge: `call_ext_tool` has no caller and `"ext."` is absent from
`HOST_NATIVE_PREFIXES`, so that whole surface is unreachable today. Adding `start` there would imply a
reachability that does not exist.

## Verified live, not just green

Against a real node + real modbus sidecar, the full sequence:

```
disable                → running=False health=disabled
start (while disabled) → {"spawned":false,"reason":"disabled"}     ← intent held, no resurrection
enable                 → running=False                             ← enable is intent; it spawns nothing
start                  → {"spawned":true,"reason":"spawned"}       ← the recovery that did not exist
start (again)          → {"spawned":false,"reason":"already-running"}
```

`running=True health=ok`, a real OS child, no node restart and no republish. The polling e2e then
passed (exit 0) against that route-started sidecar — it does real work, it is not merely alive.

## Regression tests

`ext_boot_spawn_test.rs`: `ext_start_brings_back_a_stopped_extension_and_it_answers` (the whole
sequence above, ending in a real tool call) and `ext_start_is_denied_without_the_grant_and_nothing_
spawns` (the mandatory capability-deny — spawning a process is exactly the authority a gate holds).

**Revert-checked:** with `ext_start` stubbed to the pre-fix world, the start test fails at the
"an enabled, stopped extension starts on demand" assert. The deny test correctly still passes — it
guards the gate, which is independent of the spawn.

## Lessons

- **A doc comment that names a recovery is a claim about another verb's behaviour, and nothing checks
  it.** `reset` told operators to use `ext.enable`; that sentence was wrong the day it was written and
  stayed wrong. If a doc points at a verb, the pointer deserves the same skepticism as a
  "handled elsewhere" comment (#64's `continue` named an owner that never existed — same class).
- **A missing verb hides behind a workaround.** Republishing "worked", so the absence never presented
  as a bug — it presented as an annoying but functioning procedure, which is why it survived so long.
  Ask what the recovery path costs, not just whether one exists.
