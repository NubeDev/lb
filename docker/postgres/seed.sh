#!/usr/bin/env bash
# Thin wrapper around seed.py — kept for backward compatibility with the
# README and any docs that say `./seed.sh`. All flags pass through.
#
# The Python implementation gives per-meter-randomized data, HVAC behaviour,
# and Haystack tags that the old bash generator could not.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$DIR/seed.py" "$@"
