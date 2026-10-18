#!/usr/bin/env bash
# docker/pull-images.sh <registry> <version> <commit> <local name>...
#
# Pulls <registry>/yog-x:<version> for each local name yog-x:prod, and renames
# it only once EVERY image is there and labelled with <commit>: a refusal
# leaves the images the running version uses untouched.
set -euo pipefail

log() { printf '%s  %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }
die() { log "❌ $*" >&2; exit 1; }

[ $# -ge 4 ] || { echo "usage: $0 <registry> <version> <commit> <local name>..." >&2; exit 2; }
registry=$1 version=$2 commit=$3
shift 3
locals=("$@")

remotes=()
for local_name in "${locals[@]}"; do
  [[ $local_name == *:prod ]] || die "$local_name is not a local name of the form yog-x:prod"
  remote="$registry/${local_name%:prod}:$version"
  docker pull --quiet "$remote" >/dev/null || die "$remote cannot be pulled: missing, or not public?"
  revision=$(docker image inspect --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}' "$remote")
  [ "$revision" = "$commit" ] \
    || die "$remote is labelled with commit ${revision:-none}, not ${commit}: refused"
  remotes+=("$remote")
done

for i in "${!remotes[@]}"; do
  docker tag "${remotes[$i]}" "${locals[$i]}"
  # Untagged, the version left behind by the next deploy is prunable.
  docker image rm "${remotes[$i]}" >/dev/null
done
log "pulled ${#remotes[@]} images of $version (${commit:0:7})"
