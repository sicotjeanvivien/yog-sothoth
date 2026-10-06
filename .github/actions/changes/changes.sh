#!/usr/bin/env bash
# Whether a workflow's jobs run on this event: `run=true` or `run=false` to
# $GITHUB_OUTPUT. On a pull request, true when a file the PR changes matches
# PATTERN (an extended regex over repo-relative paths); on any other event,
# true, the workflow's own `paths:` filter having decided already.
#
# Inputs, from the environment: EVENT, BASE, HEAD (the PR's base and head
# SHAs), PATTERN.
#
# Three traps, each of which would skip a workflow's jobs and turn its
# required verdict green on a PR it should have checked:
# - --no-renames: with rename detection on, `--name-only` lists a moved file
#   by its destination alone, so a file moved OUT of a watched tree would
#   not be seen leaving it;
# - core.quotePath=false: by default git prints a path with non-ASCII bytes
#   quoted and escaped ("web/caf\303\251.ts"), and an anchored pattern no
#   longer matches it;
# - grep's exit code 2: a broken PATTERN must fail the job, not read as "no
#   file matched".
set -euo pipefail

if [ "$EVENT" != pull_request ]; then
  echo "run=true" >>"$GITHUB_OUTPUT"
  exit 0
fi

changed=$(git -c core.quotePath=false diff --no-renames --name-only "$BASE...$HEAD")
rc=0
touched=$(grep -E -- "$PATTERN" <<<"$changed") || rc=$?
if [ "$rc" -gt 1 ]; then
  echo "the path pattern of this workflow is not a valid regex (grep exited $rc): $PATTERN" >&2
  exit "$rc"
fi

if [ -n "$touched" ]; then
  echo "run=true" >>"$GITHUB_OUTPUT"
  echo "files this workflow checks:"
  echo "$touched"
else
  echo "run=false" >>"$GITHUB_OUTPUT"
  echo "no file on this workflow's paths changed: its jobs are skipped"
fi
