#!/usr/bin/env bash
# docker/update.sh — update a running production server to origin/main.
#
#   docker/update.sh           deploy if origin/main moved; otherwise say so and touch nothing
#   docker/update.sh --force   run the whole sequence even with no new commit
#   docker/update.sh --check   run the post-deploy checks only, change nothing
#
# Run it on the server, from anywhere: it moves to the repository root itself.
# It is the one definition of the update procedure; a deployment pipeline
# calls this script and nothing else.
#
# The sequence, and why each step sits where it does
# --------------------------------------------------
# 1. Refuse to start unless: no other run holds the lock, the checkout is on
#    `main`, and no tracked file is modified (the .env is ignored by git).
# 2. Fast-forward to origin/main, never merge.
# 3. Build the images while the previous version still serves: the outage
#    lasts only for steps 4-6.
# 4. Stop every daemon that talks to the database. `up -d` alone is not
#    enough: it runs yog-migrate before STARTING services, but services
#    already running keep running through the migration, the old code
#    against the new schema. A migration that raises `pools.needs_refresh`
#    for a back-fill then has its flags consumed by the old yog-context,
#    which lowers them without writing what it does not know about: the
#    feature stays empty, everything looks green.
# 5. Migrate (`compose run yog-migrate`, i.e. `bootstrap`, idempotent), then
#    record which pools are flagged `needs_refresh`.
# 6. `up -d`.
# 7. Checks: every service running, stable (no restart over a minute),
#    /healthz of yog-api and the dashboard answering from inside the compose
#    network, and the pools flagged at step 5 drained by yog-context.
#
# If the migration fails, the script stops with the daemons DOWN. The images
# were already rebuilt with the new code: starting them again would run the
# new code against the old schema. The message gives the previous commit to
# check out, rebuild and start.
#
# What this script does not do
# ----------------------------
# - Roll a schema back. Migrations are forward-only; going back is a restore
#   (crates/persistence/README.md, "Backup and restore").
# - Refresh a continuous aggregate. A full-range refresh typed by hand once
#   retention has dropped a chunk erases materialised history:
#   crates/persistence/migrations/README.md says which bounded form to use.
#
# Trap: the merge in step 2 rewrites this very file while bash is running it,
# and bash reads a script as it goes. Everything therefore lives in `main`,
# parsed whole before it runs, and the last line calls it and exits at once.
#
# Tunables (environment): UPDATE_STABLE_SECS (default 60),
# UPDATE_DRAIN_TIMEOUT_SECS (default 600). yog-context resolves 100 pools per
# protocol every CONTEXT_METADATA_POLL_SECS (10 s by default).

main() {
  set -euo pipefail
  cd "$(dirname "$0")/.."

  local -r stable_secs="${UPDATE_STABLE_SECS:-60}"
  local -r drain_timeout_secs="${UPDATE_DRAIN_TIMEOUT_SECS:-600}"
  dc=(docker compose -f docker-compose.yml -f docker-compose.prod.yml --profile full)
  db_daemons=(yog-indexer yog-context yog-signals yog-api yog-archive)
  services=(postgres "${db_daemons[@]}" yog-web caddy)

  local mode=deploy
  case "${1:-}" in
    "") ;;
    --force) mode=force ;;
    --check) mode=check ;;
    *) echo "usage: $0 [--force | --check]" >&2; exit 2 ;;
  esac

  if [ "$mode" = check ]; then
    checks "$stable_secs" || exit 1
    log "✅ all checks passed"
    return
  fi

  # 1. Preconditions
  exec 9>/tmp/yog-update.lock
  flock -n 9 || die "another update is running (lock /tmp/yog-update.lock)"
  [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || die "the checkout is not on main"
  [ -z "$(git status --porcelain --untracked-files=no)" ] \
    || die "tracked files are modified: $(git status --porcelain --untracked-files=no | tr '\n' ' ')"

  # 2. Fast-forward
  git fetch --quiet origin main
  local -r old=$(git rev-parse HEAD) new=$(git rev-parse origin/main)
  if [ "$old" = "$new" ] && [ "$mode" = deploy ]; then
    log "nothing to deploy: main is at origin/main (${old:0:7})"
    return
  fi
  git merge-base --is-ancestor HEAD origin/main \
    || die "HEAD is not an ancestor of origin/main: local commits on the server?"
  log "deploying ${old:0:7} → ${new:0:7}"
  git log --oneline "$old..$new"
  git merge --ff-only --quiet origin/main

  # 3. Build while the previous version still serves
  log "building images"
  "${dc[@]}" build
  local migrations_before migrations_after flagged
  migrations_before=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations")

  # 4. Stop the daemons that talk to the database
  local -r outage_start=$(date +%s)
  log "stopping ${db_daemons[*]}"
  "${dc[@]}" stop "${db_daemons[@]}"

  # 5. Migrate
  log "migrating"
  if ! "${dc[@]}" run --rm -T yog-migrate; then
    log "❌ the migration failed. The daemons are STOPPED, on purpose: the images" >&2
    log "   now hold the new code, which must not run against the old schema." >&2
    log "   To serve the previous version: git checkout ${old:0:7}, then" >&2
    log "   ${dc[*]} build && ${dc[*]} up -d" >&2
    exit 1
  fi
  migrations_after=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations")
  flagged=$(psql_admin -c "SELECT coalesce(string_agg(pool_address, ','), '') FROM pools WHERE needs_refresh")

  # 6. Start
  log "starting"
  "${dc[@]}" up -d
  local -r outage_secs=$(( $(date +%s) - outage_start ))

  # 7. Checks
  local failed=0
  checks "$stable_secs" || failed=1
  drain "$flagged" "$drain_timeout_secs" || failed=1

  log "summary: ${old:0:7} → ${new:0:7}, $(( migrations_after - migrations_before )) migration(s) applied, outage ${outage_secs}s"
  [ "$failed" -eq 0 ] || die "deployed, but a check failed (see above)"
  log "✅ deployed and checked"
}

log() { printf '%s  %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }
die() { log "❌ $*" >&2; exit 1; }

# psql as the admin role, with the credentials of the postgres container itself.
# The single quotes are the point: $POSTGRES_USER is the container's, not ours.
psql_admin() {
  # shellcheck disable=SC2016
  "${dc[@]}" exec -T postgres sh -c \
    'exec psql -X -q -At -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$@"' sh "$@"
}

container_field() { # <service> <go-template>
  local id
  id=$("${dc[@]}" ps -a -q "$1")
  [ -n "$id" ] || { echo missing; return; }
  docker inspect -f "$2" "$id"
}

checks() { # <stable_secs>
  local failed=0 s
  local -A restarts=()
  for s in "${services[@]}"; do
    local status
    status=$(container_field "$s" '{{.State.Status}}')
    if [ "$status" = running ]; then
      restarts[$s]=$(container_field "$s" '{{.RestartCount}}')
    else
      log "❌ $s is $status, not running"; failed=1
    fi
  done

  # From inside the compose network, through caddy's busybox wget. A service
  # just started may not listen yet: up to 30 s before calling it a failure.
  local api web try
  for try in $(seq 10); do
    api=$("${dc[@]}" exec -T caddy wget -q -O - -T 10 http://yog-api:5000/healthz 2>&1) || true
    web=$("${dc[@]}" exec -T caddy wget -q -O /dev/null -T 20 http://yog-web:3000/ 2>&1 && echo ok) || true
    [ "$api" = ok ] && [ "$web" = ok ] && break
    [ "$try" -lt 10 ] && sleep 3
  done
  [ "$api" = ok ] || { log "❌ yog-api /healthz answered: ${api:-nothing}"; failed=1; }
  [ "$web" = ok ] || { log "❌ yog-web did not answer 2xx: ${web:-nothing}"; failed=1; }

  log "watching restarts for ${1}s"
  sleep "$1"
  for s in "${!restarts[@]}"; do
    local now status
    status=$(container_field "$s" '{{.State.Status}}')
    now=$(container_field "$s" '{{.RestartCount}}')
    if [ "$status" != running ] || [ "$now" != "${restarts[$s]}" ]; then
      log "❌ $s is unstable: $status, restarts ${restarts[$s]} → $now"; failed=1
    fi
  done
  [ "$failed" -eq 0 ] && log "services running and stable, /healthz ok, dashboard ok"
  return "$failed"
}

drain() { # <comma-separated pool addresses flagged after the migration> <timeout_secs>
  if [ -z "$1" ]; then
    log "no pool flagged needs_refresh after the migration: nothing to drain"
    return 0
  fi
  local total remaining deadline
  total=$(tr ',' '\n' <<<"$1" | wc -l)
  deadline=$(( $(date +%s) + $2 ))
  while :; do
    # On stdin, not -c: psql interpolates :'flagged' only in what it reads.
    remaining=$(psql_admin -v "flagged=$1" <<<\
      "SELECT count(*) FROM pools WHERE needs_refresh AND pool_address = ANY(string_to_array(:'flagged', ','));")
    [[ $remaining =~ ^[0-9]+$ ]] || { log "❌ could not count the flagged pools: ${remaining:-nothing}"; return 1; }
    log "needs_refresh: ${remaining}/${total} of the pools flagged after the migration still pending"
    [ "$remaining" -eq 0 ] && return 0
    [ "$(date +%s)" -lt "$deadline" ] || { log "❌ not drained after ${2}s"; return 1; }
    sleep 15
  done
}

main "$@"; exit
