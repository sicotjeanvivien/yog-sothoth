#!/usr/bin/env bash
# Fail when a runtime dependency of the dashboard carries a known advisory
# (moderate or above) that `audit-exceptions.json` does not accept in writing.
#
# The web counterpart of `cargo audit` and `.cargo/audit.toml`. Run by the
# `audit` job of `.github/workflows/web-quality.yml`, and locally as
# `bash web/audit.sh` — the same file both ways.
#
# Runtime only (`--omit=dev`): the image ships Next.js's standalone output, not
# `node_modules`, and no devDependency is imported outside the tests. That rests
# on `package.json` filing each package where it belongs — a runtime import
# declared under devDependencies would escape this check.
#
# `audit-exceptions.json` is an array of entries, each with three non-empty
# strings (JSON takes no comments, so the format lives here):
#   - `id`     — the advisory, as its GHSA identifier (`GHSA-xxxx-xxxx-xxxx`);
#   - `reason` — why it is accepted: not reachable here, no fix yet, …;
#   - `revisit` — when to look again: a version, a date, an event.
# An entry missing any of them fails the check, before the audit runs.

set -euo pipefail
cd "$(dirname "$0")"

exceptions=audit-exceptions.json

incomplete=$(jq -c '.[] | select(
    ([.id, .reason, .revisit] | map(type == "string" and length > 0) | all) | not
  )' "$exceptions")
if [ -n "$incomplete" ]; then
  echo "❌ $exceptions: every exception needs a non-empty id, reason and revisit:" >&2
  echo "$incomplete" >&2
  exit 1
fi

# npm exits non-zero as soon as an advisory exists; the report decides, not
# its exit code. Its stderr goes straight to the log: a warning mixed into the
# JSON would read as a missing report.
report=$(npm audit --package-lock-only --omit=dev --json || true)

# No report is not a clean report: an unreachable registry must not read as
# "0 advisories". `input` makes jq fail on empty output too — `jq -e` alone
# reads no value there and exits 0 (jq 1.6, measured).
if ! jq -en 'input | .metadata.vulnerabilities | type == "object"' >/dev/null 2>&1 <<<"$report"; then
  echo "❌ npm audit returned no report:" >&2
  echo "$report" >&2
  exit 1
fi

# One line per advisory at moderate or above: severity, GHSA, package, title,
# and the reason when an exception accepts it.
advisories=$(jq -r --slurpfile exceptions "$exceptions" '
  ($exceptions[0] | map({key: .id, value: .reason}) | from_entries) as $accepted
  | [.vulnerabilities[].via[] | objects
     | select(.severity == "moderate" or .severity == "high" or .severity == "critical")]
  | unique_by(.url)
  | .[]
  | (.url | split("/") | last) as $id
  | [.severity, $id, .name, .title, ($accepted[$id] // "")]
  | @tsv' <<<"$report")

accepted=$(awk -F'\t' '$5 != ""' <<<"$advisories")
refused=$(awk -F'\t' 'NF && $5 == ""' <<<"$advisories")

if [ -n "$accepted" ]; then
  echo "Accepted by $exceptions:"
  awk -F'\t' '{ printf "  %s  %s  %s — %s\n", $1, $2, $3, $5 }' <<<"$accepted"
fi

if [ -n "$refused" ]; then
  echo "❌ $(wc -l <<<"$refused") advisory(ies) on runtime dependencies, none accepted:" >&2
  awk -F'\t' '{ printf "  %s  %s  %s — %s\n", $1, $2, $3, $4 }' <<<"$refused" >&2
  exit 1
fi

echo "✅ no unaccepted advisory at moderate or above on runtime dependencies"
