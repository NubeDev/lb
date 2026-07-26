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

`cargo test --workspace` links **324 integration test binaries** (152 in `lb-host`, 49 in
`lb-role-gateway`, ~123 across the other 49 crates) and every one statically pulls in the
full polars + datafusion + surrealdb + typst graph. `target/` exhausts the disk partway
through the link phase.

**The scale was badly underestimated on the first attempt** — see "Second attempt" below.
The measured numbers, from the runner itself:

```
before freeing:  /dev/root  145G   58G used    88G avail
after  freeing:  /dev/root  145G   36G used   109G avail
```

109 GB free was still not enough. Freeing runner disk is necessary but **cannot** fix
this alone; the artifact set genuinely exceeds it.

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

- **`build-and-test`:** a `Free runner disk space` step (~22 GB) plus `debug=0`,
  `strip=symbols` and `CARGO_INCREMENTAL=0`, **and — the part that actually fixes it —
  the job is now SHARDED** into three matrix legs (`host`, `gateway`, `rest`) so no
  single target dir ever holds all 324 test binaries. `fail-fast: false`, so one shard
  failing never hides another's result. `rust/Cargo.toml`'s `[profile.dev]` is left alone
  — `line-tables-only` is tuned for developer machines, where backtraces get read.
  `cargo fmt` moved to its own `fmt` job: it costs seconds and should not ride on a
  25-minute job that may die before reaching it. A `Report disk + target size` step
  (`if: always()`) now leaves `df`/`du` numbers in every log, so the next person sizing
  this has data instead of a guess.
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

## Second attempt — the first disk fix was insufficient, and said so loudly

The first PR run still failed `build-and-test`, and harder: the disk hit zero while the
GitHub **runner process itself** was writing its own diagnostic log —

```
System.IO.IOException: No space left on device :
  '/home/runner/actions-runner/cached/2.336.0/_diag/Worker_...-utc.log'
```

That is why the job showed `Test` stuck in `in_progress` with `conclusion: failure`, and
why **no log blob was ever uploaded** (`gh ... /logs` → `BlobNotFound`). The `df` numbers
had to be read out of the *`deploy-image`* job, which ran the identical free-disk step and
survived.

The root-cause diagnosis (disk, not flake) was right; the **sizing was wrong**. The
initial estimate of "~14 GB free" came from an outdated runner spec and was never
measured — the runner actually offers 88 GB free, 109 GB after cleanup, and the build
overran even that. Counting the actual artifacts (`cargo metadata`: 324 integration test
targets) is what made the real scale visible. Hence sharding.

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

## What the restored gate found immediately (the point of the exercise)

With the shards actually running, `cargo test` reached code no CI run had reached in weeks.
Disk was no longer the failure — all three shards finished comfortably:

| shard | free after cleanup | free after tests | used |
|---|---|---|---|
| host    | 111 GB | 62 GB | ~49 GB |
| gateway | 111 GB | 79 GB | ~33 GB |
| rest    | 112 GB | 86 GB | ~27 GB |

Three **pre-existing** failures surfaced. None was caused by this change — the branch carried
no Rust source edits at all when they appeared.

1. **`apikey_routes_test` — 6 failures, `create failed: 400`.** FIXED here. The test's admin
   fixture hand-listed caps including `mcp:*.get:call`-style wildcards, but `b27c0bd1`
   ("updated admin caps") added `series.read` / `series.latest` / `series.find` / `query.run`
   to `APIKEY_READ_CAPS` — which **no `mcp:*.<verb>:call` wildcard covers** — and never touched
   the test. The admin therefore no longer held every cap it was granting, and the no-widening
   guard correctly refused every create with a `Widen` → 400. The guard was right; the fixture
   had drifted. Fixed by DERIVING the admin's caps from `lb_apikey::apikey_write_caps()` (a
   superset of the read bundle) instead of re-listing them, so the same drift cannot recur. The
   two escalation-deny tests still pass, which is what proves the fix did not simply paper over
   the guard.
2. **`result_cache_test::an_accepting_caller_never_waits_on_a_stricter_callers_refresh`** —
   the known pre-existing timing flake, already documented in
   `federation/direct-path-broke-result-cache-tests.md` as "flakes at base too under load …
   left untouched". It waited 2.09 s against a 350 ms bound on a 4-core runner, while the
   test's own comments are written for a 16-worker box. NOT fixed here: raising the bound could
   mask a genuine slot-rule-1 violation, and choosing between that and constraining CI test
   parallelism is a real call, not a CI-plumbing decision.
3. **`host_catalog_covers_dispatch_prefixes` — `host catalog has no entry for dispatched prefix
   forms.`** A genuine product gap: `forms.get/list/save/delete` are dispatched but absent from
   `HOST_TOOLS`, so the whole family is invisible in the console and in the agent's
   `tools.catalog`-derived menu — exactly the regression that drift test exists to catch. NOT
   fixed here, though the fix was written and verified green (4 catalog rows): adding them grows
   `system/catalog.rs` past its own ratchet baseline (1366 → 1388), and the honest resolution is
   the split that file needs, not the first bypass of a gate introduced in the same change.

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

**Getting the cause right is not the same as getting the SIZE right.** "The disk is
full" was correct on day one; the fix still failed because the estimate of *how* full
was borrowed from an outdated spec instead of measured. A quantitative failure needs a
measured number before a fix is designed — here, `cargo metadata` counting 324 test
targets, and `df` from a job that actually survived.

**"Green locally" proves nothing about CI when the difference between the two machines
is the thing under test.** A `link:` to a sibling checkout is invisible on the box that
has the sibling. Any dependency that resolves through the filesystem outside the repo is
a local-only dependency, whatever the lockfile says.
