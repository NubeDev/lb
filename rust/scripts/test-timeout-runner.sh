#!/usr/bin/env bash
#
# Cargo `runner` wrapper that turns a HUNG test binary into a FAILED one.
#
# Why this exists (issue #109): integration test binaries in this workspace can deadlock —
# blocked in `futex_do_wait` on a lock that is never signalled — rather than fail. `cargo test`
# then waits forever, and the binary survives the run, holding its listening sockets and file
# descriptors. Six such processes were found on a dev box, the oldest 4d15h old, together holding
# 50,771 FDs and 76 LISTEN sockets. A later test that binds a fixed port races a days-old ghost of
# itself, so the damage is not confined to the run that leaked.
#
# A hang that never fails is invisible: `--no-fail-fast` reports nothing, and the job simply
# wedges. This wrapper makes the hang LOUD and BOUNDED — it dumps per-thread diagnostics, kills
# the process tree, and exits non-zero.
#
# ## Why a cargo runner and not in-process code
#
# The workspace has 269 integration test targets across 27 crates. An in-process watchdog would
# need arming in every one of them (a `dev-dependency` is not linked unless it is referenced, so
# there is no zero-touch version). A `runner` wraps every test binary cargo executes, with no
# source change and no new dependency, and it keeps working under the plain sharded `cargo test`
# that CI is deliberately built around (see .github/workflows/ci.yml).
#
# ## Scope guard
#
# Cargo uses `runner` for `cargo run` and `cargo bench` too, and `cargo run -p lb-node` is
# SUPPOSED to run forever. So the timeout is applied ONLY to binaries under `target/**/deps/`,
# which is where cargo puts test binaries and nowhere else. Everything else is exec'd untouched.
#
# ## Knobs
#
#   LB_TEST_TIMEOUT_SECS   per-binary wall-clock budget. Default 900 (15 min). `0` disables.
#
# Usage (wired up in rust/.cargo/config.toml — you do not call this by hand):
#   runner = "scripts/test-timeout-runner.sh"

set -uo pipefail

bin="${1:?test-timeout-runner: no binary given}"
shift

secs="${LB_TEST_TIMEOUT_SECS:-900}"

# Not a test binary, or the guard is switched off — get out of the way entirely.
case "$bin" in
*/deps/*) ;;
*) exec "$bin" "$@" ;;
esac
if [ "$secs" = "0" ]; then
	exec "$bin" "$@"
fi

# ---------------------------------------------------------------------------------------------
# Diagnostics. No gdb on this box (and `rust-gdb` is only a wrapper around it), so the portable
# evidence is /proc.
#
# Read the dump in this order:
#
#   1. `cpu/elapsed` is the DISCRIMINATOR. Near-zero CPU against a large wall clock is a parked
#      lock; CPU tracking elapsed is just slow work. In #109 a ghost showed 14s CPU against 29m
#      elapsed, and 8 minutes later still 14s. WCHAN alone proves nothing — a HEALTHY run 5s in
#      shows `futex_do_wait` on every test thread too (they are parked on libtest's own
#      concurrency gate). Measured, not conjectured — the captured dumps are in
#      docs/debugging/build/hung-test-binaries-never-fail-and-leak-sockets.md.
#
#   2. `comm` NAMES the candidates: libtest names each test's thread after the test itself
#      (truncated to 15 chars by the kernel), so the live threads tell you which tests were still
#      in flight. Cross-reference against the `test … ok` lines already printed above the dump —
#      what is running but never reported is what is stuck.
#
#   3. `listening`/`sockets` size the leak this particular hang would have caused.
# ---------------------------------------------------------------------------------------------
dump_diagnostics() {
	local pid="$1"
	shift
	echo
	echo "================================ TEST BINARY TIMED OUT ================================"
	echo "binary   : $bin"
	echo "args     : $*"
	echo "pid      : $pid"
	echo "budget   : ${secs}s (LB_TEST_TIMEOUT_SECS)"
	echo "tracking : https://github.com/NubeDev/lb/issues/109"
	echo

	echo "--- live threads (libtest names a test's thread after the test) ---"
	printf '%-8s %-18s %-6s %s\n' TID COMM STATE WCHAN
	local t tid comm state wchan
	for t in /proc/"$pid"/task/*; do
		[ -d "$t" ] || continue
		tid="${t##*/}"
		comm=$(cat "$t/comm" 2>/dev/null || echo '?')
		state=$(awk '/^State:/{print $2}' "$t/status" 2>/dev/null || echo '?')
		wchan=$(cat "$t/wchan" 2>/dev/null || echo '?')
		printf '%-8s %-18s %-6s %s\n' "$tid" "$comm" "$state" "${wchan:-running}"
	done

	echo
	echo "--- resources held ---"
	local fds socks listen
	fds=$(ls /proc/"$pid"/fd 2>/dev/null | wc -l)
	socks=$(ls -l /proc/"$pid"/fd 2>/dev/null | grep -c socket)
	listen=$(ss -ltnp 2>/dev/null | grep -c "pid=$pid," || true)
	echo "open fds: $fds   sockets: $socks   listening: $listen"
	echo "cpu/elapsed: $(ps -o time=,etime= -p "$pid" 2>/dev/null || echo '?')"

	# Best-effort user-space backtrace when a debugger IS available (CI images often have gdb).
	if command -v gdb >/dev/null 2>&1; then
		echo
		echo "--- gdb backtraces ---"
		gdb -p "$pid" -batch -ex 'thread apply all bt' 2>/dev/null | head -200
	elif command -v eu-stack >/dev/null 2>&1; then
		echo
		echo "--- eu-stack backtraces ---"
		eu-stack -p "$pid" 2>/dev/null | head -200
	else
		echo
		echo "(no gdb/eu-stack on this host — install one for full backtraces. Without them, read"
		echo " cpu/elapsed above: near-zero CPU against a large elapsed time is a parked lock."
		echo " WCHAN alone does NOT tell you — healthy in-flight tests also sit in futex_do_wait.)"
	fi
	echo "======================================================================================"
	echo
}

# Kill the binary AND anything it spawned. The tests start real sidecars and real nodes (rule 9),
# and an orphaned child keeps the port bound long after its parent is gone — half of #109's
# damage was leaked children, not the test binary itself.
kill_tree() {
	local pid="$1" child
	for child in $(pgrep -P "$pid" 2>/dev/null); do
		kill_tree "$child"
	done
	kill -9 "$pid" 2>/dev/null
}

"$bin" "$@" &
child=$!

# Ctrl-C / cargo giving up must take the test binary down with us, or we have re-created the exact
# leak this script exists to prevent. (This is also why the child is NOT put in its own session:
# a detached process group would survive the signal.)
trap 'kill_tree "$child"; exit 130' INT TERM

waited=0
while kill -0 "$child" 2>/dev/null; do
	if [ "$secs" -gt 0 ] && [ "$waited" -ge "$secs" ]; then
		dump_diagnostics "$child" "$@"
		kill_tree "$child"
		wait "$child" 2>/dev/null
		exit 124
	fi
	sleep 1
	waited=$((waited + 1))
done

wait "$child"
exit $?
