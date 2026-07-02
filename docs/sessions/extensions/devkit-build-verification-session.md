# Session — Devkit build verification (artifact size + freshness)

**Branch:** ce-v3 · **Date:** 2026-07-02

## The ask

In Extension Studio's Build step, give the user hard proof a build actually produced
fresh output — show each artifact's **before → after size**, the byte delta, and whether
this build actually rewrote it — instead of the bare `built` boolean, which a stale
artifact satisfies forever. The user pointed at `host_tools/fs` as the size source.

## What we found

- `host.fs.stat` (`crates/host/src/host_tools/fs/stat.rs`) already reports `size` +
  `mtime` (RFC3339, seconds). No new host tool needed.
- The old `built` check (`crates/devkit/src/inspect.rs`) only tested that a
  `target/…/release` **dir** exists — `cargo` creates that even for a failed build, and it
  never inspects the artifact file. So "built" carried no proof of a fresh build.
- `InspectReport` was the natural carrier: the wizard already re-inspects after a build
  (`ui/.../studio.wizard.ts`).

## What shipped

**Rust (`lb-devkit`)**

- New `Artifact { kind, path, size, mtime }` on `InspectReport` (`model.rs`); `kind` is
  `native-bin | wasm | remote-entry`.
- New `crates/devkit/src/artifacts.rs` — `collect_artifacts(path, tier)` enumerates the
  release dir for the real binary/component (not a guessed `{crate}.wasm`) plus
  `ui/dist/remoteEntry.js`, stat'ing each. mtime uses the *same* RFC3339-seconds format as
  `host.fs.stat`, so snapshots compare directly.
- `built` is now derived from a real compiled binary/component being present — a UI remote
  alone does **not** count (the extension can't load without its binary).
- Added `chrono` (workspace dep) to `lb-devkit`.

**UI**

- `lib/devkit/artifact-diff.ts` — pure `diffArtifacts(before, after)` →
  `new | rebuilt | unchanged | missing` per artifact, with `bytesDelta`; `buildConfirmed()`
  is true only when every binary was freshly written (mtime advanced). `formatBytes` /
  `formatDelta` helpers.
- `studio.wizard.ts` — snapshots `inspect.artifacts` before the build, diffs against the
  post-build inspect, exposes `artifactDeltas`.
- `features/studio/ArtifactDiff.tsx` — the proof panel: `old → new (Δ)` per artifact + a
  green "Build confirmed" / amber "unconfirmed (unchanged or missing)" banner. Rendered in
  `BuildStep` above the log.

## Tests (green)

- `crates/devkit/tests/inspect_artifacts_test.rs` (real files on disk, no mocks):
  - `reports_wasm_and_remote_entry_artifacts_with_sizes` — seeds a real 2048-byte wasm +
    a remoteEntry.js, asserts sizes + mtime and `built`.
  - `ui_remote_alone_is_not_built` — a remote bundle with no binary must read `built: false`.
- `ui/src/lib/devkit/artifact-diff.test.ts` (9 cases) — new/rebuilt/unchanged/missing,
  `buildConfirmed` (binary-unchanged-but-UI-rebuilt ⇒ false), and the formatters.
- `cargo build -p lb-devkit -p lb-host`, `cargo fmt -p lb-devkit --check`, UI `tsc --noEmit`
  all clean.

Capability-deny + workspace-isolation for `devkit.inspect` are unchanged and still covered
by the pre-existing `crates/host/tests/devkit_test.rs` gates — this change only enriches the
report returned *after* those gates pass.

## Follow-on: loud verdict + recent-builds MRU

- **Loud verdict panel** — `ArtifactDiff.tsx` is now a full-width green **BUILD VERIFIED** /
  red **BUILD NOT VERIFIED** banner with a specific reason line, shown after *every* build.
  `studio.wizard.ts build()` re-inspects on failure too (it used to jump to the error handler
  and render nothing), so the panel is guaranteed on pass or fail.
- **Recent builds (localStorage)** — `lib/devkit/recent-extensions.ts` keeps a 5-deep MRU of
  `{path,id,tier,at}` under `lb.studio.recent-extensions`. Recorded on every successful
  open/build; corrupt/quota failures degrade to empty (never throws). `RecentBuilds.tsx`
  renders a one-click list at the top of the Create step; `openRecent(path)` inspects then
  builds in one shot (a since-deleted path fails loudly and is auto-forgotten). Covered by
  `recent-extensions.test.ts` (5 cases: MRU, cap-at-5, dedupe, forget, corrupt-store).

## Follow-ups

- `ArtifactDiff.tsx` visual rendering isn't covered by the gateway test yet (the
  `StudioView.gateway.test.tsx` path drives a real node but doesn't assert the diff panel).
- No size-budget threshold yet (e.g. warn if remoteEntry.js exceeds N MB) — deltas are shown
  but not policed.
