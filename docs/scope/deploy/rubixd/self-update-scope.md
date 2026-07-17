# rubixd scope — self-update with guarded auto-rollback

Status: scope (the ask). Slice 10 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

rubixd can update every package on the box except the one that matters most: itself.
This slice makes rubixd a package *of its own fleet* — a signed `rubixd` artifact that
installs through the same verify wall and release-dir layout as everything else — and
adds the one piece the slice-4 transaction engine cannot provide: a gate that survives
the death of the process being gated. A failed self-update **auto-rolls-back** to the
previous rubixd, and the health gate is stricter than `/health`: **if the operator UI
does not come back, the update failed**, because on an edge box the UI *is* the
recovery surface.

**Owning repo**: `rubix-fleet` — a new tiny crate `crates/rubixd-guard/` plus
`self_update/` in `crates/rubixd/`. No lb/SDK surface changes.

## Goals

- **rubixd as a package**: the release pipeline publishes a signed `rubixd` artifact
  (kind `systemd`, the binary + unit template). Resolution/verification reuses slice 8's
  local-first flow and trust wall unchanged — a self-update artifact is a
  `VerifiedArtifact` or it is nothing.
- **Self release layout**: `<state>/self/releases/<version>/` + `current` symlink —
  the slice-3 layout, dogfooded on rubixd itself. `keep_n` retention applies; the
  running previous version is always kept (slice 5's `kept_previous` rule).
- **`rubixd-guard`** (new crate, target < ~1 kLOC, blocking std only — no tokio, no
  RocksDB): the external executor of the self-transaction. rubixd stages + verifies,
  writes a plain-file JSON journal (`self/tx.json`, write-before-effect), then hands
  off; guard runs detached (transient systemd unit) and performs
  `swap → restart rubixd.service → gate → commit | rollback`. Guard never parses
  packages and never touches the ledger — the journal file is the whole contract.
- **The gate** (K consecutive green probes over a grace window, slice-4 semantics):
  1. unit active and not restart-looping (`NRestarts` over the window);
  2. `GET /health` → 200 `ok|degraded`;
  3. **UI probe**: `GET /` → 200 **and** the page body carries the UI marker, and one
     hashed asset under `/ui/*` → 200. A rubixd that serves JSON but lost its UI
     (bad embed, wrong asset hash) is a **failed** update.
- **Auto-rollback**: gate fail → guard flips `current` back, restarts, re-gates the old
  version. Old-gates-green → `RolledBack` + the new version marked bad in the journal
  (rubixd imports it into slice-4 `bad_versions` on next boot). Old-also-fails →
  `Failed`, unit left running whatever answers, journal says so loudly — this is the
  rescue-desk / emergency-SSH entry condition ([`rescue-desk-scope.md`](rescue-desk-scope.md)).
- **Crash-loop coverage**: `WatchdogSec=` + `sd_notify` watchdog pings in rubixd, so a
  hung (not just dead) new binary is killed by systemd inside the gate window where
  guard sees it as restart-looping.
- Operator verbs, bearer-gated like every data route: `POST /api/self-update`
  `{version}` (202 + tx id — the response outlives the responder, so status is polled),
  `GET /api/self-update/status`, `POST /api/self-rollback`; CLI `rubixd self-update
  <version>`, `rubixd self-rollback`, `rubixd self-status`. Bad-marked versions are
  refused with the mark shown; `clear-bad` applies.
- Crash recovery: guard resumes from the journal exactly as slice-4 `recover.rs` does
  from the ledger — any state found mid-flight re-gates or restores, never half-known.

## Non-goals

- No auto-self-update from a channel in this slice — operator-triggered only; the
  slice-6 poller may opt in later once the guard has months of scar tissue.
- No updating of `rubixd-guard` by itself mid-transaction (guard is shipped inside the
  rubixd release dir; the *new* guard is used only for the *next* update).
- No Windows service path; no downgrade-below-oldest-kept.

## Intent / approach

The whole design is one admission: **a process cannot gate its own replacement**. So
the transaction engine is not reused as code — its *semantics* (write-before-effect,
staged → swapped → gating → terminal) are re-implemented ~300 lines small in a binary
with nothing to break: no async runtime, no db engine, no config parser. Everything
that can fail interestingly (resolve, verify, stage) happens in the old, known-good
rubixd *before* handoff; guard only swaps a symlink, restarts a unit, and probes HTTP.
Alternatives rejected: **self-exec/re-spawn in place** (systemd owns the process tree;
a re-exec'd child dies with the cgroup and gates nothing), **A/B twin units on
alternate ports** (port juggling the packages don't model — same reason slice 4
rejected blue/green), and **delegating to apt/rpm** (loses the trust wall, the ledger
history, and the armv7 boxes with no package feed).

## How it fits the core

Durability: journal-before-effect is the outbox discipline in a flat file, chosen over
the ledger because the ledger's store dies with the process being replaced. The deny
path: `POST /api/self-update` without the token 401s; an unsigned/tampered/untrusted
artifact never stages (slice 8's type wall); a bad-marked version is refused typed.

## Example flow

1. `rubixd publish rubixd-0.6.0.tar.zst` lands the signed artifact in the local index.
2. `rubixd self-update 0.6.0` → resolve, verify, stage `self/releases/0.6.0/`, journal
   `Staged`, spawn guard, return tx id.
3. Guard: journal `Swapped`, flip symlink, restart unit; new rubixd boots, UI marker
   probe 200 ×K, `/health` ok → journal `Committed`; old release retained per keep-N.
4. Same flow with a 0.6.1 whose embedded UI is broken: `/health` is green but `GET /`
   probe fails → guard flips back to 0.6.0, old gates green → `RolledBack`,
   `0.6.1 ∈ bad_versions[self]`; `rubixd self-status` shows it red with the reason
   `ui-probe-failed`.

## Testing plan

Real `systemd --user`, real binaries (the purpose-built test server pattern from
slice 4, plus real rubixd builds), no mocks:

- happy path: old rubixd → self-update → new version active, `Committed`, UI probe
  passed, token/ledger/instances survive.
- gate-fail on `/health` and separately on the **UI probe** (a build with assets
  stripped) → auto-rollback, old active, bad mark recorded and refused on retry.
- rollback-also-fails → `Failed` journaled, both attempts recorded.
- kill guard mid-`Swapped`; re-run → resumes to a terminal state (journal recovery).
- watchdog: a new binary that hangs after bind → killed, seen as restart-loop, rolled
  back.
- deny: self-update without bearer 401; tampered artifact refused with nothing staged.

## Risks & hard problems

- Guard runs detached from the thing that spawned it — its lifecycle must come from
  systemd (transient unit + `RemainAfterExit` off), not a double-fork, or a reboot
  mid-gate strands the journal. Boot-time journal recovery is the non-negotiable
  review item.
- The UI marker must be cheap but honest — a static string in `index.html` plus one
  real hashed-asset fetch; probing "some 200" would pass a blank error page.
- Ledger schema drift across rubixd versions: the new rubixd must open the old ledger
  or fail *before* the gate window ends (fail-fast on open → rollback catches it).

## Open questions

- Should `self-rollback` be allowed while an instance transaction is running?
  Recommendation: no — the slice-4 global transaction lock covers self-updates too;
  serial keeps the failure story tellable.
- Grace/K defaults for self (boot is heavier than a package restart): recommend
  grace 10 s, K=3, timeout 60 s, tunable in config.

## Related

[`rollback-health-scope.md`](rollback-health-scope.md) (the semantics being
re-implemented externally) · [`local-publish-scope.md`](local-publish-scope.md) (the
trust wall the artifact enters through) · [`rescue-desk-scope.md`](rescue-desk-scope.md)
(what an operator reaches for when `Failed` happens anyway) ·
[`embedded-ui-scope.md`](embedded-ui-scope.md) (the UI whose availability is part of
the gate).
