# CI stood red on master for weeks — four jobs, four different stale/starved causes

- Area: build / CI (`.github/workflows/ci.yml`)
- Status: resolved
- First seen: 2026-07-25 (standing red across at least runs `30148863660` → `30159587034`)
- Session: ../../sessions/build/ci-red-baseline-session.md
- Regression test: the CI workflow is its own check. The file-layout ratchet is
  revert-checked in the session doc (a new >400-line file and a +1 line on a
  grandfathered file each fail the script; both verified).

## Symptom

Every push to `master` failed **all four** CI jobs, so PR checks were uninformative:
a PR could not be distinguished from the baseline. Observed on PR #105, which was
merged anyway after confirming the same four jobs failed on master without it.

| Job | Surface error |
|---|---|
| `file-layout` | `file(s) exceed the 400-line FILE-LAYOUT limit` |
| `build-and-test` | `collect2: fatal error: ld terminated with signal 7 [Bus error], core dumped` |
| `ui` | `Some specified paths were not resolved, unable to cache dependencies` |
| `deploy-image` | `failed to compute cache key: "/ui": not found` |

## Root cause

Four independent causes. Only one is a code-quality backlog; the rest are stale config.

**1. `build-and-test` — the runner ran OUT OF DISK, not out of luck.** This one was
actively mis-diagnosed as a runner infra flake, because the fatal line *is* a
signal-7 bus error from `ld`. SIGBUS is what a process gets when a writable `mmap`
can no longer be backed by storage — i.e. it is the *downstream* symptom. The actual
cause sat one line earlier in the raw log and was invisible in the GitHub UI summary:

```
rustc-LLVM ERROR: IO failure on output stream: No space left on device
error: could not compile `lb-role-gateway` (test "login_hardening_test")
  No space left on device (os error 28)
```

`cargo test --workspace` links ~200 debug test binaries that each statically pull in
polars + surrealdb + rusqlite. With full debug artifacts and incremental output on a
runner with ~14 GB free, `target/` exhausts the disk partway through the link phase.

**2. `ui` — the job tested a tree that was deleted.** `ui/` was removed in commit
`678503f` (lb is a library; product shells are vendored out-of-tree — `MIGRATION.md`).
The job still ran `pnpm install` in `ui/` and keyed the setup-node cache on
`ui/pnpm-lock.yaml`. It failed at *setup-node*, before any install — meaning the
repo's frontend (`packages/*`, `app/sdk`) had **zero** CI coverage, not merely broken
coverage. `pnpm-workspace.yaml` also still listed a `'ui'` member glob.

**3. `deploy-image` — the same deletion.** `deploy/common/Dockerfile` had a
`spa-builder` stage doing `COPY ui /src/ui` and `pnpm --filter lazybones-ui run build`.
`COPY` of a missing context path fails the build immediately.

**4. `file-layout` — a real backlog, but a check that could not report.** 114 tracked
files exceed the 400-line FILE-LAYOUT limit (the truncated CI log made this look like
~20). A flat gate that is *always* red reports nothing: a genuinely new 900-line file
looked identical to the standing backlog. An always-red check is an off check.

## Fix

- **`build-and-test`:** added a `Free runner disk space` step (drops dotnet, Android
  SDK, GHC, CodeQL, boost, swift, powershell — ~25 GB) and set
  `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0` for
  the job. The `[profile.dev]` `line-tables-only` setting in `rust/Cargo.toml` is left
  alone — it is tuned for developer machines, where backtraces are read.
- **`ui` job → `packages` job:** installs from the ROOT lockfile, runs
  `pnpm -r --filter "./packages/*" run test` and type-checks `app/sdk`. `app/sdk`'s own
  suite is `test:gateway`, which spawns a real node (rule 9) — it needs the Rust build
  and does not belong in a node-only job. pnpm pinned **11**, not 9: the workspace file
  uses `allowBuilds`/`onlyBuiltDependencies` (pnpm 10+) and depends on pnpm 11's
  `minimumReleaseAge` guard.
- **`deploy-image`:** deleted the `spa-builder` stage. The image is now explicitly
  **headless** — the lb node + federation sidecar behind Caddy, with an empty
  `/usr/share/lazybones/web` a product host fills. Caddy keeps its narrow SPA `handle`
  so the contract is unchanged; with the dir empty it 404s and everything falls to the
  gateway.
- **`file-layout` → a ratchet.** The 114-file backlog is frozen in
  `rust/scripts/file-size-baseline.txt` (path + line count). The check now fails only on
  movement in the wrong direction: a file over the limit that is *not* in the baseline,
  or a baseline file that *grew*. Shrinking always passes; dropping under the limit
  prints a notice to re-run `check-file-size.sh --update`. `dist/` is now excluded — two
  committed rolled-up `.d.ts` build artifacts were being counted as source.

## Aftershock — the new `packages` job immediately caught a real one

The first PR run of the replacement job failed on `packages/minimal-shell`:
`Failed to resolve import "@nube/ext-ui-sdk"`. That package declares

```json
"@nube/ext-ui-sdk": "link:../../../lb-ext-ui-sdk"
```

— a filesystem link to a **sibling repo checkout** that exists only on a developer box
that cloned `NubeDev/lb-ext-ui-sdk` next to `lb/`. It is the only `link:`/`file:`
dependency in the workspace. This is precisely why the suite passed locally and failed
in CI: the local box has the sibling, a runner never will.

The job now excludes that one package. It is **not** covered — `MIGRATION.md` has lb
consuming that SDK at a published tag (`ui-v0.4.1`), and converting the live link to a
tag is a cross-repo release decision, not a CI change.

Worth noting on its own: this failure had been latent for as long as the `ui` job was
red. Restoring the gate surfaced it within minutes.

## Lesson

**A stack's fatal line is not always its causal line.** `ld` dying of SIGBUS is the
most confusing possible spelling of "the disk is full"; the one line that named the
real cause was several lines up and absent from the job summary. When a build tool
dies of a *signal* rather than an error, suspect the environment underneath it
(disk, memory, mmap) and go read the raw log, not the rendered summary.

**Deleting a tree is not done until CI stops referencing it.** `ui/` was deleted
deliberately and correctly, but three separate config surfaces still pointed at it
(the CI job, the Dockerfile, the pnpm workspace). Two of them failed *loudly* for
weeks and were read as noise. A deletion checklist has to sweep `.github/`, `deploy/`,
and workspace manifests — CLAUDE.md's "never recreate `ui/`" rule covers new code but
did not cover the config left behind.

**A permanently-red gate is worse than no gate**, because it costs the same and
carries no information. If a quality backlog can't be paid down now, ratchet it: freeze
the debt and fail only on regressions. The gate starts reporting again the same day —
and, as the `minimal-shell` aftershock shows, starts finding things immediately.

**"Green locally" proves nothing about CI when the difference between the two machines
is the thing under test.** A `link:` to a sibling checkout is invisible on the box that
has the sibling. Any dependency that resolves through the filesystem outside the repo is
a local-only dependency, whatever the lockfile says.
