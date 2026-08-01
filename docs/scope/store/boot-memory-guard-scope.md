# Store scope — a boot memory guard (open without OOMing the box)

Status: scope (the ask), issue [#128](https://github.com/NubeDev/lb/issues/128). Promotes to
`doc-site/content/public/store/` once shipped. This is the **memory** half of the
store-footprint problem; [#122](https://github.com/NubeDev/lb/issues/122)
(`disk-budget-scope.md`, shipped) is the **disk** half and would not have prevented this
incident — the live set was already 2.4× `LOG_ADVISORY_BYTES` and boot still ran the pass.

`Store::open` (`crates/store/src/open.rs:176`) runs a full compaction pass unconditionally,
then the plain SurrealDB open replays the log again — two to three full replays plus a
merge that resolves **every live value from disk into memory** — before the gateway binds.
Nothing on that path knows how much RAM the machine has. On a Rubix Compute (armv7,
959 MB) with a 617 MB live set, boot climbed to 879 MB anon-RSS, the kernel's **global**
OOM-killer took `sshd` down with the node, `Restart=on-failure` re-ran the same replay
every 5 s, and the box was bricked until someone drove to it (2026-08-01, twice). This
scope makes boot **memory-aware**: the compaction pass runs only when the machine can
afford it and it is expected to pay; a store the machine provably cannot open is **refused
with a diagnostic** instead of OOMed into a restart loop; and every skip/refusal is loud
and observable. A node that cannot open its store needs an operator — never the OOM-killer.

## Goals

- **Boot compaction becomes conditional.** The pass runs only when (a) the machine has the
  memory headroom for it and (b) it is expected to reclaim anything. Skipping is loud
  (log_bytes, available RAM, last-pass reclaim ratio in the line) — never silent.
- **A hopeless open is refused, not attempted.** When the log is large relative to
  available RAM, `Store::open` returns an error naming both numbers and what to do about
  it, and the node exits cleanly. The failure mode changes from "kernel picks a victim
  machine-wide, sshd dies, box dark" to "one service down, `journalctl` says exactly why."
- **The guard fails open and is overridable.** No `/proc/meminfo` (non-Linux, containers
  with odd mounts) ⇒ no guard, today's behaviour. An operator who knows better (added
  swap, measured headroom) can force the open. A heuristic that can hard-brick a valid
  boot is worse than the bug.
- **The boot-pass outcome survives restart and is visible.** The `CompactionRecord` is
  persisted beside the log and served by `store.status`, including "skipped: <reason>" —
  so an unproductive-pass suspension survives the restart that is precisely when it
  matters, and a fleet operator can see a node that has stopped compacting.
- **Symmetric.** The guard is config + measurement, never a role branch. A cloud node
  with 64 GB and an edge node with 959 MB run the identical code; the numbers differ.

## Non-goals

- **Bounding steady-state RSS.** This scope bounds the *boot path* — the one place the
  node predictably doubles its memory demand. Runtime memory (ART index over the live
  set, caches) is real but is bounded by bounding the live set (retention, `#122`
  slice 3, shipped) and by the supervisor's `MemoryMax`, not here.
- **Changing the engine or the replay mechanism.** SurrealKV replays the whole log to
  open — that is the engine's contract (rule 2, `persistent-backend-scope.md`). A
  paged/streaming index is engine work upstream, not ours.
- **The rubixd unit generator.** `MemoryMax`/`OOMPolicy=stop`/`StartLimitBurst` in
  generated units is what turns "one service OOMs" into "sshd survives" for every
  workload rubixd manages. Tracked in the rubixd repo (issue #128 fix 4). This scope's
  refusal makes the node well-behaved even under a naked unit, but both belong deployed.
- **Downstream deployment defaults.** `rubix-ai`'s packaged env (`build/armhf/systemd/`)
  should ship `LB_STORE_MAX_BYTES` sized to the box and the unit should carry the memory
  stanza — named in "Example flow" so the product session picks it up, built there.
- **Auto-deriving a disk budget from RAM or disk size.** Rejected in `#122` decision 2
  and stays rejected. The guard below is not a budget: it never evicts, pauses, or
  bounds — it converts one guaranteed-fatal boot into a diagnostic.

## Intent / approach

Three slices, independently shippable, value front-loaded. All three are decision logic
around the existing pass — no new mechanism, no new verbs, no schema.

**Slice 1 — precondition the boot compaction pass** (`open.rs:176`, the highest-leverage
line in the incident). Today `compact_log` skips only when the directory does not exist
(`compact.rs:93`). Add two preconditions, both pure functions over numbers so they test
without seeding a gigabyte:

1. *Memory headroom:* skip when `log_bytes > BOOT_COMPACT_MEM_RATIO × available_ram`.
   The pass's peak is dominated by surrealkv's merge resolving every live value into its
   entries buffer on top of a full index build — on the incident box a 617 MB log drove
   879 MB RSS, so the pass costs on the order of the log again. Start
   `BOOT_COMPACT_MEM_RATIO = 0.5` (a named const with a test, like
   `PRODUCTIVE_RECLAIM_RATIO`): a pass over a log bigger than half of free RAM is a
   gamble the boot path must not take, because losing it takes the whole box.
2. *Expected benefit:* skip when the **persisted** last pass (slice 3) was unproductive —
   `after_bytes > PRODUCTIVE_RECLAIM_RATIO × before_bytes`, the exact judgement
   `store_admin/budget.rs:34` already makes at runtime — and the log has not grown
   materially since. The incident's 617 MB segment was compaction *output*: the live set
   itself. Re-compacting it every boot is the most expensive possible no-op, and it was
   guaranteed by construction, not bad luck.

`available_ram` is `MemAvailable` from `/proc/meminfo` (Linux; the same
read-a-proc-file posture as `wait_for_quiesce`'s fd probe and `free_disk_bytes`'s
statvfs — no `sysinfo` crate). Unreadable ⇒ `None` ⇒ precondition 1 passes (fail open).
A skip logs at **warn** with all three numbers and returns a `CompactionRecord` with
`ok: false` and a machine-readable skip reason — never the silent `.ok()` discard.

**Slice 2 — the open guard: refuse instead of OOM.** After the (possibly skipped) pass,
the plain `Surreal::new::<SurrealKv>` replay still builds the full live-set index in RAM —
skipping compaction lowers the peak but does not cap it, and on the incident box the
index alone was fatal. So before opening: if `log_bytes > OPEN_GUARD_MEM_RATIO ×
available_ram` (start at `1.0` — deliberately looser than the compaction ratio, because
refusing is a much bigger call than skipping and a plain replay peaks well below a merge),
return a new `StoreError::WontFit { log_bytes, available_ram }` whose message names both
numbers, the override, and the remedies (add RAM/swap; compact on a bigger machine; lower
retention and let the next compaction shrink the live set). The binary exits nonzero with
that on stderr. Under the naked rubixd unit that still restart-loops — but each attempt
is milliseconds of a stat, not 90 s of allocation, so **sshd survives and the box stays
reachable**, which is the difference between a remote fix and a site visit.

The override is `BootConfig::store_open_unguarded: bool` from `LB_STORE_OPEN_UNGUARDED=1`,
parsed in `node/src/config.rs::from_env` (the one place `LB_*` is read), threaded as a
parameter of `Store::open` — the store crate reads no env (binary-boundary rule). The
guard also fails open when `available_ram` is `None`.

**Slice 3 — persist the pass record; surface skips.** `last_compaction` is in-memory only
today (`open.rs:74` — "a restart re-seeds it from the boot pass"), which is exactly
backwards once boot may *skip*: the skip needs the previous pass's outcome, and after a
skip there is no fresh record to serve. Persist the `CompactionRecord` (already
serde-serializable) as JSON to `<store>/../last-compaction.json` — beside, not inside,
the engine's directory, so `log_stats` and the `#122` budget arithmetic never count it
and the engine never sees a foreign file. This is store-infrastructure state at the same
level as the `.merge/` marker the pass already manages on disk — not application state,
so the one-datastore rule is not in play. Best-effort both ways: unreadable/corrupt ⇒
`None` (precondition 2 passes, compact as today), unwritable ⇒ warn and continue.
`store.status` serves the record with its new `skipped: Option<String>` field, so "this
node has stopped compacting at boot and why" is one MCP call away, and the `#122` budget
driver's unproductive-suspension can re-seed from it instead of resetting every restart.

**Alternative rejected — open degraded (read-only) instead of refusing.** There is no
such mode: SurrealKV must replay the full log to serve a single read. The index *is* the
open. Refusal with a diagnostic is the only honest option short of engine work.

**Alternative rejected — let the supervisor handle it entirely (`MemoryMax` +
`OOMPolicy=stop`).** Necessary defense in depth (and named for rubixd/rubix-ai above),
but insufficient alone: a cgroup kill during the *merge-apply* window still risks the
interrupted-merge path, the node learns nothing (next boot repeats the attempt), and the
diagnostic is a kernel line, not "your store needs 617 MB and you have 802 MB free."
The node refusing cleanly is what makes the failure legible; the unit stanza is what
protects the box from everything else.

## How it fits the core

- **Tenancy / isolation:** below the namespace wall — the guard stats files and reads
  `/proc/meminfo`; it never reads a record as any principal. Isolation suites pass
  unmodified.
- **Capabilities:** no new verbs, no new caps. `store.status` stays `store:status:read`;
  the record gains fields. The deny path is unchanged and re-asserted.
- **Placement:** either (rule 1). The guard is arithmetic over this machine's numbers;
  a 64 GB cloud node never trips it, a 959 MB edge node is protected by it — same code.
- **MCP surface (§6.1):** *get* — `store.status`'s existing `last_compaction` gains
  `skipped`; no new tool. *CRUD / live feed / batch* — N/A: boot config and a boot-time
  decision, not a record; an operator who could disable the guard via MCP could do so on
  a box that is about to become unreachable, which is the wrong door for it.
- **Data (SurrealDB):** no schema change, no new table. The persisted record is a
  sub-KB JSON sidecar *outside* the engine dir — store infrastructure (like `.merge/`),
  not application state; it exists precisely for the moment the datastore cannot open.
- **Bus (Zenoh):** none. Skips/refusals are tracing + the status verb.
- **Sync / authority:** node-local by definition — RAM belongs to one machine.
- **Secrets:** N/A.
- **SDK/WIT impact:** none. `BootConfig` is `#[non_exhaustive]`; `StoreError` gains a
  variant (source-compatible for embedders matching non-exhaustively; flag in release
  notes for any exhaustive match).
- **Skill doc:** extend `skills/store-compact/SKILL.md` with the boot-guard flow (read
  the persisted record → interpret a skip → recover a refused open, including the
  override) grounded in a live run. Same drivable surface, no new skill.

## Example flow

The incident, replayed with this scope shipped:

1. A Rubix Compute (959 MB RAM) has a 617 MB store whose live set ≈ the log (a previous
   compaction's output). rubixd restarts `rubix-ai`.
2. Boot: `log_bytes = 617 MB`, `MemAvailable = 802 MB`. Precondition 1 fails
   (617 > 0.5 × 802) — and the persisted record shows the last pass reclaimed 0.3%
   anyway, so precondition 2 fails too. One warn line with all three numbers; no pass.
3. The open guard: 617 < 1.0 × 802 — the plain replay is allowed. The node opens in
   ~10 s at a fraction of the compaction peak, binds `:8099`, and serves. Degraded boot
   speed, zero drama. **This is the expected outcome on the incident box** — the guard
   refuses only the genuinely hopeless case.
4. `store.status` shows `last_compaction.skipped: "log 617MB > 50% of available 802MB"`.
   The operator (or fleet tooling) sees a node that has stopped compacting and either
   grants RAM, tightens retention, or accepts it.
5. Had the store been 900 MB: the guard refuses, the binary exits nonzero in
   milliseconds with both numbers and the remedies on stderr, sshd never contends —
   the operator ssh'es in, sets `LB_STORE_OPEN_UNGUARDED=1` after adding swap, or
   tightens retention and compacts from a workstation. No site visit.
6. Downstream (product repos, not this one): `rubix-ai`'s `build/armhf` env sample gains
   `LB_STORE_MAX_BYTES=268435456` (256 MB — sized so the disk half acts long before the
   memory half can matter on a 1 GB box) and its unit gains
   `MemoryMax=700M` / `OOMPolicy=stop` / `RestartSec=30` / `StartLimitIntervalSec=300` /
   `StartLimitBurst=3`; the rubixd generator gets the same stanza (its own issue).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`):
- **Workspace isolation:** suites pass unmodified — the guard is below the wall.
- **Capability deny:** re-assert `store.status` without `store:status:read` ⇒ denied,
  with the new fields present for a granted caller.
- **Hot-reload / offline:** N/A (boot path only) — stated, not skipped silently.

Key cases (real store, real files; the decisions are pure functions over numbers, so the
gigabyte-scale judgements are tested by *injecting the numbers*, not by seeding 617 MB —
injecting an integer into a pure function is not a mock, rule 9 intact):
- *Precondition arithmetic:* the skip decision over (log_bytes, available_ram,
  last_record) — headroom skip, unproductive skip, `None` RAM ⇒ run, grown-log-since
  ⇒ run. Table-driven, like `BudgetDriver`'s tests.
- *Skip is loud and recorded:* a real (small) store opened with an injected tiny
  `available_ram` skips the pass, opens fine, logs warn, and `store.status` serves
  `skipped` with the reason. **Fails with the change reverted** (today: silent pass).
- *Guard refuses:* injected `available_ram` below the log size ⇒ `Store::open` returns
  `WontFit` naming both numbers; nothing was opened; the directory is untouched
  (re-open with the override succeeds — same test).
- *Override:* `LB_STORE_OPEN_UNGUARDED=1` parses at the binary boundary (malformed ⇒
  warn + guard stays on, never panic — the `LB_STORE_MAX_BYTES` pattern) and forces the
  open past a failing guard.
- *Persisted record roundtrip:* write, restart (drop + reopen), the record is served;
  corrupt/missing file ⇒ `None`, pass runs, warn logged.
- *Sidecar is outside the budget:* `log_stats` over a store with the JSON present
  returns byte-identical numbers — the `#122` arithmetic never counts it.
- *Merge-completion rule survives:* a skipped pass with a pending `.merge/` still
  completes the merge first (the P0 in `compact.rs` — skipping compaction must never
  mean skipping merge completion, or the next writing open eats a session's writes).
- *Real-scale measurement (session doc, not CI):* on one large store (GB), record peak
  RSS for pass-vs-skip and open-vs-refuse thresholds; write the numbers into the session
  doc the way `#122` gated its slice 2. The ratios ship as named consts either way; the
  measurement tunes them.

## Risks & hard problems

- **The guard can refuse a boot that would have succeeded.** The inverse of the bug, and
  the reason `OPEN_GUARD_MEM_RATIO` starts loose (1.0), fails open without `/proc`, and
  has an env override. A refused open is recoverable in one ssh session; an OOM loop
  cost two site visits — the asymmetry justifies a conservative guard, but the ratio is
  a judgement, not a measurement, until the session-doc numbers land. Say so in the log
  line ("heuristic; override with …").
- **`MemAvailable` is a moving target.** Another service allocating during boot changes
  the answer between the stat and the replay. The guard is a tripwire, not a
  reservation; `MemoryMax` on the unit is the enforcement layer. Both are named
  deliverables; neither substitutes for the other.
- **Skipping compaction forever on a RAM-bound box.** Preconditions 1+2 can
  permanently park a node whose log *would* reclaim (e.g. bloat accumulated after RAM
  shrank). The unproductive skip requires "not grown materially since"; the headroom
  skip logs every boot and shows in `store.status` — parked is visible, and the `#122`
  runtime driver (which the boot skip does not gate) still compacts online once the
  node is up, where a failed pass costs a skipped job, not the box.
- **A new `StoreError` variant.** Embedders with exhaustive matches break at compile
  time; `#[non_exhaustive]` on the enum (if not already) or a loud release note. Cheap,
  but it is the one API-surface ripple.
- **The sidecar file is novel surface.** One more thing that can be stale, wrong, or
  half-written. Mitigated by best-effort semantics on both ends (any failure degrades
  to today's behaviour exactly) and atomic write (tmp + rename).

## Decisions (no open questions)

Every question this scope raised is answered here. Build to these; do not re-litigate
them mid-implementation. If one proves wrong in the build, change it deliberately, pick
the long-term-best resolution, and record why in the session doc — start from a decision,
not a question.

1. **The ratios ship as `BOOT_COMPACT_MEM_RATIO = 0.5` and `OPEN_GUARD_MEM_RATIO = 1.0`**
   — named consts, each pinned by a table-driven test, tunable later as a one-line change
   (the `PRODUCTIVE_RECLAIM_RATIO` pattern). They are engineering judgement from the
   incident's numbers (617 MB log → 879 MB pass RSS on a 959 MB box); they only have to
   separate "gamble the box" from "slow boot", and these do. The real-scale RSS
   measurement in the testing plan tunes them and is recorded in the session doc — it
   gates nothing.
2. **"Grown materially since" is `log_bytes > 1.25 × last.after_bytes`** — const
   `REGROWTH_RERUN_RATIO = 1.25`, pinned by a test. A quarter of fresh bloat is enough
   reclaimable material to justify re-trying a pass that last reclaimed nothing; below
   it, the skip stands.
3. **A refused open exits the binary, nonzero, with the diagnostic on stderr.**
   `Store::open` returning `WontFit` is the mechanism; `lb-node`'s boot propagates it as
   a fatal error and never falls back to `mem://` — a silently-empty node serving a
   workspace that "lost" its data is strictly worse than a down node with a legible
   reason. An embedder that wants different policy matches the variant; the default is
   exit.
4. **The persisted record lives at `<store dir>/../last-compaction.json`** — sibling of
   the engine directory, atomic write (tmp + rename), best-effort in both directions
   (unreadable ⇒ `None` ⇒ compact as today; unwritable ⇒ warn and continue). Never
   inside the engine dir: `log_stats` and the #122 budget arithmetic must not count it
   and the engine must never see a foreign file.
5. **The memory probe is `MemAvailable` from `/proc/meminfo`, read directly** — no
   `sysinfo`/`libc` dependency (the store crate's existing posture: the fd probe and
   statvfs precedent). Unreadable or absent ⇒ `None` ⇒ both guards fail open.
6. **The override is `LB_STORE_OPEN_UNGUARDED` (exact value `1`)**, parsed in
   `node/src/config.rs::from_env` into `BootConfig` and threaded to `Store::open` as a
   parameter — the store crate reads no env. Any other value ⇒ warn, guard stays on,
   never panic (the `LB_STORE_MAX_BYTES` pattern). It disables the *open* guard only;
   the compaction-skip preconditions are not overridable (skipping a pass is always
   safe; there is nothing to force).
7. **A skipped pass still completes a pending `.merge/` first.** The skip preconditions
   sit after the merge-completion step, never before it — the P0 ordering rule in
   `compact.rs` is not negotiable, and a sub-KB merge apply is not the memory hazard the
   pass is.

## Related

- Issue [#128](https://github.com/NubeDev/lb/issues/128) — the incident + first-cut
  fixes this scope firms up; [#122](https://github.com/NubeDev/lb/issues/122) — the disk
  half.
- `scope/store/disk-budget-scope.md` — bounds bytes on disk; explicitly never RSS. Its
  `PRODUCTIVE_RECLAIM_RATIO` (`host/src/store_admin/budget.rs:34`) is reused verbatim by
  precondition 2, and its budget driver re-seeds from slice 3's persisted record.
- `scope/store/online-compaction-scope.md` — the pass itself and the merge-completion
  P0 that every skip path must preserve.
- `scope/store/persistent-backend-scope.md` — why replay-everything is the engine's
  contract (and so why the only options are *don't run it* or *don't open*).
- `crates/store/src/open.rs:176` — the unconditional pass; `crates/store/src/compact.rs`
  — `compact_log` + the merge rule; `node/src/config.rs` — the binary boundary for the
  new env.
- `skills/store-compact/SKILL.md` — gains the boot-guard flow on ship.
- `debugging/store/` — the incident's debugging entry lands here during implementation.
