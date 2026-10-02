#!/usr/bin/env bash
# docker/update.sh — update a running production server to origin/main.
#
#   docker/update.sh           deploy if origin/main is ahead of what was last deployed
#   docker/update.sh --force   run the whole sequence even if it is not
#   docker/update.sh --check   run the post-deploy checks only, change nothing
#
# Run it on the server, from anywhere. It is the one definition of the update
# procedure; a deployment pipeline calls this script and nothing else.
#
# It detaches itself. The work runs in its own session, writing to a log
# under .git/yog-update/logs/, and the foreground only follows that log and
# returns its exit code. An SSH session that drops, or a Ctrl-C, ends the
# foreground and leaves the update running: stopping it half-way would leave
# the daemons down. Re-attach with `tail -f` on the log it names; abort with
# `kill -- -<pid>` on the pid it names (the whole session, docker included).
#
# The sequence, and why each step sits where it does
# --------------------------------------------------
# 1. Refuse to start unless: no other run holds the lock, the checkout is on
#    `main`, no tracked file is modified (the .env is ignored by git), and
#    HEAD is an ancestor of origin/main (no local commit on the server).
# 2. Fast-forward to origin/main, then hand over to the script as it is in
#    the new commit: a deploy always runs the procedure it ships with.
# 3. Pull the registry images and build ours while the previous version
#    still serves: the outage lasts only for steps 4-6.
# 4. Stop every daemon that talks to the database. `up -d` alone is not
#    enough: it runs yog-migrate before STARTING services, but services
#    already running keep running through the migration, the old code
#    against the new schema. A migration that raises `pools.needs_refresh`
#    for a back-fill then has its flags consumed by the old yog-context,
#    which lowers them without writing what it does not know about: the
#    feature stays empty, everything looks green.
# 5. Migrate (`compose run yog-migrate`, i.e. `bootstrap`, idempotent). The
#    pools flagged `needs_refresh` are read just before and just after, with
#    every daemon stopped: the difference is what the migration flagged.
# 6. `up -d`, and record the commit as deployed (.git/yog-update/deployed).
#    What is deployed is that record, not the checkout: a run that fails
#    after the fast-forward leaves origin/main ahead of it, and the next plain
#    run deploys again.
# 7. Checks: every service running and stable (no restart for a minute),
#    yog-api's /readyz (it pings the database) and the dashboard answering
#    from inside the compose network, and the pools flagged by the migration
#    drained by yog-context. A pool yog-context cannot resolve (closed on
#    chain, rejected account) keeps its flag for good: once the count stops
#    falling, what remains is reported as a warning. The check fails on a
#    count that never moves, on a plateau of 100 or more (yog-context takes
#    the 100 oldest per tick, so they starve the rest), and on a count still
#    falling at the deadline.
#
# If the migration fails, the script stops with the daemons DOWN. The images
# already hold the new code, which must not run against the old schema. The
# message says whether the previous version can still be served.
#
# What this script does not do
# ----------------------------
# - Roll a schema back. Migrations are forward-only; going back is a restore
#   (crates/persistence/README.md, "Backup and restore").
# - Refresh a continuous aggregate. A full-range refresh typed by hand once
#   retention has dropped a chunk erases materialised history:
#   crates/persistence/migrations/README.md says which bounded form to use.
# - Check the drain from --check: it needs the flags read around a migration.
#
# Trap: the fast-forward rewrites this very file while bash is running it,
# and bash reads a script as it goes. Everything therefore lives in `main`,
# parsed whole before it runs, and the last line calls it and exits at once.
#
# Tunables (environment): UPDATE_STABLE_SECS (default 60),
# UPDATE_DRAIN_TIMEOUT_SECS (default 600). yog-context resolves 100 pools per
# protocol every CONTEXT_METADATA_POLL_SECS (10 s by default).

main() {
  set -euo pipefail
  local -r script=$(readlink -f "$0")
  cd "$(dirname "$script")/.."

  dc=(docker compose -f docker-compose.yml -f docker-compose.prod.yml --profile full)
  db_daemons=(yog-indexer yog-context yog-signals yog-api yog-archive)
  services=(postgres "${db_daemons[@]}" yog-web caddy)
  state=$(git rev-parse --absolute-git-dir)/yog-update
  mkdir -p "$state/logs"
  local -r stable_secs="${UPDATE_STABLE_SECS:-60}"
  local -r drain_timeout_secs="${UPDATE_DRAIN_TIMEOUT_SECS:-600}"

  local mode=deploy
  case "${1:-}" in
    "") ;;
    --force) mode=force ;;
    --check) mode=check ;;
    *) echo "usage: $0 [--force | --check]" >&2; exit 2 ;;
  esac

  if [ "$mode" = check ]; then
    exec 9>"$state/lock"
    if ! flock -n 9; then
      # The only window a pipeline has on a detached update: the log itself
      # is not reachable through the deployment key.
      local latest
      latest=$(find "$state/logs" -name '*.log' -printf '%T@ %p\n' | sort -n | tail -n 1 | cut -d' ' -f2-)
      log "an update is running; the end of its log (${latest:-none found}):"
      [ -z "$latest" ] || tail -n 40 "$latest"
      die "an update is running: check again once it is over"
    fi
    checks "$stable_secs" || die "checks failed (see above)"
    log "✅ all checks passed"
    return
  fi

  if [ -z "${YOG_UPDATE_RC:-}" ]; then
    follow_detached "$script" "$@"
  fi
  [ -n "${YOG_UPDATE_FROM:-}" ] || echo "$$" >"$YOG_UPDATE_RC.pid"
  trap 'echo $? >"$YOG_UPDATE_RC"' EXIT

  local old new
  if [ -z "${YOG_UPDATE_FROM:-}" ]; then
    # 1. Preconditions
    exec 9>"$state/lock"
    flock -n 9 || die "another update is running (lock $state/lock)"
    [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || die "the checkout is not on main"
    [ -z "$(git status --porcelain --untracked-files=no)" ] \
      || die "tracked files are modified: $(git status --porcelain --untracked-files=no | tr '\n' ' ')"
    git fetch --quiet origin main
    git merge-base --is-ancestor HEAD origin/main \
      || die "HEAD is not an ancestor of origin/main: local commits on the server?"

    # No record means unknown, not "HEAD is deployed": a checkout pulled by
    # hand, or a first run that failed after its fast-forward, deploys.
    old=$(cat "$state/deployed" 2>/dev/null || echo unknown)
    new=$(git rev-parse origin/main)
    if [ "$old" = "$new" ] && [ "$mode" = deploy ]; then
      log "nothing to deploy: ${new:0:7} is deployed and is origin/main"
      return
    fi

    # 2. Fast-forward, then run the procedure as the new commit has it. The
    #    lock (fd 9) and the exit-code file go along through the exec.
    log "deploying ${old:0:7} → ${new:0:7}"
    [ "$old" = unknown ] || git log --oneline "$old..$new" 2>/dev/null || true
    git merge --ff-only --quiet origin/main
    YOG_UPDATE_FROM=$old exec "$script" "$@"
  fi
  old=$YOG_UPDATE_FROM
  new=$(git rev-parse HEAD)

  # 3. Build while the previous version still serves
  log "pulling registry images, building ours"
  "${dc[@]}" pull --quiet --ignore-buildable
  "${dc[@]}" build
  local migrations_before migrations_after
  migrations_before=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations")

  # 4. Stop the daemons that talk to the database
  local -r outage_start=$(date +%s)
  log "stopping ${db_daemons[*]}"
  "${dc[@]}" stop "${db_daemons[@]}"

  # 5. Migrate, reading the flags on each side of it
  flagged_pools >"$state/flagged.before"
  log "migrating"
  if ! "${dc[@]}" run --rm -T yog-migrate; then
    migrations_after=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations" 2>/dev/null) || migrations_after=unknown
    log "❌ the migration failed. The daemons are STOPPED, on purpose: the images" >&2
    log "   now hold the new code, which must not run against the old schema." >&2
    if [ "$migrations_after" = unknown ]; then
      log "   The database does not answer either: whether a migration of this" >&2
      log "   deploy was applied is unknown. Bring Postgres back, then compare" >&2
      log "   _sqlx_migrations (${migrations_before} rows before) with the migrations directory." >&2
    elif [ "$migrations_after" = "$migrations_before" ]; then
      log "   No migration of this deploy was applied: the previous version can" >&2
      log "   still be served — git reset --hard ${old:0:7}, then" >&2
      log "   ${dc[*]} build && ${dc[*]} up -d" >&2
      log "   and fix forward on main before the next run, which would bring it back." >&2
    else
      log "   $(( migrations_after - migrations_before )) migration(s) of this deploy were applied before the" >&2
      log "   failure: the previous binaries refuse a schema newer than theirs." >&2
      log "   Fix forward (a new migration, never an edited one), or restore." >&2
    fi
    exit 1
  fi
  migrations_after=$(psql_admin -c "SELECT count(*) FROM _sqlx_migrations")
  flagged_pools | LC_ALL=C sort | LC_ALL=C comm -13 <(LC_ALL=C sort "$state/flagged.before") - \
    >"$state/flagged.by-migration"

  # 6. Start, and record what is now deployed
  # --no-deps with the services named: yog-migrate has just run, and `up`
  # would otherwise run it a second time inside the outage.
  log "starting"
  "${dc[@]}" up -d --no-deps "${services[@]}"
  echo "$new" >"$state/deployed"
  local -r outage_secs=$(( $(date +%s) - outage_start ))

  # 7. Checks
  local failed=0
  checks "$stable_secs" || failed=1
  drain "$state/flagged.by-migration" "$drain_timeout_secs" || failed=1

  log "summary: ${old:0:7} → ${new:0:7}, $(( migrations_after - migrations_before )) migration(s) applied, outage ${outage_secs}s"
  [ "$failed" -eq 0 ] || die "deployed, but a check failed (see above)"
  log "✅ deployed and checked"
}

log() { printf '%s  %s\n' "$(date -u +%H:%M:%SZ)" "$*"; }
die() { log "❌ $*" >&2; exit 1; }

# Start the update in its own session and follow its log until it writes its
# exit code. Never returns.
follow_detached() { # <script> <args...>
  local -r script=$1; shift
  local -r logf="$state/logs/$(date -u +%Y%m%dT%H%M%SZ)-$$.log"
  local -r rcf="$logf.rc"
  : >"$logf"
  YOG_UPDATE_RC=$rcf setsid "$script" "$@" >"$logf" 2>&1 </dev/null &
  echo "update running detached as pid $!; log: $logf"
  echo "  re-attach: tail -f $logf    abort: kill -- -$!  (its whole session)"
  # A background job of a non-interactive shell ignores SIGINT: tail would
  # outlive a Ctrl-C and keep printing over the next prompt.
  tail -n +1 -f "$logf" &
  local -r tail_pid=$!
  trap 'kill "$tail_pid" 2>/dev/null' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  trap 'exit 129' HUP
  local pid=""
  while [ ! -s "$rcf" ]; do
    sleep 1
    [ -n "$pid" ] || pid=$(cat "$rcf.pid" 2>/dev/null || true)
    if [ -n "$pid" ] && ! kill -0 "$pid" 2>/dev/null && [ ! -s "$rcf" ]; then
      kill "$tail_pid" 2>/dev/null || true
      die "the update (pid $pid) ended without an exit code: read $logf"
    fi
  done
  sleep 1
  kill "$tail_pid" 2>/dev/null || true
  exit "$(cat "$rcf")"
}

# psql as the admin role, with the credentials of the postgres container itself.
# The single quotes are the point: $POSTGRES_USER is the container's, not ours.
psql_admin() {
  # shellcheck disable=SC2016
  "${dc[@]}" exec -T postgres sh -c \
    'exec psql -X -q -At -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$@"' sh "$@"
}

flagged_pools() { psql_admin -c "SELECT pool_address FROM pools WHERE needs_refresh ORDER BY 1"; }

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

  # From inside the compose network, through caddy's busybox wget (which
  # fails on any non-2xx). A service just started may not listen yet: keep
  # trying for 60 s before calling it a failure.
  local api="" web="" body="" deadline=$(( $(date +%s) + 60 ))
  while :; do
    if [ "$api" != ok ]; then
      body=$("${dc[@]}" exec -T caddy wget -q -O - -T 5 http://yog-api:5000/readyz 2>&1) || body=""
      [[ $body == *'"ready"'* ]] && api=ok
    fi
    [ "$web" = ok ] || web=$("${dc[@]}" exec -T caddy wget -q -O /dev/null -T 5 http://yog-web:3000/ 2>&1 && echo ok || true)
    if { [ "$api" = ok ] && [ "$web" = ok ]; } || [ "$(date +%s)" -ge "$deadline" ]; then break; fi
    sleep 3
  done
  [ "$api" = ok ] || { log "❌ yog-api /readyz did not answer ready within 60 s"; failed=1; }
  [ "$web" = ok ] || { log "❌ yog-web did not answer 2xx within 60 s"; failed=1; }

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
  [ "$failed" -eq 0 ] && log "services running and stable, /readyz ready, dashboard ok"
  return "$failed"
}

# How many of the pools in <file> are still flagged. The list goes in on
# stdin (COPY), never as an argument: a back-fill can flag thousands of pools.
still_flagged() { # <file>
  { echo "CREATE TEMP TABLE flagged (pool_address text); COPY flagged FROM STDIN;"
    cat "$1"
    printf '%s\n' '\.' "SELECT count(*) FROM pools JOIN flagged USING (pool_address) WHERE needs_refresh;"
  } | psql_admin
}

drain() { # <file of pools flagged by the migration> <timeout_secs>
  local total remaining last="" flat=0 deadline
  total=$(grep -c . "$1" || true)
  if [ "$total" -eq 0 ]; then
    log "the migration flagged no pool needs_refresh: nothing to drain"
    return 0
  fi
  deadline=$(( $(date +%s) + $2 ))
  while :; do
    remaining=$(still_flagged "$1")
    [[ $remaining =~ ^[0-9]+$ ]] || { log "❌ could not count the flagged pools: ${remaining:-nothing}"; return 1; }
    log "needs_refresh: ${remaining}/${total} of the pools the migration flagged still pending"
    [ "$remaining" -eq 0 ] && return 0
    if [ "$remaining" = "$last" ]; then flat=$(( flat + 1 )); else flat=0; fi
    last=$remaining
    if [ "$flat" -lt 4 ] && [ "$(date +%s)" -ge "$deadline" ]; then
      log "❌ still draining at the deadline (${remaining} left, still falling): raise UPDATE_DRAIN_TIMEOUT_SECS,"
      log "   or follow it: SELECT count(*) FROM pools WHERE needs_refresh"
      return 1
    fi
    if [ "$flat" -ge 4 ]; then
      if [ "$remaining" -eq "$total" ]; then
        log "❌ none of the ${total} pools flagged by the migration was resolved: is yog-context working?"
        return 1
      fi
      # yog-context takes the 100 oldest flagged pools per protocol each
      # tick: 100 that never resolve hold back every pool behind them.
      if [ "$remaining" -ge 100 ]; then
        log "❌ stuck at ${remaining}: at 100 or more, unresolvable pools fill yog-context's batch and starve the rest"
        return 1
      fi
      log "⚠️  ${remaining} pool(s) keep their flag: yog-context cannot resolve them (closed on chain or rejected; its WARN lines name them)"
      return 0
    fi
    sleep 15
  done
}

main "$@"; exit
