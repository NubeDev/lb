#!/usr/bin/env bash
# FILE-LAYOUT enforcement (docs/FILE-LAYOUT.md §9): no tracked .rs/.ts/.tsx over 400 lines,
# excluding generated code. Fails CI (exit 1) on any offender.
set -euo pipefail

LIMIT=400
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"   # repo root
cd "$ROOT"

# Tracked source files, minus generated trees.
mapfile -t files < <(git ls-files '*.rs' '*.ts' '*.tsx' \
  | grep -v '/generated/' \
  | grep -v '/target/' || true)

fail=0
for f in "${files[@]}"; do
  [ -f "$f" ] || continue
  n=$(wc -l < "$f")
  if [ "$n" -gt "$LIMIT" ]; then
    echo "FILE-LAYOUT: $f is $n lines (limit $LIMIT)"
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  echo "::error::file(s) exceed the ${LIMIT}-line FILE-LAYOUT limit"
  exit 1
fi
echo "FILE-LAYOUT: all source files within ${LIMIT} lines (${#files[@]} checked)"
