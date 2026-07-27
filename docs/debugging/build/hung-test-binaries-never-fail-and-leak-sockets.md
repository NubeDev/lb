# Hung test binaries never fail — they wedge the run and leak sockets

Tracking: [#109](https://github.com/NubeDev/lb/issues/109)

## Symptom

`cargo test` never returns. Nothing fails, nothing is reported, the job just sits there. This is
NOT the same shape as a flaky test, and it is why the "flaky `agent_*` tests" reading was wrong:
`--no-fail-fast` cannot report a test that never finishes, so the log's last line is whatever
happened to print before the wedge.

Worse, the test binary **outlives the run**. Six such ghosts were found on a dev box:

| what | value |
| --- | --- |
| oldest ghost | 4d15h |
| FDs held, all six | 50,771 |
| LISTEN sockets held | 76 |

The leak is not confined to the run that caused it: a later test that binds a fixed port races a
days-old ghost of itself. A "flaky port-in-use failure" weeks later is the same bug.

## Evidence

Two ghosts were caught live and dumped from `/proc` (no gdb on the box; `rust-gdb` is only a
wrapper around it, so it is inert here too):

```
flows_run_test   elapsed 29:13 (MM:SS)   cpu 00:00:14   — all 28 test threads State S, futex_do_wait
insights_test    elapsed 18:52 (MM:SS)   cpu 00:00:06   — same shape
```

Neither was making progress: sampled twice ~8 minutes apart, wall clock advanced and CPU did not.

### The discriminator is CPU-vs-elapsed, NOT wchan

Measured, not assumed. A **healthy** `flows_run_test` sampled 5s into a normal run shows the exact
same wchan on every test thread:

```
TID      COMM               STATE  WCHAN
1722360  flows_run_test-    S      futex_do_wait
1722362  any_funnel_fire    S      futex_do_wait
1722363  auto_wire_flows    S      futex_do_wait
...
1722375  tokio-rt-worker    S      ep_poll
cpu/elapsed: 00:00:05   00:05        <-- CPU tracks elapsed: BUSY, not stuck
```

Those threads are parked on libtest's own concurrency gate, which is normal. So `futex_do_wait`
alone proves nothing. What separates the ghost from the healthy run is the **ratio**: the healthy
binary burned 5s of CPU in 5s of wall clock; the ghost burned 14s of CPU in 29m of wall clock and
was still climbing in wall clock only. Read `cpu/elapsed` first.

(`ps` etime `20:51` is MM:SS — those two ghosts were ~20 and ~29 minutes old when sampled, not
hours. The 4d15h figure above came from a separate, older ghost found earlier. Same disease,
different age; a ghost survives until something reaps it.)

The other high-value field is `comm`: libtest names each test's thread after the test itself
(kernel-truncated to 15 chars), so the live threads name the candidates. Cross-reference against
the `test … ok` lines printed above the dump — what is running but never reported is what is stuck.

Also worth knowing: a healthy `flows_run_test` **5 seconds in** already holds 6,585 FDs / 2,468
sockets / 28 listening. The suite is socket-heavy by nature (rule 9 — real nodes, real sidecars),
which is why one wedged binary leaks at the scale it does.

## The hang is intermittent

`flows_run_test` run standalone under the guard **passed in 21.46s** — while another copy of that
same binary sat parked and making no progress a few PIDs away. So this does not reproduce on demand from the binary alone — contention
matters (two other `cargo test -p lb-host` runs were live on the box at the time). Do not expect a
single re-run to confirm or refute it.

## The guard

`rust/scripts/test-timeout-runner.sh` — a cargo `runner` wrapper that turns a hung binary into a
failed one: bounds each test binary, dumps the diagnostics above, kills the process **tree**, exits
124.

Why a cargo runner rather than in-process: the workspace has 269 integration test targets across 27
crates. An in-process watchdog needs arming in every one (a dev-dependency is not linked unless it
is referenced — there is no zero-touch version). A runner wraps every test binary with no source
change, no new dependency, and keeps working under the plain sharded `cargo test` that CI is
deliberately built around.

Scope guard: only binaries under `target/**/deps/` are bounded, so `cargo run -p lb-node` — which
is supposed to run forever — is exec'd untouched.

Killing the **tree** is load-bearing: the tests spawn real nodes and sidecars, and an orphaned child
keeps its port bound long after the parent is gone. Half of #109's damage was leaked children.

### Wiring

| where | how | note |
| --- | --- | --- |
| this box | `runner =` in `rust/.cargo/config.toml` | **gitignored** (holds local zigcc paths) — local only |
| `make test-be` | `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER` in `Makefile` | committed |
| CI | same var in `.github/workflows/ci.yml` job env, `LB_TEST_TIMEOUT_SECS: 600` | committed |

The path must be **absolute**: cargo runs a test binary with cwd = its *package* root
(`rust/crates/host`), so a relative `scripts/…` does not resolve.

Budget: `LB_TEST_TIMEOUT_SECS`, default 900s, `0` disables.

### Verification

- `rust/scripts/test-timeout-runner-selftest.sh` — 13/13 green. Exercises the real script against
  throwaway stand-in binaries (a permanently-hanging `#[test]` in the workspace would be the
  disease itself). Covers: hang → 124, real failure's exit code preserved, non-test binary
  untouched, `=0` disables, cargo's args forwarded.
- Real binary, real timeout: `LB_TEST_TIMEOUT_SECS=5` against `flows_run_test` → exit 124,
  diagnostics named the in-flight tests, **zero survivors** after the tree kill.
- Under real cargo, end to end: a throwaway crate with a hanging `#[test]` →
  `error: test failed … (exit status: 124)`. Cargo honors the env var and reports the hang as a
  failure. This is the whole objective.

## What this does and does not fix

It makes the hang **loud, bounded and attributable**. It does not fix the deadlock. The root cause
is still open on #109 — the shared shape is the Zenoh transport pool (`app-0`/`net-0`/`acc-0`/`tx-0`
/`rx-N` threads) present in both ghosts; `Bus::peer()` / `peer_with()` lifecycle in
`crates/host/src/bus/peer.rs` (whose existing doc comment at ~line 51 already describes this exact
symptom) is the next thread to pull.
