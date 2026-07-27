#!/usr/bin/env bash
#
# Self-test for `test-timeout-runner.sh` (issue #109).
#
# The guard's whole job is to fire on a hang — a condition that, by definition, no passing test in
# the suite produces. So the guard needs its own proof, and it cannot live as a normal `#[test]`
# (a permanently-hanging test in the workspace would be exactly the disease). It is exercised here
# against throwaway stand-in binaries instead.
#
# Nothing is mocked in the sense rule 9 cares about: the REAL runner script is run, against real
# processes, and the real exit codes / real /proc dump / real process death are asserted. The
# stand-ins are only stand-ins for "a program that hangs" and "a program that exits 3".
#
#   ./scripts/test-timeout-runner-selftest.sh

set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
runner="$here/test-timeout-runner.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/deps" "$tmp/notdeps"
pass=0
fail=0

check() {
	local name="$1" ok="$2" detail="${3:-}"
	if [ "$ok" = "yes" ]; then
		echo "ok   — $name"
		pass=$((pass + 1))
	else
		echo "FAIL — $name ${detail:+($detail)}"
		fail=$((fail + 1))
	fi
}

# --- 1. a hung binary under deps/ is killed, and the failure is non-zero -----------------------
cat >"$tmp/deps/hang_test-abc123" <<'EOF'
#!/usr/bin/env bash
sleep 300
EOF
chmod +x "$tmp/deps/hang_test-abc123"

out="$tmp/hang.out"
LB_TEST_TIMEOUT_SECS=3 "$runner" "$tmp/deps/hang_test-abc123" >"$out" 2>&1
code=$?

[ "$code" = "124" ] && check "hung binary exits non-zero (124)" yes || check "hung binary exits non-zero (124)" no "got $code"
grep -q "TEST BINARY TIMED OUT" "$out" && check "timeout is reported loudly" yes || check "timeout is reported loudly" no
grep -q "issues/109" "$out" && check "dump points at the tracking issue" yes || check "dump points at the tracking issue" no
grep -q "TID *COMM *STATE *WCHAN" "$out" && check "per-thread WCHAN table is dumped" yes || check "per-thread WCHAN table is dumped" no
grep -q "open fds:" "$out" && check "held resources are reported" yes || check "held resources are reported" no

# The point of the exercise: no survivor. This is the leak that #109 is actually about.
if pgrep -f "hang_test-abc123" >/dev/null 2>&1; then
	check "hung process is dead afterwards" no "still running"
	pkill -9 -f "hang_test-abc123" 2>/dev/null
else
	check "hung process is dead afterwards" yes
fi

# --- 2. children are reaped too (a leaked child keeps the port bound) --------------------------
cat >"$tmp/deps/parent_test-def456" <<'EOF'
#!/usr/bin/env bash
sleep 300 &   # the "sidecar"
sleep 300
EOF
chmod +x "$tmp/deps/parent_test-def456"

LB_TEST_TIMEOUT_SECS=3 "$runner" "$tmp/deps/parent_test-def456" >/dev/null 2>&1
sleep 1
if pgrep -P 1 -f "^sleep 300$" >/dev/null 2>&1 && pgrep -f "parent_test-def456" >/dev/null 2>&1; then
	check "spawned children are reaped" no "orphan survived"
	pkill -9 -f "parent_test-def456" 2>/dev/null
else
	check "spawned children are reaped" yes
fi

# --- 3. a fast binary is untouched and its exit code passes through ----------------------------
cat >"$tmp/deps/quick_test-ghi789" <<'EOF'
#!/usr/bin/env bash
echo "3 passed"
exit 0
EOF
chmod +x "$tmp/deps/quick_test-ghi789"
out2=$(LB_TEST_TIMEOUT_SECS=60 "$runner" "$tmp/deps/quick_test-ghi789" 2>&1)
code2=$?
[ "$code2" = "0" ] && check "passing binary still exits 0" yes || check "passing binary still exits 0" no "got $code2"
echo "$out2" | grep -q "3 passed" && check "stdout passes through" yes || check "stdout passes through" no

cat >"$tmp/deps/failing_test-jkl012" <<'EOF'
#!/usr/bin/env bash
exit 101
EOF
chmod +x "$tmp/deps/failing_test-jkl012"
LB_TEST_TIMEOUT_SECS=60 "$runner" "$tmp/deps/failing_test-jkl012" >/dev/null 2>&1
[ "$?" = "101" ] && check "a real test failure's exit code is preserved" yes || check "a real test failure's exit code is preserved" no

# --- 4. scope guard: `cargo run` (not under deps/) must NOT be time-boxed ----------------------
# A long-running node is the point of `cargo run`; the guard must ignore it.
cat >"$tmp/notdeps/node" <<'EOF'
#!/usr/bin/env bash
sleep 6
echo "served"
EOF
chmod +x "$tmp/notdeps/node"
out4=$(LB_TEST_TIMEOUT_SECS=2 "$runner" "$tmp/notdeps/node" 2>&1)
code4=$?
[ "$code4" = "0" ] && echo "$out4" | grep -q served &&
	check "non-test binary outlives the budget untouched" yes ||
	check "non-test binary outlives the budget untouched" no "code=$code4"

# --- 5. the escape hatch actually disables the guard -------------------------------------------
cat >"$tmp/deps/slow_test-mno345" <<'EOF'
#!/usr/bin/env bash
sleep 4
echo "finished late"
EOF
chmod +x "$tmp/deps/slow_test-mno345"
out5=$(LB_TEST_TIMEOUT_SECS=0 "$runner" "$tmp/deps/slow_test-mno345" 2>&1)
[ "$?" = "0" ] && echo "$out5" | grep -q "finished late" &&
	check "LB_TEST_TIMEOUT_SECS=0 disables the guard" yes ||
	check "LB_TEST_TIMEOUT_SECS=0 disables the guard" no

# --- 6. argument forwarding (cargo passes filters/--nocapture through the runner) --------------
cat >"$tmp/deps/args_test-pqr678" <<'EOF'
#!/usr/bin/env bash
echo "args:$*"
EOF
chmod +x "$tmp/deps/args_test-pqr678"
out6=$(LB_TEST_TIMEOUT_SECS=60 "$runner" "$tmp/deps/args_test-pqr678" --nocapture my_filter 2>&1)
echo "$out6" | grep -q -- "args:--nocapture my_filter" &&
	check "cargo's test args are forwarded" yes ||
	check "cargo's test args are forwarded" no "got: $out6"

echo
echo "passed: $pass   failed: $fail"
[ "$fail" = "0" ]
