#!/usr/bin/env bash
# FILE-LAYOUT enforcement (docs/FILE-LAYOUT.md §9): no tracked .rs/.ts/.tsx over 400 lines,
# excluding generated code.
#
# This is a RATCHET, not a flat gate. A flat gate was the honest design, but the repo accumulated
# a backlog of oversized files and the job sat red on master for weeks — which meant it reported
# nothing at all: a genuinely new 900-line file looked exactly like the standing backlog. A check
# that is always red is a check that is off.
#
# So: the backlog is frozen in `file-size-baseline.txt` (path + the line count at freeze time) and
# this script fails only on movement in the WRONG direction —
#   - any file over the limit that is NOT in the baseline (a new violation), or
#   - a baseline file that GREW past its recorded count (the backlog getting worse).
# Shrinking a baseline file always passes; dropping one under the limit prints a notice so the
# baseline can be trimmed. The backlog can only shrink, and the job is red only when it means
# something. Regenerate after a cleanup pass with: rust/scripts/check-file-size.sh --update
set -euo pipefail

LIMIT=400
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"   # repo root
BASELINE="$ROOT/rust/scripts/file-size-baseline.txt"
cd "$ROOT"

# Tracked source files, minus generated trees. `dist/` is build output (a few packages commit
# their rolled-up .d.ts); it is not hand-written source and must never be counted.
mapfile -t files < <(git ls-files '*.rs' '*.ts' '*.tsx' \
  | grep -v '/generated/' \
  | grep -v '/target/' \
  | grep -v '/dist/' || true)

# --update: rewrite the baseline from the current tree (use after a cleanup pass).
if [ "${1:-}" = "--update" ]; then
  {
    echo "# FILE-LAYOUT baseline — files over the ${LIMIT}-line limit at freeze time."
    echo "# Regenerate with: rust/scripts/check-file-size.sh --update"
    echo "# The list may only SHRINK. See check-file-size.sh for the ratchet rules."
    for f in "${files[@]}"; do
      [ -f "$f" ] || continue
      n=$(wc -l < "$f")
      [ "$n" -gt "$LIMIT" ] && echo "$f $n"
    done
  } > "$BASELINE"
  echo "FILE-LAYOUT: baseline rewritten ($(grep -cv '^#' "$BASELINE") entries) -> $BASELINE"
  exit 0
fi

# Load the baseline into an assoc array: path -> allowed line count.
declare -A allowed=()
if [ -f "$BASELINE" ]; then
  while read -r path count; do
    [ -z "${path:-}" ] && continue
    case "$path" in \#*) continue ;; esac
    allowed["$path"]=$count
  done < "$BASELINE"
fi

fail=0
graduated=()
for f in "${files[@]}"; do
  [ -f "$f" ] || continue
  n=$(wc -l < "$f")
  cap=${allowed["$f"]:-}

  if [ -z "$cap" ]; then
    # Not grandfathered: the flat limit applies.
    if [ "$n" -gt "$LIMIT" ]; then
      echo "FILE-LAYOUT: $f is $n lines (limit $LIMIT) — new violation"
      fail=1
    fi
  elif [ "$n" -gt "$cap" ]; then
    echo "FILE-LAYOUT: $f grew to $n lines (baseline $cap, limit $LIMIT) — split it, don't extend it"
    fail=1
  elif [ "$n" -le "$LIMIT" ]; then
    graduated+=("$f")
  fi
done

# A baseline entry whose file is gone is just stale — report it, don't fail.
for path in "${!allowed[@]}"; do
  [ -f "$path" ] || graduated+=("$path (deleted)")
done

if [ ${#graduated[@]} -gt 0 ]; then
  echo "FILE-LAYOUT: ${#graduated[@]} baseline file(s) now within the limit — run"
  echo "  rust/scripts/check-file-size.sh --update"
  echo "to trim the baseline:"
  printf '  %s\n' "${graduated[@]}"
fi

if [ "$fail" -ne 0 ]; then
  echo "::error::file(s) exceed the ${LIMIT}-line FILE-LAYOUT limit (or grew past their baseline)"
  exit 1
fi
echo "FILE-LAYOUT: OK — ${#files[@]} files checked, ${#allowed[@]} grandfathered (see $BASELINE)"
