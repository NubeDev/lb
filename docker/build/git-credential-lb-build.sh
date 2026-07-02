#!/usr/bin/env bash
# docker/build/git-credential-lb-build.sh — a git credential helper (see `git help
# credential`) that hands back a build-scoped token from the LB_BUILD_GIT_TOKEN env var.
#
# git invokes credential helpers with a subcommand (`get`, `store`, `erase`) and a
# key=value block on stdin; only `get` needs a real answer here. We deliberately never
# echo the token into a URL or a log line — this helper is the one place it's read, and
# it goes straight into git's credential protocol on stdout.
#
# If LB_BUILD_GIT_TOKEN is unset (no build-scoped secret configured), this yields nothing
# and git falls through to its normal (failing) auth — a private-dep checkout gets a
# plain auth error, not a silent bypass.
set -euo pipefail

action="${1:-get}"
# Drain stdin (the key=value block git sends) — we don't need it, but must consume it.
cat >/dev/null || true

[ "$action" = "get" ] || exit 0
[ -n "${LB_BUILD_GIT_TOKEN:-}" ] || exit 0

echo "username=x-access-token"
echo "password=${LB_BUILD_GIT_TOKEN}"
