# Session — CI stood red on master; restore the signal

- Date: 2026-07-26
- Area: build / CI (`.github/workflows/ci.yml`, `deploy/common/`, `rust/scripts/`)
- Outcome: all four jobs fixed at the cause; file-layout converted to a ratchet with the
  114-file backlog frozen. Debugging entry:
  [`../../debugging/build/ci-red-baseline-disk-exhaustion-and-deleted-ui.md`](../../debugging/build/ci-red-baseline-disk-exhaustion-and-deleted-ui.md).

## The ask

PR #105 showed all four checks failing. Master's latest run failed the same four jobs,
as did the two before it — so the red was **baseline, not the PR**. #105 was merged on
that basis. The ask: fix the baseline, because "that red baseline means CI can't tell
you when something real breaks."

## What we found

Four independent causes — three of them stale config, one a genuine backlog.

**`build-and-test` was NOT an infra flake.** This was the important correction. The
reported symptom, `collect2: fatal error: ld terminated with signal 7 [Bus error]`,
reads like a runner hiccup and there were no compile errors in the summary. The raw log
carried the real cause one line earlier:

```
rustc-LLVM ERROR: IO failure on output stream: No space left on device
error: could not compile `lb-role-gateway` (test "login_hardening_test")
  No space left on device (os error 28)
```

The runner ran out of disk linking ~200 debug test binaries that each statically pull in
polars + surrealdb + rusqlite. SIGBUS is the downstream symptom of an `mmap` that can no
longer be backed. Deterministic, not flaky — it would never have "gone away on a re-run".

**`ui` and `deploy-image` both died on the deleted `ui/` tree** (commit `678503f`). The
`ui` job failed at *setup-node*, before installing anything, on a cache key pointing at
`ui/pnpm-lock.yaml` — so `packages/*` and `app/sdk` had **zero** CI coverage, not broken
coverage. The Dockerfile's `spa-builder` stage still did `COPY ui /src/ui`.
`pnpm-workspace.yaml` still listed a `'ui'` member glob.

**`file-layout` had 114 offenders, not ~20.** The count in the original report came from
a truncated CI log. Two of the 114 were committed `dist/*.d.ts` build artifacts that
should never have been scanned.

## What we did

1. **`build-and-test`** — added a `Free runner disk space` step (~25 GB: dotnet, Android
   SDK, GHC, CodeQL, boost, swift, powershell) and set `CARGO_PROFILE_DEV_DEBUG=0`,
   `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0` on the job. `rust/Cargo.toml`'s
   `[profile.dev]` is untouched — `line-tables-only` is tuned for dev machines where
   backtraces get read; CI never reads them.
2. **`ui` job → `packages` job** — installs from the ROOT lockfile, runs
   `pnpm -r --filter "./packages/*" run test`, type-checks `app/sdk`. pnpm pinned **11**
   (was 9): `pnpm-workspace.yaml` uses `allowBuilds`/`onlyBuiltDependencies` (pnpm 10+)
   and leans on pnpm 11's `minimumReleaseAge` guard. `app/sdk`'s `test:gateway` spawns a
   real node (rule 9) and deliberately stays out of a node-only job.
3. **`deploy-image`** — deleted the `spa-builder` stage; the image is now explicitly
   **headless** (node + federation behind Caddy, empty `/usr/share/lazybones/web` for a
   product host to fill). Caddy's narrow SPA `handle` stays, so the routing contract is
   unchanged. Also added `**/.cargo/config.toml` to `.dockerignore` — see below.
4. **`file-layout` → ratchet** — backlog frozen in `rust/scripts/file-size-baseline.txt`
   (114 entries, path + line count). Fails only on a *new* violation or a baseline file
   that *grew*; shrinking passes and prints a notice to re-run with `--update`. `dist/`
   excluded from the scan entirely.
5. Dropped the stale `'ui'` glob from `pnpm-workspace.yaml` and the `ui/` entries from
   `.dockerignore`.

## Verification

- **file-layout ratchet, revert-checked both ways** (a test that only passes proves
  nothing — `verify-in-product-not-suite`):
  - clean tree → `FILE-LAYOUT: OK — 2390 files checked, 114 grandfathered`, exit 0;
  - planted a 500-line `rust/crates/host/src/zz_ratchet_probe.rs` → `new violation`, exit 1;
  - appended ONE line to a grandfathered file (`routes/flows.rs`, baseline 433) →
    `grew to 434 lines (baseline 433) — split it, don't extend it`, exit 1;
  - both probes reverted, tree clean, exit 0 again.
- **packages job** — `pnpm -r --filter "./packages/*" run test` green locally
  (ce-wiresheet 153, source-picker 48, panel 7, nav-rail, dashboard, genui, insights,
  minimal-shell); `cd app/sdk && pnpm typecheck` exit 0.
- **pnpm install --frozen-lockfile** still exit 0 after the workspace edit (the lockfile
  had no `ui` importer — it was regenerated after the deletion).
- **deploy-image** — `docker build --check` clean; full image build run locally.

### A trap worth recording

The first local `docker build … | tail -60` reported **exit 0 while the build had
failed** — the pipe's status, not docker's. (Same class as the known
`| tail` masks cargo exit note.) Re-run writing to a log and reading `$?` directly.
The failure it had been hiding was `linker /home/user/.local/bin/zigcc not found`:
the host-only, gitignored `rust/.cargo/config.toml` was being copied into the build
context by `COPY rust`, because **`.dockerignore` is not `.gitignore`**. CI's fresh
checkout has no such file, so this never affected CI — but it meant a local build could
not reproduce CI at all. Added `**/.cargo/config.toml` to `.dockerignore`.

## Follow-ups (not done here)

- **Pay down the 114-file backlog.** The ratchet stops it growing; it does not split
  anything. Highest-value targets are the source (not test) files:
  `host/src/system/catalog.rs` (1366), `host/src/tool_call.rs` (1159),
  `host/src/authz/builtin_roles.rs` (1055), `ext-loader/src/manifest.rs` (1017),
  `host/src/flows/run_store.rs` (1004). Each cleanup ends with `--update`.
- Consider `cargo test --workspace --no-fail-fast` so one failure stops masking the
  rest (`preexisting-failing-tests`). Left out here to avoid changing runtime/semantics
  in the same change that fixes the disk ceiling.
- `packages/thecrew/` contains only a stray `node_modules` and no `package.json` —
  probably deletable.
