#!/usr/bin/env bash
# A workflow's verdict: exits 0 only when every job in RESULTS (the calling
# job's `toJSON(needs)`) ended `success` or `skipped`. Any other result fails
# it — `failure`, `cancelled`, and those GitHub does not document, such as
# `abandoned` — and so does a RESULTS that is not a non-empty JSON object.
set -euo pipefail

# ⚠️ Not `jq -e`: on an empty input, jq 1.6 prints nothing and exits 0.
if [ "$(jq 'type == "object" and length > 0' <<<"$RESULTS")" != true ]; then
  echo "the needs context is not a non-empty JSON object" >&2
  exit 1
fi

jq -r 'to_entries[] | "\(.key): \(.value.result)"' <<<"$RESULTS"
refused=$(jq -r 'to_entries[]
  | select(.value.result != "success" and .value.result != "skipped")
  | "\(.key): \(.value.result)"' <<<"$RESULTS")
if [ -n "$refused" ]; then
  printf 'not success or skipped:\n%s\n' "$refused" >&2
  exit 1
fi
