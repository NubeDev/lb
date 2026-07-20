# CARGO — build cost, `target/` size, and the test-binary problem

Why `rust/target/` grows to hundreds of gigabytes in this repo, what actually
causes it, and the fix that is **partially verified** (see §5 — the size win is
measured, the correctness of the change is **not yet proven**).

> **Status: IN PROGRESS, NOT LANDED.** One group (`agent_*`) has been converted
> in the working tree as a proof. It links and the size win is real and measured.
> The suite has **not** been shown green. Do not convert the remaining groups and
> do not merge until §5 is closed out.

---

## 1. The symptom

`rust/target/` reaches **265 GB** on a 916 GB disk (89% full), forcing repeated
`cargo clean`. Wiping feels like the only lever, and it is deeply annoying,
because the space comes straight back on the next `cargo test --workspace`.

That is the tell: **this is not stale-artifact accumulation.** `cargo clean`
reclaims the disk and the next build regenerates all 265 GB, because nearly every
byte is a *live, current* artifact. Garbage collection cannot fix a working set
that is genuinely that large.

Measured breakdown:

| Path | Size |
|---|---|
| `target/debug/deps` | **248 GB** |
| `target/debug/incremental` | 16 GB |
| `target/debug/build` | 986 MB |
| `target/debug/examples` | 289 MB |

---

## 2. The cause

Two independent multipliers stack.

### 2.1 One binary per integration-test file

Cargo compiles **every top-level `.rs` file in `tests/`** as its own crate, with
its own `main`, statically linking the **entire dependency graph** — SurrealDB,
Zenoh, wasmtime — into each one.

| Count | What |
|---|---|
| 259 | integration test targets, workspace-wide |
| **164** | in `crates/host/tests/` alone |
| 214 | distinct ~1.1 GB test binaries in `deps/` |
| 4 | of those are stale duplicates (i.e. ~none) |

Only 4 stale duplicates out of 218 binaries. **210+ are live.** You are storing
~200 near-identical copies of the same enormous dependency graph.

This is `FILE-LAYOUT.md` rule 8 ("one responsibility per file") colliding with
Cargo's compilation model. The rule is correct for `src/`, where a file costs
nothing. In `tests/`, **each top-level file costs a gigabyte.**

### 2.2 Debug info is ~⅔ of every binary

Measured by stripping one test binary:

```
before:   0.94 GB
stripped: 0.32 GB
```

`[profile.dev]` in `rust/Cargo.toml` already sets `debug = "line-tables-only"`
and `split-debuginfo = "unpacked"` (to stop `rust-lld` OOM-ing the desktop — see
the comments there and `rust/.cargo/config.toml`). Those settings do **not** fully
reach test binaries, which is why ~⅔ of each one is still debug info.

---

## 3. The fix: aggregate test files into harness binaries

Cargo only auto-discovers **top-level** `tests/*.rs` as targets. Files in a
**subdirectory** are not compiled as separate targets — so they can be pulled in
as `mod`s of a single harness.

This keeps the on-disk file layout exactly as `FILE-LAYOUT.md` wants it (one
responsibility per file, unchanged filenames) while collapsing N binaries to 1.

**Pattern** — `tests/agent_suite.rs`:

```rust
#[path = "agent/agent_active_model_test.rs"]
mod agent_active_model_test;
#[path = "agent/agent_compact_test.rs"]
mod agent_compact_test;
// … one pair per file
```

Steps:

1. `git mv agent_*.rs agent/` — use `git mv` so history follows the files.
2. Generate the aggregator with one `#[path]` + `mod` pair per file. `#[path]`
   means **no file is renamed**; `agent/` is just a directory Cargo ignores.
3. Fix relative macro paths — see §4.1.

---

## 4. Gotchas found while converting `agent_*`

### 4.1 `include_str!` paths break — files moved one level deeper

6 files hit this. `include_str!` resolves **relative to the containing file**, so
moving into `agent/` needs one more `../`:

```rust
// before
const MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");
// after
const MANIFEST: &str = include_str!("../../../../extensions/hello/extension.toml");
```

Compile errors are loud and rustc suggests the exact fix. Mechanical.

### 4.2 Previously-invisible dead code appears

5 new warnings (unused imports, an unused `const`, an unused `let`). These are
**pre-existing** — each file used to be its own crate, so a helper unused *within
that file* was still a crate-level item and never flagged. As modules of one
crate, the compiler now sees the truth. Clean them up; they are real.

### 4.3 Process-global state is now shared — CHECK BEFORE CONVERTING

Merged tests share **one process**. Anything process-global can now collide
across files that were previously isolated. Audit before moving:

```bash
grep -ln "set_var\|remove_var" <group>_*.rs               # env mutation
grep -n  "127\.0\.0\.1:[1-9][0-9]\{3,\}" <group>_*.rs     # fixed ports
grep -ln "static \|lazy_static\|OnceLock" <group>_*.rs    # shared state
grep -ln "fn main()" <group>_*.rs                         # own harness
```

For `agent_*` all hits were benign: two `set_var` uses with uniquely-named vars
that clean up after themselves, and one `&'static str` struct field (not shared
state). Note `agent/agent_def_test_test.rs` carries a comment explicitly assuming
a *single-threaded* env — unique var names save it, but that assumption is now
load-bearing across a much larger binary.

**This class of hazard is the main risk of the whole approach**, and it is the
prime suspect for §5.

---

## 5. ⚠ UNRESOLVED: the converted suite has not been shown green

The size win is measured and real. **Correctness is not established.**

| Metric | Before | After |
|---|---|---|
| `agent_*` binaries | 30 | **1** |
| `agent_*` disk | **28.48 GB** | **0.98 GB** |
| Tests in binary | — | 167 |
| Compiles | — | yes (`EXIT=0`) |
| **Tests pass** | — | **UNKNOWN** |

29 files' worth of tests now cost what *one* used to. Extrapolated across all 259
targets that is 265 GB → roughly 15–25 GB.

**But:** `cargo test --test agent_suite -p lb-host` **timed out at 10 minutes**,
both parallel and with `--test-threads=1`. Serial timing out too means it is
**not** simple thread contention. The run that would have isolated the stalling
test was interrupted before it produced output.

Open question — one of:

- a genuine deadlock/hang from shared process state (§4.3), i.e. **a real defect
  introduced by aggregation**; or
- 167 real `Node::boot()`-backed tests (real SurrealDB + Zenoh, per rule 9) in one
  process legitimately exceeding 10 min, i.e. **just slow**; or
- the known `rules_test`-style box-load stall — a harness artifact, not a code
  problem.

**Next step:** run the binary directly with per-test visibility and no cargo
wrapper, to name the stalling test:

```bash
B=$(find rust/target/debug/deps -maxdepth 1 -type f -executable -name 'agent_suite-*' | head -1)
timeout 480 "$B" --test-threads=1 2>&1 | tail -25
```

Compare against the same tests passing pre-conversion. Until this is answered,
**treat aggregation as unproven** — a change that links is not a change that
works, and the size number alone must not be read as success.

---

## 6. Secondary lever: trim test debug info

Worth ~⅔ of whatever remains after §3, but do it **after** §5 closes — changing
two variables at once makes a regression impossible to attribute.

```toml
[profile.test]
debug = 0            # or "line-tables-only" where backtraces are needed
```

Trade-off: less useful backtraces. Given `[profile.dev]` already runs
`line-tables-only` for RAM reasons, this is a modest further step.

---

## 7. Housekeeping (does NOT fix the problem)

`cargo-sweep` / a `target-prune` make target only reclaims genuinely stale
artifacts — **4 duplicate binaries today**. Worth having, but it is not the fix,
and reaching for it first is what makes this feel unfixable.

---

## 8. Rule of thumb

> A new **top-level** file in `tests/` costs ~1 GB of `target/` and a full link.
> Add the file — `FILE-LAYOUT.md` still applies — but add it as a **module of an
> existing suite**, not as a new top-level target.

---

## Current working-tree state

Uncommitted, from the `agent_*` proof:

- 29 files moved `crates/host/tests/agent_*.rs` → `crates/host/tests/agent/`
- new `crates/host/tests/agent_suite.rs` aggregator (68 lines)
- 6 `include_str!` paths fixed (§4.1)
- top-level targets in `crates/host/tests/`: **164 → 136**

To abandon: `git checkout -- rust/crates/host/tests/` and delete
`agent_suite.rs` + the `agent/` directory.

---

## See also

- `rust/.cargo/config.toml` — `jobs = 4` and the zig C toolchain, both load-bearing.
- `rust/Cargo.toml` — `[profile.dev]` link-RAM tuning; `libsqlite3-sys` opt-level.
- `docs/FILE-LAYOUT.md` — rule 8, which this doc qualifies **for `tests/` only**.
- `docs/scope/testing/testing-scope.md` — no mocks; tests boot real infrastructure,
  which is why 167-in-one-process is plausibly just slow.
